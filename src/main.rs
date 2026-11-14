// Unter Windows kein Konsolenfenster öffnen
#![cfg_attr(windows, windows_subsystem = "windows")]

mod assets;
mod auth;
mod eventsub;
mod focus;
mod i18n;
mod images;
mod irc;
mod render;
mod thirdparty;
mod tray;
mod update;
mod widgets;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontId, Rect, ResizeDirection, Sense,
    Stroke, Vec2, ViewportCommand, pos2, vec2,
};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use serde::{Deserialize, Serialize};

use auth::{Auth, AuthState};
use eventsub::EsStatus;
use i18n::Lang;
use irc::{Badge, ChatEvent, ChatMessage, Highlight, Status};
use update::UpdateState;

const APP_NAME: &str = "ChatGameOverlay";

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Config {
    channel: String,
    font_size: f32,
    /// Deckkraft des Hintergrunds, 0.0 – 1.0
    bg_opacity: f32,
    /// Nachrichten im fixierten Modus nach X Sekunden ausblenden (0 = nie)
    fade_seconds: u32,
    max_messages: usize,
    show_badges: bool,
    /// Hintergrund im fixierten Modus nur zeigen, wenn Nachrichten sichtbar sind
    auto_hide_bg: bool,
    start_locked: bool,
    /// Beim Start auf GitHub nach neuer Version suchen
    auto_update_check: bool,
    /// Sprache der Oberfläche
    language: Lang,
    /// Kanalpunkte-Einlösungen im Chat zeigen
    show_redemptions: bool,
    /// „Nachricht hervorheben“ farbig markieren
    mark_highlighted: bool,
    /// Farbe für Hervorhebungen
    highlight_color: [u8; 3],
    /// Twitch-Emotes als Bild statt als Text
    emote_images: bool,
    /// Emotes von BetterTTV, FrankerFaceZ und 7TV
    third_party_emotes: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            channel: String::new(),
            font_size: 16.0,
            bg_opacity: 0.35,
            fade_seconds: 0,
            max_messages: 150,
            show_badges: true,
            auto_hide_bg: true,
            start_locked: false,
            auto_update_check: true,
            language: Lang::detect(),
            show_redemptions: true,
            mark_highlighted: true,
            highlight_color: [145, 70, 255],
            emote_images: true,
            third_party_emotes: true,
        }
    }
}

struct OverlayApp {
    cfg: Config,
    messages: VecDeque<ChatMessage>,
    tx: Sender<ChatEvent>,
    rx: Receiver<ChatEvent>,
    stop: Arc<AtomicBool>,
    status: Status,
    connected_channel: String,
    channel_input: String,

    /// Fixiert = Mausklicks gehen durch das Fenster ins Spiel
    locked: bool,
    /// Komplett ausgeblendet (per Hotkey)
    hidden: bool,
    show_settings: bool,
    passthrough_applied: Option<bool>,

    _hotkeys: Option<GlobalHotKeyManager>,
    hk_lock: u32,
    hk_hide: u32,
    hk_write: u32,
    hotkey_error: Option<String>,

    update: update::Shared,

    tray: Option<tray::Tray>,
    /// zuletzt an das Einstellungsfenster geschickter Zustand (true = offen)
    settings_applied: Option<bool>,
    settings_cmd_at: Instant,

    /// Twitch-Anmeldung
    auth: Auth,
    /// Stand der Zugangsdaten, mit dem der Chat zuletzt verbunden wurde
    auth_gen: u64,
    /// eigene Nachrichten an den Chat-Thread
    out_tx: Sender<String>,
    /// eigener Anzeigename, Farbe und Abzeichen laut Twitch
    own: Option<(String, Option<Color32>, Vec<Badge>)>,
    /// Text in der Eingabezeile
    compose: String,
    compose_focus: bool,
    /// per Hotkey zum Schreiben geöffnet: (Fenster, das vorher den Fokus hatte, war fixiert)
    write_return: Option<(isize, bool)>,
    /// Kanalpunkte-Empfang: `None` = läuft nicht (nicht angemeldet, fremder Kanal …)
    es_status: Option<EsStatus>,
    /// Beenden angefordert (nach Update); Sicherheitsnetz, falls das Fenster nicht schließt
    quitting: Option<Instant>,

    /// geladene Emote- und Abzeichen-Bilder
    images: images::Images,
    /// Abzeichen- und Emote-Listen von Twitch
    assets: assets::Shared,
    /// Anmeldungs-Stand, für den die globalen Listen geladen wurden
    assets_gen: Option<u64>,
}

