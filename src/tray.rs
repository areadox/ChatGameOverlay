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
    use crate::i18n::Lang;
    use std::cell::Cell;
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub struct Tray {
        _icon: TrayIcon,
        settings_item: MenuItem,
        quit_item: MenuItem,
        settings: MenuId,
        quit: MenuId,
        lock: CheckMenuItem,
        hide: CheckMenuItem,
        last: Cell<Option<(bool, bool, Lang)>>,
    }

    impl Tray {
        pub fn new(rgba: Vec<u8>, size: u32, lang: Lang) -> Option<Self> {
            let menu = Menu::new();
            let (t_settings, t_lock, t_hide, t_quit) = texts(lang);
            let settings = MenuItem::new(t_settings, true, None);
            let lock = CheckMenuItem::new(t_lock, true, false, None);
            let hide = CheckMenuItem::new(t_hide, true, false, None);
            let quit = MenuItem::new(t_quit, true, None);
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
                settings_item: settings,
                quit_item: quit,
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

        /// Häkchen und Sprache im Menü an den aktuellen Zustand anpassen
        pub fn sync(&self, locked: bool, hidden: bool, lang: Lang) {
            let prev = self.last.get();
            if prev == Some((locked, hidden, lang)) {
                return;
            }
            if prev.map(|p| p.2) != Some(lang) {
                let (t_settings, t_lock, t_hide, t_quit) = texts(lang);
                self.settings_item.set_text(t_settings);
                self.lock.set_text(t_lock);
                self.hide.set_text(t_hide);
                self.quit_item.set_text(t_quit);
            }
            self.lock.set_checked(locked);
            self.hide.set_checked(hidden);
            self.last.set(Some((locked, hidden, lang)));
        }
    }

    fn texts(l: Lang) -> (&'static str, &'static str, &'static str, &'static str) {
        (
            l.t("Einstellungen öffnen", "Open settings"),
            l.t("Fixiert (Strg+Umschalt+F9)", "Locked (Ctrl+Shift+F9)"),
            l.t("Ausgeblendet (Strg+Umschalt+F10)", "Hidden (Ctrl+Shift+F10)"),
            l.t("Beenden", "Quit"),
        )
    }
}

#[cfg(not(windows))]
mod imp {
    use super::TrayAction;

    /// Unter Linux gibt es (noch) kein Tray-Symbol.
    pub struct Tray;

    impl Tray {
        pub fn new(_rgba: Vec<u8>, _size: u32, _lang: crate::i18n::Lang) -> Option<Self> {
            None
        }
        pub fn poll(&self) -> Vec<TrayAction> {
            Vec::new()
        }
        pub fn sync(&self, _locked: bool, _hidden: bool, _lang: crate::i18n::Lang) {}
    }
}

pub use imp::Tray;
