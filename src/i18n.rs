//! Sprachauswahl Deutsch / Englisch.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    /// Sprache des Betriebssystems: Deutsch, wenn es Deutsch ist, sonst Englisch.
    pub fn detect() -> Self {
        match sys_locale::get_locale() {
            Some(l) if l.to_lowercase().starts_with("de") => Lang::De,
            _ => Lang::En,
        }
    }

    /// Wählt den passenden Text.
    pub fn t(self, de: &'static str, en: &'static str) -> &'static str {
        match self {
            Lang::De => de,
            Lang::En => en,
        }
    }

    pub fn hotkey_lock(self) -> &'static str {
        self.t("Strg + Umschalt + F9", "Ctrl + Shift + F9")
    }

    pub fn hotkey_hide(self) -> &'static str {
        self.t("Strg + Umschalt + F10", "Ctrl + Shift + F10")
    }

    pub fn hotkey_write(self) -> &'static str {
        self.t("Strg + Umschalt + F11", "Ctrl + Shift + F11")
    }
}

impl Default for Lang {
    fn default() -> Self {
        Self::detect()
    }
}
