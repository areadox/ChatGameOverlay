// Unter Windows kein Konsolenfenster öffnen
#![cfg_attr(windows, windows_subsystem = "windows")]

mod irc;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontId, Rect, ResizeDirection, Sense,
    Stroke, TextFormat, Vec2, ViewportCommand, pos2, text::LayoutJob, vec2,
};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use serde::{Deserialize, Serialize};

use irc::{Badge, ChatEvent, ChatMessage, Status};

const APP_NAME: &str = "ChatGameOverlay";
const HOTKEY_LOCK_TEXT: &str = "Strg + Umschalt + F9";
const HOTKEY_HIDE_TEXT: &str = "Strg + Umschalt + F10";

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
    hotkey_error: Option<String>,
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
        let (manager, hotkey_error) = match GlobalHotKeyManager::new() {
            Ok(m) => match m.register_all(&[lock, hide]) {
                Ok(()) => (Some(m), None),
                Err(e) => (Some(m), Some(format!("Hotkeys nicht verfügbar: {e}"))),
            },
            Err(e) => (None, Some(format!("Hotkeys nicht verfügbar: {e}"))),
        };

        let (tx, rx) = mpsc::channel();
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
            hotkey_error,
        };
        if !app.cfg.channel.trim().is_empty() {
            app.connect(&cc.egui_ctx);
        }
        app
    }

    fn connect(&mut self, ctx: &egui::Context) {
        self.stop.store(true, Ordering::Relaxed);
        // Ereignisse der alten Verbindung verwerfen
        let (tx, rx) = mpsc::channel();
        self.tx = tx;
        self.rx = rx;
        self.messages.clear();

        let channel = self.cfg.channel.trim().trim_start_matches('#').to_lowercase();
        self.cfg.channel = channel.clone();
        self.connected_channel = channel.clone();
        if channel.is_empty() {
            self.status = Status::Disconnected;
            return;
        }
        self.stop = Arc::new(AtomicBool::new(false));
        self.status = Status::Connecting;
        irc::spawn(channel, self.tx.clone(), ctx.clone(), self.stop.clone());
    }

    fn handle_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                ChatEvent::Message(m) => {
                    self.messages.push_back(m);
                    while self.messages.len() > self.cfg.max_messages.max(10) {
                        self.messages.pop_front();
                    }
                }
                ChatEvent::Delete(id) => self.messages.retain(|m| m.id != id),
                ChatEvent::ClearUser(Some(user)) => self.messages.retain(|m| m.user_login != user),
                ChatEvent::ClearUser(None) => self.messages.clear(),
                ChatEvent::Status(s) => self.status = s,
            }
        }
        while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
            if ev.state != HotKeyState::Pressed {
                continue;
            }
            if ev.id == self.hk_lock {
                self.hidden = false;
                self.locked = !self.locked;
                if self.locked {
                    self.show_settings = false;
                }
            } else if ev.id == self.hk_hide {
                self.hidden = !self.hidden;
            }
        }
    }

    /// Sichtbarkeit einer Nachricht (0.0 – 1.0) beim Ausblenden nach Zeit
    fn message_alpha(&self, m: &ChatMessage) -> f32 {
        if !self.locked || self.cfg.fade_seconds == 0 {
            return 1.0;
        }
        let total = self.cfg.fade_seconds as f32;
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

    fn title_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (dot, status_text) = match self.status {
                Status::Connected => (Color32::from_rgb(60, 200, 90), "verbunden"),
                Status::Connecting => (Color32::from_rgb(230, 180, 40), "verbinde …"),
                Status::Disconnected => (Color32::from_rgb(200, 60, 60), "getrennt"),
            };
            let title = if self.connected_channel.is_empty() {
                "✥ ChatGameOverlay".to_string()
            } else {
                format!("✥ #{}", self.connected_channel)
            };
            let drag = ui
                .add(egui::Label::new(egui::RichText::new(title).strong()).sense(Sense::drag()))
                .on_hover_cursor(CursorIcon::Grab)
                .on_hover_text("Hier ziehen, um das Fenster zu verschieben");
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
            }
            ui.colored_label(dot, "●").on_hover_text(status_text);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✖").on_hover_text("Beenden").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                if ui
                    .button("🔒")
                    .on_hover_text(format!(
                        "Fixieren: Klicks gehen ins Spiel durch.\nZurück mit {HOTKEY_LOCK_TEXT}"
                    ))
                    .clicked()
                {
                    self.locked = true;
                    self.show_settings = false;
                }
                if ui
                    .selectable_label(self.show_settings, "⚙")
                    .on_hover_text("Einstellungen")
                    .clicked()
                {
                    self.show_settings = !self.show_settings;
                }
            });
        });
        ui.separator();
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("settings")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Einstellungen");
                ui.add_space(4.0);

                ui.label("Twitch-Kanal:");
                ui.horizontal(|ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.channel_input)
                            .hint_text("z. B. areadox")
                            .desired_width(160.0),
                    );
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Verbinden").clicked() || enter {
                        self.cfg.channel = self.channel_input.clone();
                        let ctx = ui.ctx().clone();
                        self.connect(&ctx);
                        self.channel_input = self.cfg.channel.clone();
                    }
                });
                ui.add_space(6.0);

                ui.add(egui::Slider::new(&mut self.cfg.font_size, 10.0..=40.0).text("Schriftgröße"));
                let mut pct = (self.cfg.bg_opacity * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").text("Hintergrund"))
                    .changed()
                {
                    self.cfg.bg_opacity = pct / 100.0;
                }
                ui.add(
                    egui::Slider::new(&mut self.cfg.fade_seconds, 0..=300)
                        .suffix(" s")
                        .text("Ausblenden nach"),
                )
                .on_hover_text("Im fixierten Modus verschwinden Nachrichten nach dieser Zeit. 0 = nie.");
                ui.add(egui::Slider::new(&mut self.cfg.max_messages, 20..=500).text("Max. Nachrichten"));
                ui.checkbox(&mut self.cfg.show_badges, "Abzeichen (Mod, VIP, Sub) anzeigen");
                ui.checkbox(
                    &mut self.cfg.auto_hide_bg,
                    "Hintergrund ausblenden, wenn keine Nachricht sichtbar ist",
                );
                ui.checkbox(&mut self.cfg.start_locked, "Beim Start gleich fixieren");

                ui.add_space(8.0);
                ui.separator();
                ui.label(egui::RichText::new("Hotkeys (auch im Spiel):").strong());
                ui.label(format!("{HOTKEY_LOCK_TEXT}: fixieren / bearbeiten"));
                ui.label(format!("{HOTKEY_HIDE_TEXT}: ein- / ausblenden"));
                if let Some(e) = &self.hotkey_error {
                    ui.colored_label(Color32::from_rgb(230, 120, 80), e);
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Wichtig: Das Spiel muss im Modus „Randloses Fenster“ \
                         (Borderless / Vollbild-Fenster) laufen. Im exklusiven Vollbild \
                         kann kein Programm darüber angezeigt werden.",
                    )
                    .small()
                    .weak(),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Chat leeren").clicked() {
                        self.messages.clear();
                    }
                    if ui.button("Fertig").clicked() {
                        self.show_settings = false;
                    }
                });
            });
    }

    fn chat(&self, ui: &mut egui::Ui, edit_mode: bool) {
        let size = self.cfg.font_size;
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
                // Nachrichten unten ausrichten wie im echten Chat
                let width = ui.available_width();
                if self.messages.is_empty() {
                    let hint = if self.connected_channel.is_empty() {
                        "Kein Kanal eingestellt – ⚙ öffnen."
                    } else {
                        "Warte auf Nachrichten …"
                    };
                    if edit_mode {
                        ui.label(egui::RichText::new(hint).weak());
                    }
                    return;
                }
                for m in &self.messages {
                    let alpha = self.message_alpha(m);
                    if alpha <= 0.0 {
                        continue;
                    }
                    let job = build_job(m, size, width, alpha, self.cfg.show_badges);
                    let galley = ui.painter().layout_job(job);
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(width, galley.size().y + 2.0),
                        Sense::hover(),
                    );
                    // dunkler Schatten für Lesbarkeit auf hellem Spielhintergrund
                    let shadow = Color32::from_black_alpha((200.0 * alpha) as u8);
                    for off in [vec2(1.0, 1.0), vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(-1.0, -1.0)] {
                        ui.painter().galley_with_override_text_color(
                            rect.min + off,
                            galley.clone(),
                            shadow,
                        );
                    }
                    ui.painter().galley(rect.min, galley, Color32::WHITE);
                    ui.add_space(size * 0.15);
                }
            });
    }

    fn visible_count(&self) -> usize {
        self.messages.iter().filter(|m| self.message_alpha(m) > 0.0).count()
    }
}

