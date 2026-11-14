//! Update-Funktion: prüft das neueste GitHub-Release und ersetzt die eigene Programmdatei.

use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::Context;
use serde::Deserialize;

use crate::i18n::Lang;

const LATEST_URL: &str = "https://api.github.com/repos/areadox/ChatGameOverlay/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/areadox/ChatGameOverlay/releases";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[cfg(windows)]
const ASSET_NAME: &str = "ChatGameOverlay-windows.exe";
#[cfg(not(windows))]
const ASSET_NAME: &str = "ChatGameOverlay-linux";

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available { version: String, url: String },
    Downloading { version: String },
    /// Neue Version ist installiert – Programm muss neu gestartet werden
    Installed { version: String },
    Failed(String),
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

pub type Shared = Arc<Mutex<UpdateState>>;

fn user_agent() -> String {
    format!("ChatGameOverlay/{CURRENT}")
}

fn set(state: &Shared, ctx: &Context, s: UpdateState) {
    if let Ok(mut g) = state.lock() {
        *g = s;
    }
    ctx.request_repaint();
}

/// Fragt GitHub im Hintergrund nach dem neuesten Release.
pub fn check(state: Shared, ctx: Context, l: Lang) {
    set(&state, &ctx, UpdateState::Checking);
    thread::spawn(move || {
        let result = (|| -> Result<UpdateState, String> {
            let release: Release = ureq::get(LATEST_URL)
                .header("User-Agent", &user_agent())
                .header("Accept", "application/vnd.github+json")
                .call()
                .map_err(|e| format!("{} {e}", l.t("GitHub nicht erreichbar:", "Cannot reach GitHub:")))?
                .body_mut()
                .read_json()
                .map_err(|e| format!("{} {e}", l.t("Antwort unlesbar:", "Unreadable response:")))?;
            let version = release.tag_name.trim_start_matches('v').to_string();
            if !is_newer(&version, CURRENT) {
                return Ok(UpdateState::UpToDate);
            }
            let asset = release
                .assets
                .into_iter()
                .find(|a| a.name == ASSET_NAME)
                .ok_or_else(|| l.t("Version {v} hat (noch) keine Datei {f}", "Version {v} has no file {f} (yet)")
                    .replace("{v}", &version)
                    .replace("{f}", ASSET_NAME))?;
            Ok(UpdateState::Available { version, url: asset.browser_download_url })
        })();
        set(&state, &ctx, result.unwrap_or_else(UpdateState::Failed));
    });
}

/// Lädt die neue Version herunter und ersetzt die laufende Programmdatei.
pub fn install(state: Shared, ctx: Context, version: String, url: String, l: Lang) {
    set(&state, &ctx, UpdateState::Downloading { version: version.clone() });
    thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let data = ureq::get(&url)
                .header("User-Agent", &user_agent())
                .call()
                .map_err(|e| format!("{} {e}", l.t("Download fehlgeschlagen:", "Download failed:")))?
                .body_mut()
                .with_config()
                .limit(200 * 1024 * 1024)
                .read_to_vec()
                .map_err(|e| format!("{} {e}", l.t("Download abgebrochen:", "Download aborted:")))?;
            if data.len() < 100 * 1024 {
                return Err(l.t("Heruntergeladene Datei ist unvollständig", "Downloaded file is incomplete").into());
            }
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let dir = exe.parent().ok_or(l.t("Programmordner nicht gefunden", "Program folder not found"))?;
            let tmp = dir.join(".ChatGameOverlay-update.tmp");
            std::fs::write(&tmp, &data)
                .map_err(|e| format!("{} {e}", l.t("Kann nicht in den Programmordner schreiben:", "Cannot write to the program folder:")))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
            }
            let r = self_replace::self_replace(&tmp).map_err(|e| format!("{} {e}", l.t("Ersetzen fehlgeschlagen:", "Replacing failed:")));
            let _ = std::fs::remove_file(&tmp);
            r
        })();
        set(
            &state,
            &ctx,
            match result {
                Ok(()) => UpdateState::Installed { version },
                Err(e) => UpdateState::Failed(e),
            },
        );
    });
}

/// Startet das (neue) Programm und gibt `true` zurück, wenn das geklappt hat.
/// Die neue Instanz bekommt unsere Prozess-ID und wartet, bis wir beendet sind.
pub fn restart() -> bool {
    std::env::current_exe()
        .and_then(|exe| {
            std::process::Command::new(exe)
                .arg("--nach-update")
                .arg(std::process::id().to_string())
                .spawn()
        })
        .is_ok()
}

/// Neue Instanz nach einem Update: warten, bis die alte beendet ist
/// (gibt Hotkeys und Tray frei und hat ihre Einstellungen gespeichert).
pub fn wait_for_old_instance(args: &[String]) {
    let Some(pos) = args.iter().position(|a| a == "--nach-update") else {
        return;
    };
    match args.get(pos + 1).and_then(|p| p.parse::<u32>().ok()) {
        Some(pid) => wait_for_exit(pid, 8000),
        // ältere Versionen übergeben keine Prozess-ID
        None => std::thread::sleep(std::time::Duration::from_millis(1500)),
    }
}

#[cfg(windows)]
fn wait_for_exit(pid: u32, timeout_ms: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
    // SAFETY: Handle wird nur geöffnet, abgewartet und wieder geschlossen
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !handle.is_null() {
            WaitForSingleObject(handle, timeout_ms);
            CloseHandle(handle);
        }
    }
}

#[cfg(not(windows))]
fn wait_for_exit(pid: u32, timeout_ms: u32) {
    let start = std::time::Instant::now();
    while std::path::Path::new(&format!("/proc/{pid}")).exists()
        && start.elapsed().as_millis() < timeout_ms as u128
    {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Vergleicht Versionen wie „0.0.10“ und „0.0.9“ zahlenweise.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split(['.', '-', '+'])
            .take(3)
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parse(candidate) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn version_compare() {
        assert!(is_newer("0.0.3", "0.0.2"));
        assert!(is_newer("0.0.10", "0.0.9"));
        assert!(is_newer("0.1.0", "0.0.99"));
        assert!(!is_newer("0.0.2", "0.0.2"));
        assert!(!is_newer("0.0.1", "0.0.2"));
    }
}
