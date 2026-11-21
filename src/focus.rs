//! Tastatur-Fokus nach dem Schreiben an das Spiel zurückgeben (Windows).

/// Fenster, das gerade den Fokus hat (0 = keins / nicht Windows)
#[cfg(windows)]
pub fn current() -> isize {
    // SAFETY: einfache Win32-Abfrage ohne Zeiger-Argumente
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() as isize }
}

/// Fokus an ein früher gemerktes Fenster zurückgeben
#[cfg(windows)]
pub fn restore(window: isize) {
    if window != 0 {
        // SAFETY: ein ungültiges Fenster-Handle lässt der Aufruf einfach scheitern
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(window as _);
        }
    }
}

#[cfg(not(windows))]
pub fn current() -> isize {
    0
}

#[cfg(not(windows))]
pub fn restore(_window: isize) {}

/// Fenster ganz nach oben in die Ebene „immer im Vordergrund“ holen,
/// ohne ihm den Fokus zu geben (das Spiel behält die Tastatur).
#[cfg(windows)]
pub fn bring_to_top(window: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos,
    };
    if window != 0 {
        // SAFETY: ungültige Handles lassen den Aufruf nur scheitern
        unsafe {
            SetWindowPos(
                window as _,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn bring_to_top(_window: isize) {}

/// Fenster-Handle des Hauptfensters (nur Windows, sonst 0)
#[cfg(windows)]
pub fn window_of(frame: &eframe::Frame) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match frame.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(w)) => w.hwnd.get(),
        _ => 0,
    }
}

#[cfg(not(windows))]
pub fn window_of(_frame: &eframe::Frame) -> isize {
    0
}
