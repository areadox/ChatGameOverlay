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
