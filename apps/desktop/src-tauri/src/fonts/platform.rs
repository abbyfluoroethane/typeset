//! OS-specific parts of font installation: where files go and how the OS is
//! told about them. Everything is per-user; no admin rights are needed.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Directory Typeset installs into.
///
/// macOS: `~/Library/Fonts/Typeset/`. CoreText scans ~/Library/Fonts
/// recursively, so a subfolder is activated and keeps our files apart from
/// the user's own fonts.
/// Linux: `$XDG_DATA_HOME/fonts/typeset/` (fontconfig scans recursively).
/// Windows: `%LOCALAPPDATA%\Microsoft\Windows\Fonts\`, the per-user font
/// folder, which must be flat and registered in the registry.
pub fn font_dir() -> AppResult<PathBuf> {
    let missing = || AppError::Other("cannot locate the user font directory".into());
    #[cfg(target_os = "macos")]
    {
        Ok(dirs::home_dir()
            .ok_or_else(missing)?
            .join("Library/Fonts/Typeset"))
    }
    #[cfg(windows)]
    {
        Ok(dirs::data_local_dir()
            .ok_or_else(missing)?
            .join("Microsoft")
            .join("Windows")
            .join("Fonts"))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Ok(dirs::data_dir().ok_or_else(missing)?.join("fonts/typeset"))
    }
}

/// Tells the OS about a newly written font file.
#[cfg(not(windows))]
pub fn register(_path: &Path, _family: &str, _style: &str, _otf: bool) -> AppResult<()> {
    Ok(())
}

/// Tells the OS a font file is about to be deleted or replaced.
#[cfg(not(windows))]
pub fn unregister(_path: &Path) {}

/// Runs once after a batch of installs or removals.
#[cfg(not(windows))]
pub async fn refresh(_dir: &Path) {
    #[cfg(target_os = "linux")]
    {
        // Best effort: apps pick fonts up on their next fontconfig refresh
        // anyway.
        let result = tokio::process::Command::new("fc-cache")
            .arg("-f")
            .arg(_dir)
            .output()
            .await;
        if let Err(e) = result {
            tracing::warn!("fc-cache failed: {e}");
        }
    }
}

#[cfg(windows)]
pub use win::{refresh, register, unregister};

#[cfg(windows)]
mod win {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Graphics::Gdi::{AddFontResourceW, RemoveFontResourceW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, PostMessageW, WM_FONTCHANGE,
    };
    use winreg::{RegKey, enums::*};

    use crate::error::{AppError, AppResult};

    const FONTS_KEY: &str = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    pub fn register(path: &Path, family: &str, style: &str, otf: bool) -> AppResult<()> {
        let kind = if otf { "OpenType" } else { "TrueType" };
        let name = format!("{family} {style} ({kind})");
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(FONTS_KEY)
            .map_err(AppError::Io)?;
        key.set_value(&name, &path.to_string_lossy().into_owned())
            .map_err(AppError::Io)?;
        // SAFETY: the buffer is NUL-terminated and outlives the call.
        unsafe { AddFontResourceW(wide(path).as_ptr()) };
        Ok(())
    }

    /// Removes every registry value pointing at `path`, then the font
    /// resource itself.
    pub fn unregister(path: &Path) {
        let target = path.to_string_lossy().to_lowercase();
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(FONTS_KEY, KEY_READ | KEY_WRITE)
        {
            let names: Vec<String> = key
                .enum_values()
                .filter_map(Result::ok)
                .map(|(name, _)| name)
                .filter(|name| {
                    key.get_value::<String, _>(name)
                        .map(|v| v.to_lowercase() == target)
                        .unwrap_or(false)
                })
                .collect();
            for name in names {
                let _ = key.delete_value(name);
            }
        }
        // SAFETY: the buffer is NUL-terminated and outlives the call.
        unsafe { RemoveFontResourceW(wide(path).as_ptr()) };
    }

    pub async fn refresh(_dir: &Path) {
        // SAFETY: plain broadcast with no pointers.
        unsafe { PostMessageW(HWND_BROADCAST, WM_FONTCHANGE, 0, 0) };
    }
}