/// Baut eine Chatzeile: [Abzeichen] Name: Text
fn build_job(m: &ChatMessage, size: f32, width: f32, alpha: f32, badges: bool) -> LayoutJob {
    let a = |c: Color32| c.gamma_multiply(alpha);
    let font = FontId::proportional(size);
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;

    if badges {
        for b in &m.badges {
            let (sym, col) = match b {
                Badge::Broadcaster => ("◆", Color32::from_rgb(233, 25, 22)),
                Badge::Moderator => ("⚔", Color32::from_rgb(0, 173, 3)),
                Badge::Vip => ("♦", Color32::from_rgb(224, 5, 185)),
                Badge::Subscriber => ("★", Color32::from_rgb(169, 112, 255)),
            };
            job.append(
                sym,
                0.0,
                TextFormat { font_id: font.clone(), color: a(col), ..Default::default() },
            );
            job.append(" ", 0.0, TextFormat { font_id: font.clone(), ..Default::default() });
        }
    }

    let name_color = a(readable(m.color));
    job.append(
        &m.display_name,
        0.0,
        TextFormat { font_id: font.clone(), color: name_color, ..Default::default() },
    );
    if m.is_action {
        job.append(
            &format!(" {}", m.text),
            0.0,
            TextFormat { font_id: font, color: name_color, italics: true, ..Default::default() },
        );
    } else {
        job.append(
            ": ",
            0.0,
            TextFormat { font_id: font.clone(), color: a(Color32::WHITE), ..Default::default() },
        );
        job.append(
            &m.text,
            0.0,
            TextFormat { font_id: font, color: a(Color32::WHITE), ..Default::default() },
        );
    }
    job
}