impl OverlayApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let cfg: Config = cc
            .storage
            .and_then(|s| eframe::get_value(s, "config"))
            .unwrap_or_default();

        // Globale Hotkeys – funktionieren auch, während das Spiel im Fokus ist
        let lock = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F9);
        let hide = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F10);
        let write = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F11);
        let (manager, hotkey_error) = match GlobalHotKeyManager::new() {
            Ok(m) => match m.register_all(&[lock, hide, write]) {
                Ok(()) => (Some(m), None),
                Err(e) => (Some(m), Some(e.to_string())),
            },
            Err(e) => (None, Some(e.to_string())),
        };

        let cfg_lang = cfg.language;
        // gespeichert wird `Option<Tokens>` – also genau so wieder laden
        let tokens: Option<auth::Tokens> = cc
            .storage
            .and_then(|s| eframe::get_value::<Option<auth::Tokens>>(s, "twitch"))
            .flatten();
        let auth = Auth::new(cc.egui_ctx.clone(), tokens);
        let (tx, rx) = mpsc::channel();
        let (out_tx, _) = mpsc::channel();
        let mut app = Self {
            channel_input: cfg.channel.clone(),
            locked: cfg.start_locked && !cfg.channel.trim().is_empty(),
            show_settings: cfg.channel.trim().is_empty(),
            cfg,
            messages: VecDeque::new(),
            tx,
            rx,
            stop: Arc::new(AtomicBool::new(true)),
            status: Status::Disconnected,
            connected_channel: String::new(),
            hidden: false,
            passthrough_applied: None,
            _hotkeys: manager,
            hk_lock: lock.id(),
            hk_hide: hide.id(),
            hk_write: write.id(),
            hotkey_error,
            update: Arc::new(std::sync::Mutex::new(UpdateState::Idle)),
            tray: tray::Tray::new(icon().rgba, 32, cfg_lang),
            settings_applied: None,
            settings_cmd_at: Instant::now(),
            auth_gen: auth.generation(),
            auth,
            out_tx,
            own: None,
            compose: String::new(),
            compose_focus: false,
            write_return: None,
            es_status: None,
            quitting: None,
            images: images::Images::new(cc.egui_ctx.clone()),
            assets: Default::default(),
            assets_gen: None,
        };
        if !app.cfg.channel.trim().is_empty() {
            app.connect(&cc.egui_ctx, false);
        }
        // globale Emotes von BTTV/FFZ/7TV (ohne Anmeldung)
        thirdparty::fetch_global(app.assets.clone(), cc.egui_ctx.clone());
        if app.cfg.auto_update_check {
            update::check(app.update.clone(), cc.egui_ctx.clone(), app.cfg.language);
        }
        app
    }

    /// (Neu) verbinden. `keep_messages`: bisherigen Chat stehen lassen (z. B. nach Anmeldung)
    fn connect(&mut self, ctx: &egui::Context, keep_messages: bool) {
        self.stop.store(true, Ordering::Relaxed);
        // Ereignisse der alten Verbindung verwerfen
        let (tx, rx) = mpsc::channel();
        self.tx = tx;
        self.rx = rx;
        let (out_tx, out_rx) = mpsc::channel();
        self.out_tx = out_tx;
        self.own = None;
        if let Ok(mut a) = self.assets.lock() {
            a.clear_channel();
        }
        if !keep_messages {
            self.messages.clear();
        }

        let channel = self.cfg.channel.trim().trim_start_matches('#').to_lowercase();
        self.cfg.channel = channel.clone();
        self.connected_channel = channel.clone();
        if channel.is_empty() {
            self.status = Status::Disconnected;
            return;
        }
        self.stop = Arc::new(AtomicBool::new(false));
        self.status = Status::Connecting;
        irc::spawn(
            channel.clone(),
            self.auth.credentials(),
            self.tx.clone(),
            out_rx,
            ctx.clone(),
            self.stop.clone(),
        );

        // Kanalpunkte: nur im eigenen Kanal und mit passender Berechtigung
        self.es_status = None;
        if let Some(t) = self.auth.tokens()
            && t.login.eq_ignore_ascii_case(&channel)
            && t.can_read_redemptions()
            && !t.user_id.is_empty()
        {
            self.es_status = Some(EsStatus::Connecting);
            eventsub::spawn(t.access_token, t.user_id, self.tx.clone(), ctx.clone(), self.stop.clone());
        }
    }

    fn push_message(&mut self, m: ChatMessage) {
        self.messages.push_back(m);
        while self.messages.len() > self.cfg.max_messages.max(10) {
            self.messages.pop_front();
        }
    }

    /// Neue Nachricht aus Chat oder EventSub einsortieren
    fn incoming(&mut self, mut m: ChatMessage) {
        // Emote-Namen merken, damit sie auch in eigenen Nachrichten als Bild erscheinen
        if !m.emotes.is_empty()
            && let Ok(mut a) = self.assets.lock()
        {
            let chars: Vec<char> = m.text.chars().collect();
            for e in &m.emotes {
                let name: String = chars[e.start..e.end].iter().collect();
                a.learn_emote(&name, &e.id);
            }
        }
        if m.reward_id.is_some() {
            // Belohnung mit Texteingabe: EventSub liefert sie samt Titel – dann nicht doppelt zeigen
            if self.es_status == Some(EsStatus::Active) {
                return;
            }
            if self.cfg.show_redemptions {
                m.highlight = Some(Highlight::Reward { title: String::new(), cost: 0 });
            }
        }
        if matches!(m.highlight, Some(Highlight::Reward { .. })) && !self.cfg.show_redemptions {
            if m.reward_id.is_none() {
                return; // Einlösung ohne Chattext
            }
            m.highlight = None;
        }
        self.push_message(m);
    }

    /// Beispiele, um Aussehen und Farbe zu prüfen
    fn push_preview(&mut self) {
        let l = self.cfg.language;
        let mk = |name: &str, text: &str, hl: Option<Highlight>| ChatMessage {
            id: String::new(),
            user_login: name.to_lowercase(),
            display_name: name.to_string(),
            color: irc::color_for(&name.to_lowercase()),
            text: text.to_string(),
            is_action: false,
            system: false,
            highlight: hl,
            reward_id: None,
            badges: Vec::new(),
            emotes: Vec::new(),
            emotes_from_twitch: false,
            received: Instant::now(),
        };
        let samples = [
            mk(
                "Zuschauer42",
                "",
                Some(Highlight::Reward { title: l.t("Trink was!", "Hydrate!").into(), cost: 500 }),
            ),
            mk(
                "MaxMustermann",
                l.t("Spiel bitte Darude – Sandstorm", "Please play Darude – Sandstorm"),
                Some(Highlight::Reward { title: l.t("Musikwunsch", "Song request").into(), cost: 1000 }),
            ),
            mk(
                "Lisa",
                "VoHiYo",
                Some(Highlight::Reward { title: "#auto:random_sub_emote_unlock".into(), cost: 200 }),
            ),
            mk(
                "Kevin",
                l.t("Diese Nachricht ist hervorgehoben!", "This message is highlighted!"),
                Some(Highlight::Highlighted),
            ),
            {
                let text = l.t("Hallo Kappa schöner Stream VoHiYo", "Hi Kappa nice stream VoHiYo");
                let mut m = mk("Anna", text, None);
                m.badges = vec![irc::Badge::new("moderator", "1"), irc::Badge::new("subscriber", "12")];
                let chars: Vec<char> = text.chars().collect();
                let find = |w: &str| {
                    let wc: Vec<char> = w.chars().collect();
                    chars.windows(wc.len()).position(|x| x == wc.as_slice()).unwrap_or(0)
                };
                let (k, v) = (find("Kappa"), find("VoHiYo"));
                m.emotes = vec![
                    irc::EmoteSpan { start: k, end: k + 5, id: "25".into() },
                    irc::EmoteSpan { start: v, end: v + 6, id: "81274".into() },
                ];
                m
            },
        ];
        for m in samples {
            self.incoming(m);
        }
    }

    fn push_system(&mut self, text: String) {
        self.push_message(ChatMessage {
            id: String::new(),
            user_login: String::new(),
            display_name: String::new(),
            color: Color32::GRAY,
            text,
            is_action: false,
            system: true,
            highlight: None,
            reward_id: None,
            badges: Vec::new(),
            emotes: Vec::new(),
            emotes_from_twitch: false,
            received: Instant::now(),
        });
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                ChatEvent::Message(m) => self.incoming(m),
                ChatEvent::Delete(id) => self.messages.retain(|m| m.id != id),
                ChatEvent::ClearUser(Some(user)) => self.messages.retain(|m| m.user_login != user),
                ChatEvent::ClearUser(None) => self.messages.clear(),
                ChatEvent::Status(s) => self.status = s,
                ChatEvent::OwnState { display_name, color, badges } => {
                    self.own = Some((display_name, color, badges));
                }
                ChatEvent::Notice(text) => self.push_system(text),
                ChatEvent::AuthFailed => self.auth.report_auth_failed(),
                ChatEvent::Redemptions(s) => self.es_status = Some(s),
                ChatEvent::Joined { room_id } => {
                    self.status = Status::Connected;
                    if let Ok(mut a) = self.assets.lock() {
                        a.set_room(room_id.clone());
                    }
                    // BTTV/FFZ/7TV brauchen keine Anmeldung
                    if let Some(id) = &room_id {
                        thirdparty::fetch_channel(id.clone(), self.assets.clone(), ctx.clone());
                    }
                    // Kanal-Abzeichen (z. B. Abo-Stufen) und Kanal-Emotes holen
                    if let (Some(id), Some((_, token))) = (room_id, self.auth.credentials()) {
                        assets::fetch_channel(token, id, self.assets.clone(), ctx.clone());
                    }
                }
            }
        }
        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            if ev.state != HotKeyState::Pressed {
                continue;
            }
            if ev.id == self.hk_lock {
                self.toggle_lock();
            } else if ev.id == self.hk_hide {
                self.hidden = !self.hidden;
            } else if ev.id == self.hk_write {
                self.begin_write(ctx);
            }
        }
    }

    fn logged_in(&self) -> bool {
        matches!(self.auth.state(), AuthState::LoggedIn { .. })
    }

    /// Hotkey „Schreiben“: Overlay holt sich die Tastatur, danach geht der Fokus ans Spiel zurück
    fn begin_write(&mut self, ctx: &egui::Context) {
        if !self.logged_in() || self.connected_channel.is_empty() {
            self.show_settings = true;
            return;
        }
        if self.write_return.is_none() {
            self.write_return = Some((focus::current(), self.locked));
        }
        self.hidden = false;
        self.locked = false;
        self.compose_focus = true;
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Focus);
    }

    fn end_write(&mut self) {
        if let Some((window, was_locked)) = self.write_return.take() {
            self.locked = was_locked;
            focus::restore(window);
        }
    }

    fn send_message(&mut self) {
        let l = self.cfg.language;
        let text: String = self.compose.trim().chars().take(500).collect();
        if text.is_empty() {
            return;
        }
        if self.status != Status::Connected {
            // Text stehen lassen, damit man es gleich noch einmal versuchen kann
            self.push_system(
                l.t(
                    "Nicht mit dem Chat verbunden – Nachricht wurde nicht gesendet.",
                    "Not connected to chat – message was not sent.",
                )
                .to_string(),
            );
            return;
        }
        self.compose.clear();
        let (wire, shown, is_action) = if let Some(rest) = text.strip_prefix("/me ") {
            (format!("\u{1}ACTION {rest}\u{1}"), rest.to_string(), true)
        } else if text.starts_with('/') {
            self.push_system(
                l.t(
                    "Chat-Befehle (/…) werden hier nicht unterstützt – nur /me.",
                    "Chat commands (/…) aren't supported here – only /me.",
                )
                .to_string(),
            );
            return;
        } else {
            (text.clone(), text, false)
        };
        if self.out_tx.send(wire).is_err() {
            return;
        }
        // Twitch schickt eigene Nachrichten nicht zurück – also selbst anzeigen
        let login = self.auth.credentials().map(|c| c.0).unwrap_or_default();
        let (name, color, badges) = match &self.own {
            Some((n, c, b)) => (
                if n.is_empty() { login.clone() } else { n.clone() },
                c.unwrap_or_else(|| irc::color_for(&login)),
                b.clone(),
            ),
            None => (login.clone(), irc::color_for(&login), Vec::new()),
        };
        self.push_message(ChatMessage {
            id: String::new(),
            user_login: login,
            display_name: name,
            color,
            text: shown,
            is_action,
            system: false,
            highlight: None,
            reward_id: None,
            badges,
            // Emotes erkennt die Darstellung über die bekannten Namen
            emotes: Vec::new(),
            emotes_from_twitch: false,
            received: Instant::now(),
        });
    }

    /// Sichtbarkeit einer Nachricht (0.0 – 1.0) beim Ausblenden nach Zeit
    fn message_alpha(&self, m: &ChatMessage) -> f32 {
        message_alpha(self.locked, self.cfg.fade_seconds, m)
    }

}

