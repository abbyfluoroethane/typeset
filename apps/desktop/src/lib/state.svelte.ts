import { notify } from "$lib/notify";
import * as api from "$lib/api";
import type {
  CatalogFamily,
  Collection,
  InstalledFamily,
  LibraryEntry,
  LocalState,
  ReconcileReport,
  Settings,
  User,
} from "$lib/api";

export type View = "browse" | "library" | "collections" | "uploads" | "settings";

const SYNC_INTERVAL_MS = 10 * 60 * 1000;

const emptyLocal = (): LocalState => ({
  library: [],
  collections: [],
  uploads: [],
  lastRev: 0,
  lastSyncedAt: null,
});

const plural = (n: number, noun: string) => `${n} ${noun}${n === 1 ? "" : "s"}`;

/** Human summary of what a sync changed on this computer, or "" if nothing. */
function describeChanges(r: ReconcileReport): string {
  const parts = [
    r.installed.length && `${plural(r.installed.length, "font")} installed`,
    r.updated.length && `${plural(r.updated.length, "font")} updated`,
    r.removed.length && `${plural(r.removed.length, "font")} removed`,
  ].filter(Boolean);
  return parts.join(", ");
}

/** In `pnpm dev`, `?view=library` and `?detail=google:Inter` open a screen directly. */
const preview = import.meta.env.DEV ? new URLSearchParams(globalThis.location?.search) : null;

class AppState {
  booted = $state(false);
  view = $state<View>((preview?.get("view") as View | null) ?? "browse");
  settings = $state<Settings>({ serverUrl: null, autoInstall: true });
  user = $state<User | null>(null);
  catalog = $state.raw<CatalogFamily[]>([]);
  catalogFetchedAt = $state<number | null>(null);
  catalogLoading = $state(false);
  catalogError = $state<string | null>(null);
  local = $state.raw<LocalState>(emptyLocal());
  installed = $state.raw<InstalledFamily[]>([]);
  syncing = $state(false);
  /** Message from the last failed sync; null after a successful one. */
  lastSyncError = $state<string | null>(null);
  signingIn = $state(false);
  signInError = $state<string | null>(null);
  previewText = $state("");
  /** Family currently shown in the detail dialog. */
  detailKey = $state<string | null>(preview?.get("detail") ?? null);
  /** Keys with an install/uninstall in flight. */
  busyKeys = $state<string[]>([]);

  #timer: ReturnType<typeof setInterval> | undefined;

  library = $derived(this.local.library);
  collections = $derived(this.local.collections);
  uploads = $derived(this.local.uploads);
  libraryByKey = $derived(new Map(this.local.library.map((e) => [e.key, e])));
  installedByKey = $derived(new Map(this.installed.map((f) => [f.key, f])));
  catalogByName = $derived(new Map(this.catalog.map((f) => [f.family, f])));
  signedIn = $derived(this.user !== null);

  // ---- helpers -----------------------------------------------------------

  fail(e: unknown, prefix?: string) {
    const msg = api.errorMessage(e);
    notify.error(prefix ? `${prefix}: ${msg}` : msg);
  }

