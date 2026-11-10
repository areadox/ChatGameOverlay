//! Eingabefeld mit Rechtsklick-Menü (Ausschneiden, Kopieren, Einfügen, Alles auswählen).

use eframe::egui::{self, Button, Response, TextEdit, text::CCursorRange};

use crate::i18n::Lang;

pub struct FieldOutput {
    pub response: Response,
    /// Enter wurde gedrückt, während das Feld aktiv war
    pub submitted: bool,
}

/// Einzeiliges Eingabefeld; `configure` setzt Hinweistext, Breite usw.
pub fn text_field(
    ui: &mut egui::Ui,
    text: &mut String,
    configure: impl FnOnce(TextEdit<'_>) -> TextEdit<'_>,
    l: Lang,
) -> FieldOutput {
    let out = configure(TextEdit::singleline(text)).show(ui);
    let response = out.response.response.clone();
    let id = response.id;
    let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

    // egui setzt beim Drücken der rechten Maustaste den Cursor an die Klickstelle und
    // löscht damit die Markierung. Deshalb die Markierung des vorigen Bildes merken und
    // beim Rechtsklick wiederherstellen, damit „Kopieren“/„Ausschneiden“ sie finden.
    let key = id.with("letzte_markierung");
    let right_press = response.hovered()
        && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary));
    if right_press {
        let prev: Option<CCursorRange> = ui.data(|d| d.get_temp(key)).flatten();
        let mut state = out.state.clone();
        state.cursor.set_char_range(prev);
        state.store(ui.ctx(), id);
    } else if !response.context_menu_opened() {
        let now: Option<CCursorRange> = out.state.cursor.char_range();
        ui.data_mut(|d| d.insert_temp(key, now));
    }
    let selection = ui
        .data(|d| d.get_temp::<Option<CCursorRange>>(key))
        .flatten()
        .filter(|r| !r.is_empty())
        .map(|r| r.as_sorted_char_range())
        .map(|r| r.start.0..r.end.0)
        .filter(|r| r.end <= text.chars().count());

    response.context_menu(|ui| {
        let has_sel = selection.is_some();
        let selected_text = selection.clone().map(|r| char_slice(text, r)).unwrap_or_default();

        if ui
            .add_enabled(has_sel, Button::new(l.t("Ausschneiden", "Cut")).shortcut_text("Ctrl+X"))
            .clicked()
        {
            if let Some(r) = selection.clone() {
                ui.ctx().copy_text(selected_text.clone());
                let (a, b) = (byte_at(text, r.start), byte_at(text, r.end));
                text.replace_range(a..b, "");
                set_cursor(ui.ctx(), id, r.start, r.start);
                ui.data_mut(|d| d.insert_temp::<Option<CCursorRange>>(key, None));
            }
            ui.close();
        }
        if ui
            .add_enabled(has_sel, Button::new(l.t("Kopieren", "Copy")).shortcut_text("Ctrl+C"))
            .clicked()
        {
            ui.ctx().copy_text(selected_text);
            ui.close();
        }
        if ui
            .add(Button::new(l.t("Einfügen", "Paste")).shortcut_text("Ctrl+V"))
            .clicked()
        {
            // Zwischenablage direkt lesen und an der Cursorposition einsetzen
            // (ersetzt eine Markierung, wie in jedem anderen Programm)
            if let Some(clip) = clipboard_text() {
                let clip = clip.replace(['\r', '\n'], " ");
                let len = text.chars().count();
                let r = selection.clone().unwrap_or_else(|| {
                    let c = cursor_pos(ui.ctx(), id).unwrap_or(len).min(len);
                    c..c
                });
                let (a, b) = (byte_at(text, r.start), byte_at(text, r.end));
                text.replace_range(a..b, &clip);
                let end = r.start + clip.chars().count();
                set_cursor(ui.ctx(), id, end, end);
                ui.data_mut(|d| d.insert_temp::<Option<CCursorRange>>(key, None));
            }
            ui.memory_mut(|m| m.request_focus(id));
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(!text.is_empty(), Button::new(l.t("Alles auswählen", "Select all")).shortcut_text("Ctrl+A"))
            .clicked()
        {
            set_cursor(ui.ctx(), id, 0, text.chars().count());
            ui.memory_mut(|m| m.request_focus(id));
            ui.close();
        }
    });

    FieldOutput { response, submitted }
}

fn set_cursor(ctx: &egui::Context, id: egui::Id, from: usize, to: usize) {
    if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
        let range = CCursorRange::two(
            egui::text::CCursor::new(from),
            egui::text::CCursor::new(to),
        );
        state.cursor.set_char_range(Some(range));
        state.store(ctx, id);
    }
}

fn clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok().filter(|t| !t.is_empty())
}

/// aktuelle Cursorposition (Zeichen) eines Feldes
fn cursor_pos(ctx: &egui::Context, id: egui::Id) -> Option<usize> {
    let state = egui::TextEdit::load_state(ctx, id)?;
    Some(state.cursor.char_range()?.primary.index.0)
}

fn byte_at(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(s.len())
}

fn char_slice(s: &str, r: std::ops::Range<usize>) -> String {
    s.chars().skip(r.start).take(r.end - r.start).collect()
}

#[cfg(test)]
mod tests {
    use super::{byte_at, char_slice};

    #[test]
    fn umlaut_safe_slicing() {
        let s = "Grüße an äöü";
        assert_eq!(char_slice(s, 0..5), "Grüße");
        assert_eq!(&s[byte_at(s, 9)..], "äöü");
        assert_eq!(byte_at(s, 99), s.len());
    }
}