/// Sichtbarkeit einer Nachricht (0.0 – 1.0) beim Ausblenden nach Zeit
fn message_alpha(locked: bool, fade_seconds: u32, m: &ChatMessage) -> f32 {
    {
        if !locked || fade_seconds == 0 {
            return 1.0;
        }
        let total = fade_seconds as f32;
        let age = m.received.elapsed().as_secs_f32();
        let fade = 1.5_f32.min(total);
        if age >= total {
            0.0
        } else if age > total - fade {
            (total - age) / fade
        } else {
            1.0
        }
    }
}

impl OverlayApp {
    fn update_state(&self) -> UpdateState {
        self.update.lock().map(|g| g.clone()).unwrap_or(UpdateState::Idle)
    }

    fn restart_after_update(&mut self, ctx: &egui::Context) {
        let l = self.cfg.language;
        if update::restart() {
            // ans Hauptfenster schicken – das Einstellungsfenster würde sich nur minimieren
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Close);
            self.quitting = Some(Instant::now());
        } else if let Ok(mut g) = self.update.lock() {
            *g = UpdateState::Failed(
                l.t(
                    "Neustart fehlgeschlagen – bitte Programm selbst neu starten",
                    "Restart failed – please start the program again yourself",
                )
                .into(),
            );
        }
    }

    fn update_section(&mut self, ui: &mut egui::Ui) {
        let l = self.cfg.language;
        ui.label(egui::RichText::new(l.t("Updates:", "Updates:")).strong());
        ui.label(format!("{} {}", l.t("Installierte Version:", "Installed version:"), update::CURRENT));
        let ok = Color32::from_rgb(60, 200, 90);
        let warn = Color32::from_rgb(230, 120, 80);
        let ctx = ui.ctx().clone();
        match self.update_state() {
            UpdateState::Idle => {}
            UpdateState::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(l.t("Suche nach Updates …", "Checking for updates …"));
                });
            }
            UpdateState::UpToDate => {
                ui.colored_label(ok, l.t("✔ Du hast die neueste Version.", "✔ You have the latest version."));
            }
            UpdateState::Available { version, url } => {
                ui.colored_label(ok, l.t("Neue Version {v} verfügbar.", "New version {v} available.").replace("{v}", &version));
                ui.horizontal(|ui| {
                    if ui.button(l.t("Jetzt aktualisieren", "Update now")).clicked() {
                        update::install(self.update.clone(), ctx.clone(), version.clone(), url.clone(), l);
                    }
                    ui.hyperlink_to(l.t("Änderungen ansehen", "See changes"), update::RELEASES_PAGE);
                });
            }
            UpdateState::Downloading { version } => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(l.t("Lade Version {v} herunter …", "Downloading version {v} …").replace("{v}", &version));
                });
            }
            UpdateState::Installed { version } => {
                ui.colored_label(ok, l.t("Version {v} ist installiert.", "Version {v} is installed.").replace("{v}", &version));
                if ui.button(l.t("Jetzt neu starten", "Restart now")).clicked() {
                    self.restart_after_update(&ctx);
                }
            }
            UpdateState::Failed(e) => {
                ui.colored_label(warn, e);
            }
        }
        let busy = matches!(
            self.update_state(),
            UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installed { .. }
        );
        ui.horizontal(|ui| {
            if ui.add_enabled(!busy, egui::Button::new(l.t("Nach Updates suchen", "Check for updates"))).clicked() {
                update::check(self.update.clone(), ctx.clone(), l);
            }
        });
        ui.checkbox(&mut self.cfg.auto_update_check, l.t("Beim Start automatisch nach Updates suchen", "Check for updates on startup"));
    }

    fn toggle_lock(&mut self) {
        self.hidden = false;
        self.locked = !self.locked;
    }

    fn handle_tray(&mut self, ctx: &egui::Context) {
        let Some(tray) = &self.tray else { return };
        for action in tray.poll() {
            match action {
                tray::TrayAction::OpenSettings => self.show_settings = true,
                tray::TrayAction::ToggleLock => self.toggle_lock(),
                tray::TrayAction::ToggleHide => self.hidden = !self.hidden,
                tray::TrayAction::Quit => {
                    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Close)
                }
            }
        }
        if let Some(tray) = &self.tray {
            tray.sync(self.locked, self.hidden, self.cfg.language);
        }
    }

    /// Einstellungen in einem eigenen, normalen (nicht durchsichtigen) Fenster.
    /// Das Fenster bleibt immer bestehen und hat den Eintrag in der Taskleiste;
    /// „geschlossen“ heißt minimiert – ein Klick in der Taskleiste öffnet es wieder.
    fn settings_window(&mut self, ctx: &egui::Context) {
        let id = egui::ViewportId::from_hash_of("einstellungen");
        let builder = egui::ViewportBuilder::default()
            .with_title("ChatGameOverlay")
            .with_app_id("chat-game-overlay-settings")
            .with_inner_size([380.0, 600.0])
            .with_min_inner_size([300.0, 300.0])
            .with_always_on_top()
            .with_icon(Arc::new(icon()));

        // gewünschten Zustand an das Fenster schicken
        if self.settings_applied != Some(self.show_settings) {
            ctx.send_viewport_cmd_to(id, ViewportCommand::Minimized(!self.show_settings));
            if self.show_settings {
                ctx.send_viewport_cmd_to(id, ViewportCommand::Focus);
            }
            self.settings_applied = Some(self.show_settings);
            self.settings_cmd_at = Instant::now();
        }

        ctx.show_viewport_immediate(id, builder, |ui, _class| {
            let info = ui.ctx().input(|i| i.viewport().clone());
            if info.close_requested() {
                // Schließen-Knopf: nicht beenden, sondern in die Taskleiste
                ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
                self.show_settings = false;
            } else if self.settings_cmd_at.elapsed() > Duration::from_millis(600) {
                // vom Benutzer über die Taskleiste geöffnet oder minimiert
                if let Some(min) = info.minimized
                    && min == self.show_settings
                {
                    self.show_settings = !min;
                    self.settings_applied = Some(self.show_settings);
                }
            }
            ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().panel_fill);
            egui::Frame::new().inner_margin(12).show(ui, |ui| self.settings(ui));
        });
    }

    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let l = self.cfg.language;
        ui.horizontal_centered(|ui| {
            let (dot, status_text) = match self.status {
                Status::Connected => (Color32::from_rgb(60, 200, 90), l.t("verbunden", "connected")),
                Status::Connecting => (Color32::from_rgb(230, 180, 40), l.t("verbinde …", "connecting …")),
                Status::Disconnected => (Color32::from_rgb(200, 60, 60), l.t("getrennt", "disconnected")),
            };
            let title = if self.connected_channel.is_empty() {
                "✥ ChatGameOverlay".to_string()
            } else {
                format!("✥ #{}", self.connected_channel)
            };
            // Label ohne eigene Maus-Erkennung – Ziehen geht an die ganze Leiste durch
            ui.add(egui::Label::new(egui::RichText::new(title).strong()).selectable(false));
            ui.add(egui::Label::new(egui::RichText::new("●").color(dot)).selectable(false))
                .on_hover_text(status_text);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✖").on_hover_text(l.t("Beenden", "Quit")).clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                let update_color = Color32::from_rgb(60, 200, 90);
                match self.update_state() {
                    UpdateState::Available { version, .. } => {
                        if ui
                            .button(egui::RichText::new("⬆").color(update_color))
                            .on_hover_text(l.t("Version {v} verfügbar – in den Einstellungen installieren", "Version {v} available – install it in the settings").replace("{v}", &version))
                            .clicked()
                        {
                            self.show_settings = true;
                        }
                    }
                    #[allow(clippy::collapsible_match)]
                    UpdateState::Installed { version } => {
                        if ui
                            .button(egui::RichText::new("⟳").color(update_color))
                            .on_hover_text(l.t("Version {v} installiert – jetzt neu starten", "Version {v} installed – restart now").replace("{v}", &version))
                            .clicked()
                        {
                            self.restart_after_update(ui.ctx());
                        }
                    }
                    _ => {}
                }
                if ui
                    .button("🔒")
                    .on_hover_text(format!(
                        "{}{}",
                        l.t(
                            "Fixieren: Klicks gehen ins Spiel durch.\nZurück mit ",
                            "Lock: clicks go through to the game.\nUnlock with "
                        ),
                        l.hotkey_lock()
                    ))
                    .clicked()
                {
                    self.locked = true;
                }
                if ui
                    .selectable_label(self.show_settings, "⚙")
                    .on_hover_text(l.t("Einstellungen", "Settings"))
                    .clicked()
                {
                    self.show_settings = !self.show_settings;
                }
            });
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let l = self.cfg.language;
        egui::ScrollArea::vertical()
            .id_salt("settings")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(l.t("Einstellungen", "Settings"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.selectable_value(&mut self.cfg.language, Lang::En, "English");
                        ui.selectable_value(&mut self.cfg.language, Lang::De, "Deutsch");
                        ui.label("🌐");
                    });
                });
                ui.add_space(4.0);

                ui.label(l.t("Twitch-Kanal:", "Twitch channel:"));
                ui.horizontal(|ui| {
                    let enter = widgets::text_field(
                        ui,
                        &mut self.channel_input,
                        |e| e.hint_text(l.t("z. B. areadox", "e.g. areadox")).desired_width(160.0),
                        l,
                    )
                    .submitted;
                    if ui.button(l.t("Verbinden", "Connect")).clicked() || enter {
                        self.cfg.channel = self.channel_input.clone();
                        let ctx = ui.ctx().clone();
                        self.connect(&ctx, false);
                        self.channel_input = self.cfg.channel.clone();
                    }
                });
                ui.add_space(6.0);
                self.account_section(ui);
                ui.add_space(4.0);
                ui.separator();
                self.channel_points_section(ui);
                ui.add_space(4.0);
                ui.separator();

                ui.add(egui::Slider::new(&mut self.cfg.font_size, 10.0..=40.0).text(l.t("Schriftgröße", "Font size")));
                let mut pct = (self.cfg.bg_opacity * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").text(l.t("Hintergrund", "Background")))
                    .changed()
                {
                    self.cfg.bg_opacity = pct / 100.0;
                }
                ui.add(
                    egui::Slider::new(&mut self.cfg.fade_seconds, 0..=300)
                        .suffix(" s")
                        .text(l.t("Ausblenden nach", "Fade out after")),
                )
                .on_hover_text(l.t(
                    "Im fixierten Modus verschwinden Nachrichten nach dieser Zeit. 0 = nie.",
                    "In locked mode, messages disappear after this time. 0 = never.",
                ));
                ui.add(egui::Slider::new(&mut self.cfg.max_messages, 20..=500).text(l.t("Max. Nachrichten", "Max. messages")));
                ui.checkbox(&mut self.cfg.show_badges, l.t("Abzeichen anzeigen", "Show badges"));
                if self.cfg.show_badges && self.auth.credentials().is_none() {
                    ui.label(
                        egui::RichText::new(l.t(
                            "Die echten Twitch-Abzeichen gibt es nur mit Anmeldung – sonst einfache Symbole.",
                            "Real Twitch badges need a login – otherwise simple symbols are shown.",
                        ))
                        .weak(),
                    );
                }
                ui.checkbox(&mut self.cfg.emote_images, l.t("Emotes als Bild anzeigen", "Show emotes as images"));
                ui.add_enabled(
                    self.cfg.emote_images,
                    egui::Checkbox::new(
                        &mut self.cfg.third_party_emotes,
                        l.t("Emotes von BetterTTV, FrankerFaceZ und 7TV", "Emotes from BetterTTV, FrankerFaceZ and 7TV"),
                    ),
                );
                ui.checkbox(
                    &mut self.cfg.auto_hide_bg,
                    l.t(
                        "Hintergrund ausblenden, wenn keine Nachricht sichtbar ist",
                        "Hide background when no message is visible",
                    ),
                );
                ui.checkbox(&mut self.cfg.start_locked, l.t("Beim Start gleich fixieren", "Start locked"));

                ui.add_space(8.0);
                ui.separator();
                self.update_section(ui);
                ui.separator();
                ui.label(egui::RichText::new(l.t("Hotkeys (auch im Spiel):", "Hotkeys (work in-game too):")).strong());
                ui.label(format!("{}: {}", l.hotkey_lock(), l.t("fixieren / bearbeiten", "lock / edit")));
                ui.label(format!("{}: {}", l.hotkey_hide(), l.t("ein- / ausblenden", "show / hide")));
                ui.label(format!(
                    "{}: {}",
                    l.hotkey_write(),
                    l.t("im Chat schreiben (angemeldet)", "write in chat (logged in)")
                ));
                if let Some(e) = &self.hotkey_error {
                    ui.colored_label(
                        Color32::from_rgb(230, 120, 80),
                        format!("{} {e}", l.t("Hotkeys nicht verfügbar:", "Hotkeys unavailable:")),
                    );
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(l.t(
                        "Wichtig: Das Spiel muss im Modus „Randloses Fenster“ \
                         (Borderless / Vollbild-Fenster) laufen. Im exklusiven Vollbild \
                         kann kein Programm darüber angezeigt werden.",
                        "Important: the game must run in “borderless window” \
                         (windowed fullscreen) mode. Nothing can be shown on top of \
                         exclusive fullscreen.",
                    ))
                    .small()
                    .weak(),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(l.t("Chat leeren", "Clear chat")).clicked() {
                        self.messages.clear();
                    }
                    if ui.button(l.t("Schließen", "Close")).clicked() {
                        self.show_settings = false;
                    }
                    if ui.button(l.t("Programm beenden", "Quit program")).clicked() {
                        ui.ctx().send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Close);
                    }
                });
            });
    }

    fn channel_points_section(&mut self, ui: &mut egui::Ui) {
        let l = self.cfg.language;
        let ok = Color32::from_rgb(60, 200, 90);
        let warn = Color32::from_rgb(230, 120, 80);
        ui.label(egui::RichText::new(l.t("Kanalpunkte:", "Channel points:")).strong());
        ui.checkbox(
            &mut self.cfg.show_redemptions,
            l.t("Einlösungen im Chat anzeigen", "Show redemptions in chat"),
        );
        ui.checkbox(
            &mut self.cfg.mark_highlighted,
            l.t("„Nachricht hervorheben“ farbig markieren", "Mark “Highlight my message” in color"),
        );
        ui.horizontal(|ui| {
            ui.label(l.t("Farbe:", "Color:"));
            ui.color_edit_button_srgb(&mut self.cfg.highlight_color);
            if ui
                .button(l.t("Vorschau", "Preview"))
                .on_hover_text(l.t(
                    "Zeigt Beispiel-Einlösungen im Chatfenster",
                    "Shows sample redemptions in the chat window",
                ))
                .clicked()
            {
                self.push_preview();
            }
        });

        let weak = |ui: &mut egui::Ui, t: &str| {
            ui.label(egui::RichText::new(t).weak());
        };
        let tokens = self.auth.tokens();
        let Some(t) = tokens else {
            weak(
                ui,
                l.t(
                    "Melde dich oben mit deinem Twitch-Konto an, um alle Einlösungen in deinem Kanal zu sehen. \
                     Ohne Anmeldung erscheinen nur Belohnungen mit Texteingabe.",
                    "Log in with your Twitch account above to see all redemptions in your channel. \
                     Without logging in, only rewards with text input show up.",
                ),
            );
            return;
        };
        if !t.login.eq_ignore_ascii_case(&self.connected_channel) {
            weak(
                ui,
                &l.t(
                    "Alle Einlösungen gibt Twitch nur für deinen eigenen Kanal (#{k}) heraus. \
                     Hier erscheinen nur Belohnungen mit Texteingabe.",
                    "Twitch only provides all redemptions for your own channel (#{k}). \
                     Here, only rewards with text input show up.",
                )
                .replace("{k}", &t.login),
            );
            return;
        }
        let relogin = |ui: &mut egui::Ui, auth: &Auth| {
            ui.colored_label(
                warn,
                l.t(
                    "Für die Kanalpunkte fehlt eine Berechtigung – bitte einmal neu anmelden.",
                    "A permission for channel points is missing – please log in again once.",
                ),
            );
            if ui.button(l.t("Neu anmelden", "Log in again")).clicked() {
                auth.logout();
                auth.start_login(l);
            }
        };
        if t.user_id.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(l.t("Prüfe Anmeldung …", "Checking login …"));
            });
        } else if !t.can_read_redemptions() {
            relogin(ui, &self.auth);
        } else {
            match &self.es_status {
                Some(EsStatus::Active) => {
                    ui.colored_label(ok, l.t("✔ Einlösungen werden empfangen", "✔ Receiving redemptions"));
                }
                Some(EsStatus::MissingScope) => relogin(ui, &self.auth),
                Some(EsStatus::Error(e)) => {
                    ui.colored_label(
                        warn,
                        l.t(
                            "Verbindung für Kanalpunkte gestört – neuer Versuch läuft …",
                            "Channel points connection interrupted – retrying …",
                        ),
                    );
                    if !e.is_empty() {
                        weak(ui, e);
                    }
                }
                Some(EsStatus::Connecting) | None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(l.t("Verbinde Kanalpunkte …", "Connecting channel points …"));
                    });
                }
            }
        }
    }

    fn account_section(&mut self, ui: &mut egui::Ui) {
        let l = self.cfg.language;
        let warn = Color32::from_rgb(230, 120, 80);
        ui.label(egui::RichText::new(l.t("Twitch-Konto:", "Twitch account:")).strong());
        match self.auth.state() {
            state @ (AuthState::LoggedOut | AuthState::Error(_)) => {
                if let AuthState::Error(e) = state {
                    ui.colored_label(warn, e);
                }
                ui.label(
                    egui::RichText::new(l.t(
                        "Ohne Anmeldung liest du nur mit. Angemeldet kannst du direkt aus dem Overlay schreiben.",
                        "Without logging in you can only read. Logged in, you can write right from the overlay.",
                    ))
                    .weak(),
                );
                if auth::available() {
                    if ui.button(l.t("Mit Twitch anmelden", "Log in with Twitch")).clicked() {
                        self.auth.start_login(l);
                    }
                } else {
                    ui.label(
                        egui::RichText::new(l.t(
                            "Anmeldung ist in dieser Version noch nicht verfügbar.",
                            "Login isn't available in this version yet.",
                        ))
                        .italics(),
                    );
                }
            }
            AuthState::Starting => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(l.t("Verbinde mit Twitch …", "Contacting Twitch …"));
                });
            }
            AuthState::WaitingForUser { user_code, uri } => {
                ui.label(l.t(
                    "Bestätige die Anmeldung im Browser. Falls er sich nicht geöffnet hat:",
                    "Confirm the login in your browser. If it didn't open:",
                ));
                ui.hyperlink_to(l.t("twitch.tv/activate öffnen", "Open twitch.tv/activate"), &uri);
                ui.horizontal(|ui| {
                    ui.label(l.t("Code:", "Code:"));
                    ui.label(egui::RichText::new(&user_code).monospace().size(20.0).strong());
                    if ui
                        .small_button("📋")
                        .on_hover_text(l.t("Code kopieren", "Copy code"))
                        .clicked()
                    {
                        ui.ctx().copy_text(user_code.clone());
                    }
                });
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(l.t("Warte auf Bestätigung …", "Waiting for confirmation …"));
                });
                if ui.button(l.t("Abbrechen", "Cancel")).clicked() {
                    self.auth.cancel_login();
                }
            }
            AuthState::LoggedIn { login } => {
                let name = self
                    .own
                    .as_ref()
                    .map(|o| o.0.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or(login);
                ui.colored_label(
                    Color32::from_rgb(60, 200, 90),
                    format!("✔ {} {name}", l.t("Angemeldet als", "Logged in as")),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}",
                        l.t(
                            "Schreiben: im Bearbeiten-Modus unten im Chatfenster oder per",
                            "To write: at the bottom of the chat window in edit mode, or press",
                        ),
                        l.hotkey_write()
                    ))
                    .weak(),
                );
                if ui.button(l.t("Abmelden", "Log out")).clicked() {
                    self.auth.logout();
                }
            }
        }
    }

    /// Eingabezeile unten im Chatfenster
    fn compose_bar(&mut self, ui: &mut egui::Ui) {
        let l = self.cfg.language;
        ui.horizontal_centered(|ui| {
            let send_w = 28.0;
            let field_w = (ui.available_width() - send_w - 8.0).max(60.0);
            let out = widgets::text_field(
                ui,
                &mut self.compose,
                |e| {
                    e.hint_text(l.t("Nachricht senden …", "Send a message …"))
                        .desired_width(field_w)
                        .char_limit(500)
                        .font(FontId::proportional(15.0))
                },
                l,
            );
            if self.compose_focus {
                out.response.request_focus();
                self.compose_focus = false;
            }
            let escape = out.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
            let send_clicked = ui
                .add_sized([send_w, 22.0], egui::Button::new("➤"))
                .on_hover_text(l.t("Senden (Enter)", "Send (Enter)"))
                .clicked();
            if out.submitted || send_clicked {
                self.send_message();
                if self.write_return.is_some() {
                    self.end_write();
                } else {
                    out.response.request_focus();
                }
            } else if escape {
                self.end_write();
            }
        });
    }

    fn chat(&mut self, ui: &mut egui::Ui, edit_mode: bool) {
        let l = self.cfg.language;
        let size = self.cfg.font_size;
        let opts = render::Options {
            size,
            badges: self.cfg.show_badges,
            emotes: self.cfg.emote_images,
            third_party: self.cfg.third_party_emotes,
            lang: l,
        };
        // getrennte Felder, damit Bilder-Cache und Nachrichten gleichzeitig nutzbar sind
        let (locked, fade) = (self.locked, self.cfg.fade_seconds);
        let (show_red, mark_hl, hl_color) =
            (self.cfg.show_redemptions, self.cfg.mark_highlighted, self.cfg.highlight_color);
        let messages = &self.messages;
        let images = &mut self.images;
        let assets_lock = self.assets.clone();
        let empty_hint = if self.connected_channel.is_empty() {
            l.t("Kein Kanal eingestellt – ⚙ öffnen.", "No channel set – open ⚙.")
        } else {
            l.t("Warte auf Nachrichten …", "Waiting for messages …")
        };
        egui::ScrollArea::vertical()
            .id_salt("chat")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .scroll_bar_visibility(if edit_mode {
                egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
            } else {
                egui::scroll_area::ScrollBarVisibility::AlwaysHidden
            })
            .show(ui, |ui| {
                let width = ui.available_width();
                if messages.is_empty() {
                    if edit_mode {
                        ui.label(egui::RichText::new(empty_hint).weak());
                    }
                    return;
                }
                let Ok(assets) = assets_lock.lock() else { return };
                for m in messages {
                    let alpha = message_alpha(locked, fade, m);
                    if alpha <= 0.0 {
                        continue;
                    }
                    // Kanalpunkte: farbiger Kasten mit Balken links
                    let boxed = match &m.highlight {
                        Some(Highlight::Reward { .. }) => show_red,
                        Some(Highlight::Highlighted) => mark_hl,
                        None => false,
                    };
                    let (pad, bar) = if boxed { (6.0, 3.0) } else { (0.0, 0.0) };
                    let text_w = (width - 2.0 * pad - bar).max(40.0);
                    let atoms = render::atoms(m, &opts, &assets);
                    let (placed, content) = render::layout(ui, atoms, text_w, images);
                    let (rect, resp) = ui.allocate_exact_size(
                        vec2(width, content.y + 2.0 + 2.0 * pad),
                        if edit_mode { Sense::click() } else { Sense::hover() },
                    );
                    if !ui.is_rect_visible(rect) {
                        ui.add_space(size * 0.15);
                        continue;
                    }
                    if boxed {
                        let [r, g, b] = hl_color;
                        let col = Color32::from_rgb(r, g, b);
                        ui.painter().rect_filled(rect, CornerRadius::same(4), col.gamma_multiply(0.35 * alpha));
                        ui.painter().rect_filled(
                            Rect::from_min_size(rect.min, vec2(bar, rect.height())),
                            CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 },
                            col.gamma_multiply(alpha),
                        );
                    }
                    let origin = rect.min + vec2(pad + bar, pad);
                    if edit_mode {
                        resp.context_menu(|ui| {
                            if ui.button(l.t("Nachricht kopieren", "Copy message")).clicked() {
                                ui.ctx().copy_text(m.text.clone());
                                ui.close();
                            }
                            if !m.system {
                                if ui.button(l.t("Name kopieren", "Copy name")).clicked() {
                                    ui.ctx().copy_text(m.display_name.clone());
                                    ui.close();
                                }
                                if ui
                                    .button(l.t("Name und Nachricht kopieren", "Copy name and message"))
                                    .clicked()
                                {
                                    ui.ctx().copy_text(format!("{}: {}", m.display_name, m.text));
                                    ui.close();
                                }
                            }
                        });
                    }
                    paint_placed(ui, images, &placed, origin, alpha);
                    ui.add_space(size * 0.15);
                }
            });
    }

    fn visible_count(&self) -> usize {
        self.messages.iter().filter(|m| self.message_alpha(m) > 0.0).count()
    }
}

