//! Symbol im Infobereich (Tray) mit Menü – nur unter Windows.

#[cfg_attr(not(windows), allow(dead_code))]
pub enum TrayAction {
    OpenSettings,
    ToggleLock,
    ToggleHide,
    Quit,
}

#[cfg(windows)]
mod imp {
    use super::TrayAction;
    use std::cell::Cell;
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub struct Tray {
        _icon: TrayIcon,
        settings: MenuId,
        quit: MenuId,
        lock: CheckMenuItem,
        hide: CheckMenuItem,
        last: Cell<Option<(bool, bool)>>,
    }

    impl Tray {
        pub fn new(rgba: Vec<u8>, size: u32) -> Option<Self> {
            let menu = Menu::new();
            let settings = MenuItem::new("Einstellungen öffnen", true, None);
            let lock = CheckMenuItem::new("Fixiert (Strg+Umschalt+F9)", true, false, None);
            let hide = CheckMenuItem::new("Ausgeblendet (Strg+Umschalt+F10)", true, false, None);
            let quit = MenuItem::new("Beenden", true, None);
            menu.append(&settings).ok()?;
            menu.append(&PredefinedMenuItem::separator()).ok()?;
            menu.append(&lock).ok()?;
            menu.append(&hide).ok()?;
            menu.append(&PredefinedMenuItem::separator()).ok()?;
            menu.append(&quit).ok()?;

            let icon = Icon::from_rgba(rgba, size, size).ok()?;
            let tray = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("ChatGameOverlay")
                .with_icon(icon)
                .with_menu_on_left_click(false)
                .build()
                .ok()?;
            Some(Self {
                _icon: tray,
                settings: settings.id().clone(),
                quit: quit.id().clone(),
                lock,
                hide,
                last: Cell::new(None),
            })
        }

        pub fn poll(&self) -> Vec<TrayAction> {
            let mut out = Vec::new();
            while let Ok(ev) = MenuEvent::receiver().try_recv() {
                if ev.id == self.settings {
                    out.push(TrayAction::OpenSettings);
                } else if ev.id == self.quit {
                    out.push(TrayAction::Quit);
                } else if ev.id == *self.lock.id() {
                    out.push(TrayAction::ToggleLock);
                } else if ev.id == *self.hide.id() {
                    out.push(TrayAction::ToggleHide);
                }
            }
            while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                // Linksklick auf das Symbol öffnet die Einstellungen
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = ev
                {
                    out.push(TrayAction::OpenSettings);
                }
            }
            out
        }

        /// Häkchen im Menü an den aktuellen Zustand anpassen
        pub fn sync(&self, locked: bool, hidden: bool) {
            if self.last.get() != Some((locked, hidden)) {
                self.lock.set_checked(locked);
                self.hide.set_checked(hidden);
                self.last.set(Some((locked, hidden)));
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::TrayAction;

    /// Unter Linux gibt es (noch) kein Tray-Symbol.
    pub struct Tray;

    impl Tray {
        pub fn new(_rgba: Vec<u8>, _size: u32) -> Option<Self> {
            None
        }
        pub fn poll(&self) -> Vec<TrayAction> {
            Vec::new()
        }
        pub fn sync(&self, _locked: bool, _hidden: bool) {}
    }
}

pub use imp::Tray;
