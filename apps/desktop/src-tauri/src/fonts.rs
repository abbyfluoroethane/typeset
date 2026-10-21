//! Installing fonts into the per-user font directory of each OS, and
//! reconciling installed fonts with the synced library.
//!
//! Google font files are always downloaded fresh from the URLs in the
//! cached catalog; uploaded fonts come from `GET /api/uploads/{id}/file`.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::{sync::Semaphore, task::JoinSet};
use typeset_core::{
    CatalogFamily, CatalogResponse, FontSource, LibraryEntry, Upload, parse_family_key,
};

use crate::{
    api::{self, Api},
    error::{AppError, AppResult},
    state::{AppState, now},
};

mod platform;

/// Downloads running at once for one family.
const MAX_CONCURRENT_DOWNLOADS: usize = 4;

/// Serializes installs, removals and reconciles so two of them never write
/// the same files.
static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFile {
    pub variant: String,
    pub path: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFamily {
    pub key: String,
    pub family: String,
    pub source: FontSource,
    pub files: Vec<InstalledFile>,
    pub installed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileFailure {
    pub key: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReport {
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub failed: Vec<ReconcileFailure>,
}

/// Makes installed fonts match the local library mirror: installs missing
/// live entries, reinstalls Google families whose catalog version changed
/// (and uploads whose sha256 changed), and removes families whose entry is
/// a tombstone. Called by `sync::sync_now` when `auto_install` is on.
pub async fn reconcile(state: &AppState) -> AppResult<ReconcileReport> {
    let library = load_library(state).await?;
    let uploads = load_uploads(state).await?;
    let installed = load_installed_rows(state).await?;
    let catalog = load_catalog(state).await.map_err(|e| e.to_string());

    let items: Vec<LibraryItem> = library
        .iter()
        .map(|entry| LibraryItem {
            key: entry.key.clone(),
            live: !entry.deleted,
            desired: desired_for(entry, &uploads, catalog.as_ref().ok()),
        })
        .collect();
    let installed_map: InstalledMap = installed
        .iter()
        .map(|(key, rows)| {
            let files = rows.iter().map(|r| (r.variant.clone(), r.version.clone()));
            (key.clone(), files.collect())
        })
        .collect();
    let plan = diff(&items, &installed_map);

    let mut report = ReconcileReport::default();
    let catalog_ref = catalog.as_ref().map_err(String::as_str);
    for (keys, list) in [
        (&plan.install, &mut report.installed),
        (&plan.update, &mut report.updated),
    ] {
        for key in keys {
            match install_inner(state, key, catalog_ref).await {
                Ok(_) => list.push(key.clone()),
                Err(e) => report.failed.push(ReconcileFailure {
                    key: key.clone(),
                    error: e.to_string(),
                }),
            }
        }
    }
    for key in &plan.remove {
        match uninstall_inner(state, key).await {
            Ok(()) => report.removed.push(key.clone()),
            Err(e) => report.failed.push(ReconcileFailure {
                key: key.clone(),
                error: e.to_string(),
            }),
        }
    }
    Ok(report)
}

#[tauri::command]
pub async fn list_installed(state: State<'_, AppState>) -> AppResult<Vec<InstalledFamily>> {
    let rows = load_installed_rows(&state).await?;
    Ok(rows
        .into_iter()
        .filter_map(|(key, rows)| family_from_rows(key, rows))
        .collect())
}

#[tauri::command]
pub async fn install_family(state: State<'_, AppState>, key: String) -> AppResult<InstalledFamily> {
    // Only Google families need the catalog; a load failure is reported by
    // `install_inner` only if it is needed.
    let catalog = match parse_family_key(&key) {
        Some((FontSource::Google, _)) => load_catalog(&state).await.map_err(|e| e.to_string()),
        _ => Err(String::new()),
    };
    install_inner(&state, &key, catalog.as_ref().map_err(String::as_str)).await
}

#[tauri::command]
pub async fn uninstall_family(state: State<'_, AppState>, key: String) -> AppResult<()> {
    uninstall_inner(&state, &key).await
}

#[tauri::command]
pub async fn reconcile_fonts(state: State<'_, AppState>) -> AppResult<ReconcileReport> {
    reconcile(&state).await
}

// ---------------------------------------------------------------------------
// Pure helpers

/// What the library says a live family should look like on disk.
#[derive(Debug, Clone, PartialEq)]
enum Desired {
    Google {
        version: String,
        variants: BTreeSet<String>,
    },
    /// Upload id to sha256.
    Upload(BTreeMap<String, String>),
}

#[derive(Debug, Clone)]
struct LibraryItem {
    key: String,
    live: bool,
    /// `None` when it cannot be known (family missing from the catalog, or
    /// no catalog); the family is then only installed if absent.
    desired: Option<Desired>,
}

/// Family key to `(variant or upload id, recorded version)` pairs.
type InstalledMap = BTreeMap<String, Vec<(String, String)>>;

#[derive(Debug, Default, PartialEq)]
struct Plan {
    install: Vec<String>,
    update: Vec<String>,
    remove: Vec<String>,
}

fn diff(items: &[LibraryItem], installed: &InstalledMap) -> Plan {
    // Families installed on this machine only (no library entry) are left
    // alone: only an explicit library delete removes fonts.
    let mut plan = Plan::default();
    for item in items {
        let current = installed.get(&item.key).filter(|files| !files.is_empty());
        match (item.live, current) {
            (false, Some(_)) => plan.remove.push(item.key.clone()),
            (false, None) => {}
            (true, None) => plan.install.push(item.key.clone()),
            (true, Some(files)) => {
                let stale = match &item.desired {
                    None => false,
                    Some(Desired::Google { version, variants }) => {
                        let have: BTreeSet<&String> = files.iter().map(|f| &f.0).collect();
                        files.iter().any(|f| &f.1 != version)
                            || have != variants.iter().collect::<BTreeSet<_>>()
                    }
                    Some(Desired::Upload(want)) => {
                        let have: BTreeMap<String, String> = files.iter().cloned().collect();
                        &have != want
                    }
                };
                if stale {
                    plan.update.push(item.key.clone());
                }
            }
        }
    }
    plan
}

/// Keeps ASCII letters, digits, `-` and `_`; everything else is dropped.
fn sanitize(s: &str) -> String {
    let out: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if out.is_empty() { "Font".into() } else { out }
}

fn google_stem(family: &str, variant: &str) -> String {
    format!("{}-{}", sanitize(family), sanitize(variant))
}

fn upload_stem(family: &str, style: &str, id: &str) -> String {
    let short: String = sanitize(id).chars().take(8).collect();
    format!("{}-{}-{}", sanitize(family), sanitize(style), short)
}

/// `otf` for CFF fonts (magic `OTTO`), `ttf` for everything else.
fn detect_ext(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"OTTO") {
        "otf"
    } else {
        "ttf"
    }
}

/// Human style name for a Google variant: `regular`, `italic`, `700`,
/// `700italic`.
fn style_name(variant: &str) -> String {
    let (weight, italic) = match variant.strip_suffix("italic") {
        Some(w) => (w, true),
        None => (variant, false),
    };
    let name = match weight {
        "100" => "Thin",
        "200" => "ExtraLight",
        "300" => "Light",
        "" | "regular" | "400" => "Regular",
        "500" => "Medium",
        "600" => "SemiBold",
        "700" => "Bold",
        "800" => "ExtraBold",
        "900" => "Black",
        other => other,
    };
    match (name, italic) {
        ("Regular", true) => "Italic".into(),
        (n, true) => format!("{n} Italic"),
        (n, false) => n.into(),
    }
}

/// Variants to install with their URLs: the wanted ones that the catalog
/// has, or all of them when `wanted` is empty.
fn select_variants(
    family: &CatalogFamily,
    wanted: &[String],
) -> Result<Vec<(String, String)>, String> {
    let picked: Vec<(String, String)> = if wanted.is_empty() {
        family
            .files
            .iter()
            .map(|(v, u)| (v.clone(), u.clone()))
            .collect()
    } else {
        let mut seen = BTreeSet::new();
        wanted
            .iter()
            .filter(|v| seen.insert(v.as_str()))
            .filter_map(|v| family.files.get(v).map(|u| (v.clone(), u.clone())))
            .collect()
    };
    if picked.is_empty() {
        return Err(format!("{} has no downloadable variants", family.family));
    }
    Ok(picked)
}

fn desired_for(
    entry: &LibraryEntry,
    uploads: &[Upload],
    catalog: Option<&CatalogResponse>,
) -> Option<Desired> {
    match entry.source {
        FontSource::Google => {
            let family = catalog?
                .families
                .iter()
                .find(|f| f.family == entry.family)?;
            let variants = select_variants(family, &entry.variants).ok()?;
            Some(Desired::Google {
                version: family.version.clone(),
                variants: variants.into_iter().map(|(v, _)| v).collect(),
            })
        }
        FontSource::Upload => Some(Desired::Upload(
            family_uploads(uploads, &entry.family)
                .map(|u| (u.id.clone(), u.sha256.clone()))
                .collect(),
        )),
    }
}

fn family_uploads<'a>(uploads: &'a [Upload], family: &'a str) -> impl Iterator<Item = &'a Upload> {
    uploads
        .iter()
        .filter(move |u| !u.deleted && u.family == family)
}

// ---------------------------------------------------------------------------
// Database

struct Row {
    variant: String,
    path: String,
    version: String,
    installed_at: i64,
}

async fn load_installed_rows(state: &AppState) -> AppResult<BTreeMap<String, Vec<Row>>> {
    let rows: Vec<(String, String, String, String, i64)> = sqlx::query_as(
        "SELECT family_key, variant, path, version, installed_at
         FROM installed_files ORDER BY family_key, variant",
    )
    .fetch_all(&state.db)
    .await?;
    let mut map: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for (key, variant, path, version, installed_at) in rows {
        map.entry(key).or_default().push(Row {
            variant,
            path,
            version,
            installed_at,
        });
    }
    Ok(map)
}

fn family_from_rows(key: String, rows: Vec<Row>) -> Option<InstalledFamily> {
    let (source, family) = parse_family_key(&key)?;
    let family = family.to_string();
    Some(InstalledFamily {
        installed_at: rows.iter().map(|r| r.installed_at).max().unwrap_or(0),
        files: rows
            .into_iter()
            .map(|r| InstalledFile {
                variant: r.variant,
                path: r.path,
                version: r.version,
            })
            .collect(),
        key,
        family,
        source,
    })
}

async fn load_library(state: &AppState) -> AppResult<Vec<LibraryEntry>> {
    let bodies: Vec<String> = sqlx::query_scalar("SELECT body FROM library")
        .fetch_all(&state.db)
        .await?;
    Ok(bodies
        .iter()
        .map(|b| serde_json::from_str(b))
        .collect::<Result<_, _>>()?)
}

async fn load_uploads(state: &AppState) -> AppResult<Vec<Upload>> {
    let bodies: Vec<String> = sqlx::query_scalar("SELECT body FROM uploads")
        .fetch_all(&state.db)
        .await?;
    Ok(bodies
        .iter()
        .map(|b| serde_json::from_str(b))
        .collect::<Result<_, _>>()?)
}

/// Reads the cached catalog, fetching and caching it when there is none.
async fn load_catalog(state: &AppState) -> AppResult<CatalogResponse> {
    let cached: Option<String> = sqlx::query_scalar("SELECT body FROM catalog_cache WHERE id = 1")
        .fetch_optional(&state.db)
        .await?;
    if let Some(body) = cached {
        return Ok(serde_json::from_str(&body)?);
    }
    let catalog: CatalogResponse = Api::new(state).await?.get("/api/catalog").await?;
    sqlx::query(
        "INSERT INTO catalog_cache (id, fetched_at, body) VALUES (1, ?, ?)
         ON CONFLICT (id) DO UPDATE SET fetched_at = excluded.fetched_at, body = excluded.body",
    )
    .bind(catalog.fetched_at)
    .bind(serde_json::to_string(&catalog)?)
    .execute(&state.db)
    .await?;
    Ok(catalog)
}

// ---------------------------------------------------------------------------
// Install and uninstall

enum FileSource {
    Url(String),
    Upload(String),
}

struct Wanted {
    /// Google variant or upload id; the `installed_files.variant` value.
    variant: String,
    version: String,
    style: String,
    stem: String,
    source: FileSource,
}

async fn wanted_files(
    state: &AppState,
    key: &str,
    source: FontSource,
    family: &str,
    catalog: Result<&CatalogResponse, &str>,
) -> AppResult<Vec<Wanted>> {
    match source {
        FontSource::Google => {
            let catalog =
                catalog.map_err(|e| AppError::Other(format!("font catalog unavailable: {e}")))?;
            let cat_family = catalog
                .families
                .iter()
                .find(|f| f.family == family)
                .ok_or_else(|| AppError::Other(format!("{family} is not in the font catalog")))?;
            let body: Option<String> = sqlx::query_scalar("SELECT body FROM library WHERE key = ?")
                .bind(key)
                .fetch_optional(&state.db)
                .await?;
            let entry: Option<LibraryEntry> = body.map(|b| serde_json::from_str(&b)).transpose()?;
            let wanted = match &entry {
                Some(e) if !e.deleted => e.variants.as_slice(),
                _ => &[],
            };
            let picked = select_variants(cat_family, wanted).map_err(AppError::Other)?;
            Ok(picked
                .into_iter()
                .map(|(variant, url)| Wanted {
                    stem: google_stem(family, &variant),
                    style: style_name(&variant),
                    version: cat_family.version.clone(),
                    source: FileSource::Url(url),
                    variant,
                })
                .collect())
        }
        FontSource::Upload => {
            let uploads = load_uploads(state).await?;
            let list: Vec<Wanted> = family_uploads(&uploads, family)
                .map(|u| Wanted {
                    variant: u.id.clone(),
                    version: u.sha256.clone(),
                    style: u.style.clone(),
                    stem: upload_stem(family, &u.style, &u.id),
                    source: FileSource::Upload(u.id.clone()),
                })
                .collect();
            if list.is_empty() {
                return Err(AppError::Other(format!("no uploaded files for {family}")));
            }
            Ok(list)
        }
    }
}

/// Downloads every wanted file, a few at a time, returning bytes in order.
async fn download_all(state: &AppState, wanted: &[Wanted]) -> AppResult<Vec<Vec<u8>>> {
    let api = if wanted
        .iter()
        .any(|w| matches!(w.source, FileSource::Upload(_)))
    {
        Some(Arc::new(Api::new(state).await?))
    } else {
        None
    };
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
    let mut tasks = JoinSet::new();
    for (index, w) in wanted.iter().enumerate() {
        let permits = permits.clone();
        let http = state.http.clone();
        let api = api.clone();
        let (url, upload_id) = match &w.source {
            FileSource::Url(u) => (Some(u.clone()), None),
            FileSource::Upload(id) => (None, Some(id.clone())),
        };
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await;
            let bytes = match (url, upload_id, api) {
                (Some(url), _, _) => {
                    let response = http.get(&url).send().await?.error_for_status()?;
                    response.bytes().await?.to_vec()
                }
                (_, Some(id), Some(api)) => {
                    api.get_bytes(&format!("/api/uploads/{}/file", api::encode_segment(&id)))
                        .await?
                }
                _ => return Err(AppError::Other("no download source".into())),
            };
            if bytes.len() < 12 {
                return Err(AppError::Other("downloaded font file is empty".into()));
            }
            Ok::<_, AppError>((index, bytes))
        });
    }
    let mut out: Vec<Option<Vec<u8>>> = vec![None; wanted.len()];
    while let Some(joined) = tasks.join_next().await {
        let (index, bytes) = joined.map_err(|e| AppError::Other(e.to_string()))??;
        out[index] = Some(bytes);
    }
    Ok(out.into_iter().flatten().collect())
}