  async #try<T>(prefix: string, fn: () => Promise<T>): Promise<T | undefined> {
    try {
      return await fn();
    } catch (e) {
      this.fail(e, prefix);
      return undefined;
    }
  }

  #setLocal(patch: Partial<LocalState>) {
    this.local = { ...this.local, ...patch };
  }

  #setBusy(key: string, on: boolean) {
    this.busyKeys = on ? [...this.busyKeys, key] : this.busyKeys.filter((k) => k !== key);
  }

  // ---- startup -----------------------------------------------------------

  async boot() {
    try {
      this.settings = await api.getSettings();
    } catch (e) {
      this.fail(e, "Could not load settings");
    }
    try {
      this.user = await api.currentUser();
    } catch (e) {
      this.fail(e, "Could not load account");
    }
    this.booted = true;
    await Promise.all([this.refreshLocal(), this.refreshInstalled(), this.loadCatalog(false)]);
    this.#startSyncLoop();
  }

  #startSyncLoop() {
    clearInterval(this.#timer);
    if (!this.user) return;
    void this.sync({ quiet: true });
    this.#timer = setInterval(() => void this.sync({ quiet: true }), SYNC_INTERVAL_MS);
  }

  stop() {
    clearInterval(this.#timer);
  }

  // ---- auth & settings ---------------------------------------------------

  async saveServerUrl(url: string): Promise<boolean> {
    const serverUrl = url.trim().replace(/\/+$/, "") || null;
    const saved = await this.#try("Could not save settings", () =>
      api.updateSettings({ ...this.settings, serverUrl }),
    );
    if (saved) this.settings = saved;
    return !!saved;
  }

  async signIn() {
    this.signingIn = true;
    this.signInError = null;
    try {
      this.user = await api.login();
      notify.success("Signed in");
      await this.refreshLocal();
      this.#startSyncLoop();
    } catch (e) {
      this.signInError = api.errorMessage(e);
    } finally {
      this.signingIn = false;
    }
  }

  /** Dismiss the waiting state. The browser flow itself cannot be aborted from here. */
  cancelSignIn() {
    this.signingIn = false;
  }

  async signOut() {
    try {
      await api.logout();
    } catch (e) {
      this.fail(e, "Sign out failed");
      return;
    }
    clearInterval(this.#timer);
    this.user = null;
    this.lastSyncError = null;
    this.local = emptyLocal();
    await this.refreshInstalled();
  }

  async setAutoInstall(autoInstall: boolean) {
    const saved = await this.#try("Could not save settings", () =>
      api.updateSettings({ ...this.settings, autoInstall }),
    );
    if (saved) this.settings = saved;
  }

  // ---- loading -----------------------------------------------------------

  async loadCatalog(refresh: boolean) {
    this.catalogLoading = true;
    this.catalogError = null;
    try {
      const res = await api.getCatalog(refresh);
      this.catalog = res.families;
      this.catalogFetchedAt = res.fetchedAt;
    } catch (e) {
      this.catalogError = api.errorMessage(e);
    } finally {
      this.catalogLoading = false;
    }
  }

  async refreshLocal() {
    try {
      this.local = await api.getLocalState();
    } catch {
      // Not available until signed in / backend ready; keep the current mirror.
    }
  }

  async refreshInstalled() {
    try {
      this.installed = await api.listInstalled();
    } catch {
      // ignore: installed list is best-effort
    }
  }

  /**
   * Background syncs (`quiet`) only speak up when something changed on this
   * computer or something failed; manual syncs always confirm briefly.
   */
  async sync(opts: { quiet?: boolean } = {}) {
    if (this.syncing || !this.user) return;
    const quiet = !!opts.quiet;
    const retry = { Retry: () => void this.sync() };
    this.syncing = true;
    try {
      const { state, reconcile } = await api.syncNow();
      this.local = state;
      await this.refreshInstalled();
      const failed = reconcile?.failed ?? [];
      const changes = reconcile ? describeChanges(reconcile) : "";
      if (failed.length > 0) {
        this.lastSyncError = `${plural(failed.length, "font")} could not be installed`;
        const detail = failed.map((f) => `${f.key}: ${f.error}`).join("; ");
        notify.warning(`${this.lastSyncError}. ${detail}`, retry);
      } else {
        this.lastSyncError = null;
        if (changes) notify.success(quiet ? changes : `Synced · ${changes}`);
        else if (!quiet) notify.success("Up to date");
      }
    } catch (e) {
      const msg = api.errorMessage(e);
      const isNew = msg !== this.lastSyncError;
      this.lastSyncError = msg;
      // Don't repeat the same background failure every interval.
      if (!quiet || isNew) notify.error(`Sync failed: ${msg}`, retry);
    } finally {
      this.syncing = false;
    }
  }

  // ---- library -----------------------------------------------------------

  #upsertEntry(entry: LibraryEntry) {
    const rest = this.local.library.filter((e) => e.key !== entry.key);
    this.#setLocal({ library: entry.deleted ? rest : [...rest, entry] });
  }

  async putLibrary(key: string, variants: string[], favorite: boolean) {
    const entry = await this.#try("Could not update library", () =>
      api.libraryPut(key, { variants, favorite }),
    );
    if (entry) this.#upsertEntry(entry);
    return entry;
  }

  async addToLibrary(key: string, variants: string[] = []) {
    const e = await this.putLibrary(key, variants, this.libraryByKey.get(key)?.favorite ?? false);
    if (e) notify.success("Added to library");
  }

  async removeFromLibrary(key: string) {
    const entry = await this.#try("Could not update library", () => api.libraryDelete(key));
    if (entry) this.#setLocal({ library: this.local.library.filter((e) => e.key !== key) });
  }

  async toggleFavorite(key: string) {
    const cur = this.libraryByKey.get(key);
    await this.putLibrary(key, cur?.variants ?? [], !(cur?.favorite ?? false));
  }

  // ---- install -----------------------------------------------------------

  async install(key: string) {
    this.#setBusy(key, true);
    const res = await this.#try("Install failed", () => api.installFamily(key));
    this.#setBusy(key, false);
    if (res) {
      this.installed = [...this.installed.filter((f) => f.key !== key), res];
      notify.success("Installed on this computer");
    }
  }

  async uninstall(key: string) {
    this.#setBusy(key, true);
    try {
      await api.uninstallFamily(key);
      this.installed = this.installed.filter((f) => f.key !== key);
    } catch (e) {
      this.fail(e, "Uninstall failed");
    } finally {
      this.#setBusy(key, false);
    }
  }

  // ---- collections -------------------------------------------------------

  #upsertCollection(c: Collection) {
    const rest = this.local.collections.filter((x) => x.id !== c.id);
    this.#setLocal({ collections: c.deleted ? rest : [...rest, c] });
  }

  async createCollection(name: string, familyKeys: string[] = []) {
    const c = await this.#try("Could not create collection", () =>
      api.collectionPut(crypto.randomUUID(), { name, familyKeys }),
    );
    if (c) this.#upsertCollection(c);
    return c;
  }

  async renameCollection(c: Collection, name: string) {
    const next = await this.#try("Could not rename collection", () =>
      api.collectionPut(c.id, { name, familyKeys: c.familyKeys }),
    );
    if (next) this.#upsertCollection(next);
  }

  async deleteCollection(id: string) {
    const c = await this.#try("Could not delete collection", () => api.collectionDelete(id));
    if (c) this.#setLocal({ collections: this.local.collections.filter((x) => x.id !== id) });
  }

  async toggleInCollection(c: Collection, key: string) {
    const familyKeys = c.familyKeys.includes(key)
      ? c.familyKeys.filter((k) => k !== key)
      : [...c.familyKeys, key];
    const next = await this.#try("Could not update collection", () =>
      api.collectionPut(c.id, { name: c.name, familyKeys }),
    );
    if (next) this.#upsertCollection(next);
  }

  // ---- uploads -----------------------------------------------------------

  async uploadFonts(paths: string[]) {
    if (paths.length === 0) return;
    const ups = await this.#try("Upload failed", () => api.uploadFonts(paths));
    if (ups) {
      notify.success(`Uploaded ${ups.length} font file${ups.length === 1 ? "" : "s"}`);
      await this.refreshLocal();
      await this.refreshInstalled();
    }
  }

  async deleteUpload(id: string) {
    const u = await this.#try("Could not delete upload", () => api.deleteUpload(id));
    if (u) {
      this.#setLocal({ uploads: this.local.uploads.filter((x) => x.id !== id) });
      await this.refreshInstalled();
    }
  }

  openDetail(key: string) {
    this.detailKey = key;
  }
}

export const app = new AppState();