/// Zeichnet gesetzte Text- und Bildstücke; Text mit dunklem Schatten für Lesbarkeit
fn paint_placed(ui: &egui::Ui, images: &mut images::Images, placed: &[render::Placed], origin: egui::Pos2, alpha: f32) {
    let shadow = Color32::from_black_alpha(200);
    for p in placed {
        let pos = origin + p.offset;
        match &p.piece {
            render::Piece::Text(g) => {
                for off in [vec2(1.0, 1.0), vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(-1.0, -1.0)] {
                    ui.painter().add(
                        egui::epaint::TextShape::new(pos + off, g.clone(), shadow)
                            .with_override_text_color(shadow)
                            .with_opacity_factor(alpha),
                    );
                }
                ui.painter().add(
                    egui::epaint::TextShape::new(pos, g.clone(), Color32::WHITE).with_opacity_factor(alpha),
                );
            }
            render::Piece::Image { url } => {
                if let Some(tex) = images.get(url) {
                    ui.painter().image(
                        tex.id(),
                        Rect::from_min_size(pos, p.size),
                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE.gamma_multiply(alpha),
                    );
                }
            }
        }
    }
}

impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // vollständig durchsichtig
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "config", &self.cfg);
        eframe::set_value(storage, "twitch", &self.auth.tokens());
    }

    fn auto_save_interval(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(t) = self.quitting {
            if t.elapsed() > Duration::from_secs(3) {
                // Fenster hat sich nicht geschlossen → Programm hart beenden
                std::process::exit(0);
            }
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Close);
        }
        self.handle_events(&ctx);
        self.images.poll();
        // globale Abzeichen/Emotes einmal pro Anmeldung laden
        let gen_assets = self.auth.generation();
        if self.assets_gen != Some(gen_assets)
            && let Some((_, token)) = self.auth.credentials()
        {
            self.assets_gen = Some(gen_assets);
            assets::fetch_global(token, self.assets.clone(), ctx.clone());
        }
        // An- oder abgemeldet bzw. Token erneuert → Chat mit neuen Zugangsdaten verbinden
        let gen_now = self.auth.generation();
        if gen_now != self.auth_gen {
            self.auth_gen = gen_now;
            if !self.connected_channel.is_empty() {
                self.connect(&ctx, true);
            }
        }
        // regelmäßig neu zeichnen: Hotkeys abfragen, Ausblenden animieren
        ctx.request_repaint_after(Duration::from_millis(100));

        let passthrough = self.locked || self.hidden;
        if self.passthrough_applied != Some(passthrough) {
            ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(passthrough));
            self.passthrough_applied = Some(passthrough);
        }
        self.handle_tray(&ctx);
        self.settings_window(&ctx);
        if self.hidden {
            return;
        }

        let edit_mode = !self.locked;
        let full = ui.max_rect();
        let bar_h = if edit_mode { 30.0 } else { 0.0 };
        let chat_rect = Rect::from_min_max(pos2(full.min.x, full.min.y + bar_h), full.max);

        // Hintergrund des Chats – auch beim Bearbeiten genau so durchsichtig wie eingestellt
        let show_bg = !self.cfg.auto_hide_bg || self.visible_count() > 0;
        if show_bg && self.cfg.bg_opacity > 0.0 {
            let radius = if edit_mode {
                CornerRadius { nw: 0, ne: 0, sw: 6, se: 6 }
            } else {
                CornerRadius::same(6)
            };
            ui.painter().rect_filled(
                chat_rect,
                radius,
                Color32::from_black_alpha((self.cfg.bg_opacity * 255.0) as u8),
            );
        }

        if edit_mode {
            // Rahmen, damit die Fenstergröße auch bei 0 % Hintergrund sichtbar ist
            ui.painter().rect_stroke(
                full.shrink(0.5),
                CornerRadius::same(6),
                Stroke::new(1.0, Color32::from_rgb(145, 70, 255)),
                egui::StrokeKind::Inside,
            );
            // Obere Leiste: deckend, die ganze Fläche dient zum Verschieben
            let bar = Rect::from_min_size(full.min, vec2(full.width(), bar_h));
            ui.painter().rect_filled(
                bar,
                CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 },
                Color32::from_rgb(28, 24, 38),
            );
            let drag = ui
                .interact(bar, ui.scope_id().with("drag_bar"), Sense::click_and_drag())
                .on_hover_cursor(CursorIcon::Grab);
            if drag.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(vec2(8.0, 0.0))));
            self.title_bar(&mut bar_ui);
        }

        // Eingabezeile, wenn angemeldet und im Bearbeiten-Modus
        let show_compose = edit_mode && self.logged_in() && !self.connected_channel.is_empty();
        let compose_h = if show_compose { 34.0 } else { 0.0 };
        let messages_rect =
            Rect::from_min_max(chat_rect.min, pos2(chat_rect.max.x, chat_rect.max.y - compose_h));
        let mut content = ui.new_child(egui::UiBuilder::new().max_rect(messages_rect.shrink(8.0)));
        self.chat(&mut content, edit_mode);
        if show_compose {
            let bar = Rect::from_min_max(pos2(chat_rect.min.x, chat_rect.max.y - compose_h), chat_rect.max);
            ui.painter().rect_filled(
                bar,
                CornerRadius { nw: 0, ne: 0, sw: 6, se: 6 },
                Color32::from_rgb(28, 24, 38),
            );
            // rechts Platz für den Größen-Griff lassen
            let inner = Rect::from_min_max(bar.min + vec2(6.0, 0.0), bar.max - vec2(20.0, 0.0));
            let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            self.compose_bar(&mut bar_ui);
        }

        // Größe ändern: Griff unten rechts
        if edit_mode {
            let grip = Rect::from_min_size(full.max - Vec2::splat(18.0), Vec2::splat(18.0));
            let resp = ui
                .interact(grip, ui.scope_id().with("resize_grip"), Sense::drag())
                .on_hover_cursor(CursorIcon::ResizeSouthEast);
            if resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(ResizeDirection::SouthEast));
            }
            let p = ui.painter();
            let c = Color32::from_rgb(145, 70, 255);
            for i in 0..3 {
                let d = 4.0 + i as f32 * 4.0;
                p.line_segment(
                    [pos2(full.max.x - 3.0, full.max.y - 3.0 - d), pos2(full.max.x - 3.0 - d, full.max.y - 3.0)],
                    Stroke::new(1.5, c),
                );
            }
        }
    }
}