/// Writes `bytes` to `dir/name` via a temp file and rename.
async fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let target = dir.join(name);
    let temp = dir.join(format!(".{name}.tmp"));
    tokio::fs::write(&temp, bytes).await?;
    if let Err(e) = tokio::fs::rename(&temp, &target).await {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(e);
    }
    Ok(target)
}

async fn remove_recorded_file(path: &str) {
    let path = Path::new(path);
    platform::unregister(path);
    match tokio::fs::remove_file(path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!("could not delete {}: {e}", path.display()),
    }
}

/// Installs the wanted files of one family, replaces the recorded rows and
/// removes previously installed files that are no longer wanted. `catalog`
/// is only consulted for Google families.
async fn install_inner(
    state: &AppState,
    key: &str,
    catalog: Result<&CatalogResponse, &str>,
) -> AppResult<InstalledFamily> {
    let _guard = INSTALL_LOCK.lock().await;
    let (source, family) = parse_family_key(key)
        .ok_or_else(|| AppError::Other(format!("invalid family key: {key}")))?;

    let wanted = wanted_files(state, key, source, family, catalog).await?;
    let blobs = download_all(state, &wanted).await?;

    let old = load_installed_rows(state)
        .await?
        .remove(key)
        .unwrap_or_default();
    let old_paths: BTreeSet<&str> = old.iter().map(|r| r.path.as_str()).collect();
    let dir = platform::font_dir()?;
    tokio::fs::create_dir_all(&dir).await?;

    let mut written: Vec<(PathBuf, &Wanted)> = Vec::new();
    let mut failure = None;
    for (w, bytes) in wanted.iter().zip(&blobs) {
        let ext = detect_ext(bytes);
        let target = dir.join(format!("{}.{ext}", w.stem));
        if old_paths.contains(target.to_string_lossy().as_ref()) {
            // Windows cannot replace a file that is loaded as a font.
            platform::unregister(&target);
        }
        let result = match write_atomic(&dir, &format!("{}.{ext}", w.stem), bytes).await {
            Ok(path) => platform::register(&path, family, &w.style, ext == "otf").map(|_| path),
            Err(e) => Err(e.into()),
        };
        match result {
            Ok(path) => written.push((path, w)),
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    if let Some(e) = failure {
        // Roll back files this call created; keep any that replaced old ones.
        for (path, _) in &written {
            if !old_paths.contains(path.to_string_lossy().as_ref()) {
                remove_recorded_file(&path.to_string_lossy()).await;
            }
        }
        platform::refresh(&dir).await;
        return Err(e);
    }

    let installed_at = now();
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM installed_files WHERE family_key = ?")
        .bind(key)
        .execute(&mut *tx)
        .await?;
    for (path, w) in &written {
        sqlx::query(
            "INSERT INTO installed_files (family_key, variant, path, version, installed_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(key)
        .bind(&w.variant)
        .bind(path.to_string_lossy().as_ref())
        .bind(&w.version)
        .bind(installed_at)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let new_paths: BTreeSet<String> = written
        .iter()
        .map(|(p, _)| p.to_string_lossy().into_owned())
        .collect();
    for row in old.iter().filter(|r| !new_paths.contains(&r.path)) {
        remove_recorded_file(&row.path).await;
    }
    platform::refresh(&dir).await;

    Ok(InstalledFamily {
        key: key.to_string(),
        family: family.to_string(),
        source,
        files: written
            .iter()
            .map(|(path, w)| InstalledFile {
                variant: w.variant.clone(),
                path: path.to_string_lossy().into_owned(),
                version: w.version.clone(),
            })
            .collect(),
        installed_at,
    })
}

/// Deletes the recorded files of one family and their rows.
async fn uninstall_inner(state: &AppState, key: &str) -> AppResult<()> {
    let _guard = INSTALL_LOCK.lock().await;
    let rows = load_installed_rows(state)
        .await?
        .remove(key)
        .unwrap_or_default();
    for row in &rows {
        remove_recorded_file(&row.path).await;
    }
    sqlx::query("DELETE FROM installed_files WHERE family_key = ?")
        .bind(key)
        .execute(&state.db)
        .await?;
    if !rows.is_empty() {
        platform::refresh(&platform::font_dir()?).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_family(variants: &[&str], version: &str) -> CatalogFamily {
        CatalogFamily {
            family: "Roboto".into(),
            category: "sans-serif".into(),
            variants: variants.iter().map(|v| v.to_string()).collect(),
            subsets: vec![],
            version: version.into(),
            last_modified: String::new(),
            files: variants
                .iter()
                .map(|v| (v.to_string(), format!("https://x/{v}.ttf")))
                .collect(),
            menu: None,
        }
    }

    #[test]
    fn sanitizes_and_names_files() {
        assert_eq!(sanitize("Noto Sans JP"), "NotoSansJP");
        assert_eq!(sanitize("../../etc"), "etc");
        assert_eq!(sanitize("日本"), "Font");
        assert_eq!(google_stem("Open Sans", "700italic"), "OpenSans-700italic");
        assert_eq!(
            upload_stem("My Font", "Bold Italic", "abcdef12-3456"),
            "MyFont-BoldItalic-abcdef12"
        );
        assert_eq!(detect_ext(b"OTTO\0\0\0\0\0\0\0\0"), "otf");
        assert_eq!(detect_ext(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), "ttf");
    }

    #[test]
    fn names_styles() {
        assert_eq!(style_name("regular"), "Regular");
        assert_eq!(style_name("italic"), "Italic");
        assert_eq!(style_name("700"), "Bold");
        assert_eq!(style_name("300italic"), "Light Italic");
    }

    #[test]
    fn selects_variants() {
        let f = catalog_family(&["regular", "700"], "v1");
        assert_eq!(select_variants(&f, &[]).unwrap().len(), 2);
        let some = select_variants(&f, &["700".into(), "700".into(), "900".into()]).unwrap();
        assert_eq!(
            some,
            vec![("700".to_string(), "https://x/700.ttf".to_string())]
        );
        assert!(select_variants(&f, &["900".into()]).is_err());
    }

    fn google(version: &str, variants: &[&str]) -> Option<Desired> {
        Some(Desired::Google {
            version: version.into(),
            variants: variants.iter().map(|v| v.to_string()).collect(),
        })
    }

    fn item(key: &str, live: bool, desired: Option<Desired>) -> LibraryItem {
        LibraryItem {
            key: key.into(),
            live,
            desired,
        }
    }

    fn files(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn diffs_library_against_installed() {
        let items = vec![
            item("google:New", true, google("v1", &["regular"])),
            item("google:Same", true, google("v1", &["regular"])),
            item("google:Bumped", true, google("v2", &["regular"])),
            item("google:Variants", true, google("v1", &["regular", "700"])),
            item("google:Unknown", true, None),
            item("google:Gone", false, None),
            item("google:NeverHad", false, None),
            item(
                "upload:Mine",
                true,
                Some(Desired::Upload([("a".into(), "s2".into())].into())),
            ),
        ];
        let installed: InstalledMap = [
            ("google:Same".to_string(), files(&[("regular", "v1")])),
            ("google:Bumped".to_string(), files(&[("regular", "v1")])),
            ("google:Variants".to_string(), files(&[("regular", "v1")])),
            ("google:Unknown".to_string(), files(&[("regular", "v1")])),
            ("google:Gone".to_string(), files(&[("regular", "v1")])),
            ("google:LocalOnly".to_string(), files(&[("regular", "v1")])),
            ("upload:Mine".to_string(), files(&[("a", "s1")])),
        ]
        .into();
        let plan = diff(&items, &installed);
        assert_eq!(plan.install, vec!["google:New"]);
        assert_eq!(
            plan.update,
            vec!["google:Bumped", "google:Variants", "upload:Mine"]
        );
        assert_eq!(plan.remove, vec!["google:Gone"]);
    }
}