/// Sehr dunkle Namensfarben aufhellen, damit sie auf dunklem Grund lesbar sind
fn readable(c: Color32) -> Color32 {
    let lum = 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
    if lum >= 90.0 {
        return c;
    }
    let t = (90.0 - lum) / 90.0 * 0.6;
    let mix = |v: u8| (v as f32 + (255.0 - v as f32) * t) as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // vollständig durchsichtig
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "config", &self.cfg);
    }

    fn auto_save_interval(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_events();
        let ctx = ui.ctx().clone();
        // regelmäßig neu zeichnen: Hotkeys abfragen, Ausblenden animieren
        ctx.request_repaint_after(Duration::from_millis(100));

        let passthrough = self.locked || self.hidden;
        if self.passthrough_applied != Some(passthrough) {
            ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(passthrough));
            self.passthrough_applied = Some(passthrough);
        }
        if self.hidden {
            return;
        }

        let edit_mode = !self.locked;
        let full = ui.max_rect();
        let show_bg = edit_mode || !self.cfg.auto_hide_bg || self.visible_count() > 0;
        let bg_alpha = if edit_mode { self.cfg.bg_opacity.max(0.55) } else { self.cfg.bg_opacity };
        if show_bg {
            ui.painter().rect_filled(
                full,
                CornerRadius::same(6),
                Color32::from_black_alpha((bg_alpha * 255.0) as u8),
            );
        }
        if edit_mode {
            ui.painter().rect_stroke(
                full.shrink(0.5),
                CornerRadius::same(6),
                Stroke::new(1.0, Color32::from_rgb(145, 70, 255)),
                egui::StrokeKind::Inside,
            );
        }

        let inner = full.shrink(8.0);
        let mut content = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        if edit_mode {
            self.title_bar(&mut content);
        }
        if edit_mode && self.show_settings {
            self.settings(&mut content);
        } else {
            self.chat(&mut content, edit_mode);
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("chat-game-overlay")
            .with_inner_size([380.0, 520.0])
            .with_min_inner_size([200.0, 120.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_icon(Arc::new(icon())),
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(OverlayApp::new(cc)))),
    )
}