fn icon() -> egui::IconData {
    // kleines Sprechblasen-Symbol in Twitch-Lila, ohne Bilddatei
    let s = 32usize;
    let mut rgba = vec![0u8; s * s * 4];
    for y in 0..s {
        for x in 0..s {
            let in_body = (3..29).contains(&x) && (4..22).contains(&y);
            let in_tail = (8..14).contains(&x) && (22..28).contains(&y) && x + 22 >= y + 8;
            let in_line = (8..24).contains(&x) && (y == 10 || y == 15) && in_body;
            let i = (y * s + x) * 4;
            if in_line {
                rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            } else if in_body || in_tail {
                rgba[i..i + 4].copy_from_slice(&[145, 70, 255, 255]);
            }
        }
    }
    egui::IconData { rgba, width: s as u32, height: s as u32 }
}

fn main() -> eframe::Result {
    // Nach einem Update warten, bis die alte Instanz beendet ist
    let args: Vec<String> = std::env::args().collect();
    update::wait_for_old_instance(&args);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("chat-game-overlay")
            .with_inner_size([380.0, 520.0])
            .with_min_inner_size([200.0, 120.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            // in der Taskleiste steht nur das Einstellungsfenster
            .with_taskbar(false)
            .with_icon(Arc::new(icon())),
        persist_window: true,
        // OpenGL-Renderer: unter Windows zuverlässig transparent (wgpu/DirectX nicht)
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(OverlayApp::new(cc)))),
    )
}
