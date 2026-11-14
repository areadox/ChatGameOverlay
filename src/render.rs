//! Chatzeilen aus Text und Bildern (Emotes, Abzeichen) zusammensetzen.
//!
//! egui kann Bilder nicht in Fließtext setzen – deshalb ein eigener, einfacher
//! Zeilenumbruch: Wörter und Bilder werden nacheinander gesetzt, passt etwas
//! nicht mehr in die Zeile, beginnt eine neue.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Galley, TextFormat, Vec2, text::LayoutJob, vec2};

use crate::assets::Assets;
use crate::i18n::Lang;
use crate::images::{Images, emote_url};
use crate::irc::{Badge, ChatMessage, EmoteSpan, Highlight};

pub enum Atom {
    Text(String, TextFormat),
    /// Bild; `alt` wird gezeigt, wenn das Bild nicht geladen werden kann
    Image { url: String, height: f32, alt: String, alt_format: TextFormat },
    Newline,
}

pub enum Piece {
    Text(Arc<Galley>),
    Image { url: String },
}

pub struct Placed {
    pub offset: Vec2,
    pub size: Vec2,
    pub piece: Piece,
}

pub struct Options {
    pub size: f32,
    pub badges: bool,
    pub emotes: bool,
    /// Emotes von BTTV, FFZ und 7TV
    pub third_party: bool,
    pub lang: Lang,
}

/// Setzt die Bausteine in Zeilen der Breite `width`
pub fn layout(ui: &egui::Ui, atoms: Vec<Atom>, width: f32, images: &mut Images) -> (Vec<Placed>, Vec2) {
    let mut placed: Vec<Placed> = Vec::new();
    let mut line: Vec<Placed> = Vec::new();
    let (mut x, mut y, mut max_w) = (0.0_f32, 0.0_f32, 0.0_f32);

    let finish = |line: &mut Vec<Placed>, placed: &mut Vec<Placed>, y: &mut f32, x: &mut f32, max_w: &mut f32| {
        let h = line.iter().map(|p| p.size.y).fold(0.0, f32::max);
        for mut p in line.drain(..) {
            p.offset.y = *y + (h - p.size.y) / 2.0;
            placed.push(p);
        }
        *max_w = max_w.max(*x);
        *y += h;
        *x = 0.0;
    };

    for atom in atoms {
        let (piece, size) = match atom {
            Atom::Newline => {
                finish(&mut line, &mut placed, &mut y, &mut x, &mut max_w);
                continue;
            }
            Atom::Text(text, format) => {
                let mut job = LayoutJob::single_section(text.clone(), format.clone());
                job.wrap.max_width = f32::INFINITY;
                let mut g = ui.painter().layout_job(job);
                if g.size().x > width {
                    // überlanges Wort: in eigener Zeile umbrechen
                    if x > 0.0 {
                        finish(&mut line, &mut placed, &mut y, &mut x, &mut max_w);
                    }
                    let mut job = LayoutJob::single_section(text, format);
                    job.wrap.max_width = width;
                    job.wrap.break_anywhere = true;
                    g = ui.painter().layout_job(job);
                }
                let s = g.size();
                (Piece::Text(g), s)
            }
            Atom::Image { url, height, alt, alt_format } => {
                if images.failed(&url) {
                    let g = ui.painter().layout_job(LayoutJob::single_section(alt, alt_format));
                    let s = g.size();
                    (Piece::Text(g), s)
                } else {
                    let aspect = images.aspect(&url).unwrap_or(1.0);
                    (Piece::Image { url }, vec2(height * aspect, height))
                }
            }
        };
        if x > 0.0 && x + size.x > width {
            finish(&mut line, &mut placed, &mut y, &mut x, &mut max_w);
        }
        line.push(Placed { offset: vec2(x, 0.0), size, piece });
        x += size.x;
    }
    if !line.is_empty() {
        finish(&mut line, &mut placed, &mut y, &mut x, &mut max_w);
    }
    (placed, vec2(max_w, y))
}

/// Zerlegt eine Chatnachricht in Bausteine
pub fn atoms(m: &ChatMessage, o: &Options, assets: &Assets) -> Vec<Atom> {
    let size = o.size;
    let l = o.lang;
    let fmt = |font: FontId, color: Color32| TextFormat { font_id: font, color, ..Default::default() };
    let font = FontId::proportional(size);
    let white = fmt(font.clone(), Color32::WHITE);
    let mut out = Vec::new();

    if m.system {
        let f = TextFormat {
            italics: true,
            ..fmt(FontId::proportional(size * 0.9), Color32::from_gray(170))
        };
        push_words(&mut out, &m.text, &f);
        return out;
    }

    let name_fmt = fmt(font.clone(), readable(m.color));

    if let Some(Highlight::Reward { title, cost }) = &m.highlight {
        let small = FontId::proportional(size * 0.92);
        let gold = Color32::from_rgb(255, 210, 90);
        let w = fmt(small.clone(), Color32::WHITE);
        out.push(Atom::Text("◆ ".into(), fmt(small.clone(), gold)));
        out.push(Atom::Text(m.display_name.clone(), fmt(small.clone(), readable(m.color))));
        if let Some(kind) = title.strip_prefix("#auto:") {
            let phrase = match kind {
                "random_sub_emote_unlock" => l.t(" hat ein zufälliges Emote freigeschaltet", " unlocked a random emote"),
                "chosen_sub_emote_unlock" => l.t(" hat ein Emote freigeschaltet", " unlocked an emote"),
                "chosen_modified_sub_emote_unlock" => {
                    l.t(" hat ein verändertes Emote freigeschaltet", " unlocked a modified emote")
                }
                _ => l.t(" hat eine Belohnung eingelöst", " redeemed a reward"),
            };
            push_words(&mut out, phrase, &w);
        } else if title.is_empty() {
            push_words(&mut out, l.t(" hat eine Kanalpunkte-Belohnung eingelöst", " redeemed a channel points reward"), &w);
        } else {
            push_words(&mut out, l.t(" hat ", " redeemed "), &w);
            push_words(
                &mut out,
                &format!("{}{title}{}", l.t("„", "“"), l.t("“", "”")),
                &fmt(small.clone(), gold),
            );
            push_words(&mut out, l.t(" eingelöst", ""), &w);
        }
        if *cost > 0 {
            out.push(Atom::Text(format!("  · {cost}"), fmt(small, Color32::from_gray(190))));
        }
        if !m.text.is_empty() {
            out.push(Atom::Newline);
            push_text(&mut out, m, &white, o, assets);
        }
        return out;
    }

    if o.badges {
        for b in &m.badges {
            match assets.badge(b) {
                Some(info) => {
                    let (alt, col) = badge_symbol(b).unwrap_or(("", Color32::WHITE));
                    out.push(Atom::Image {
                        url: info.url,
                        height: (size * 1.05).round(),
                        alt: alt.into(),
                        alt_format: fmt(font.clone(), col),
                    });
                    out.push(Atom::Text(" ".into(), white.clone()));
                }
                None => {
                    if let Some((sym, col)) = badge_symbol(b) {
                        out.push(Atom::Text(format!("{sym} "), fmt(font.clone(), col)));
                    }
                }
            }
        }
    }

    out.push(Atom::Text(m.display_name.clone(), name_fmt.clone()));
    if m.is_action {
        out.push(Atom::Text(" ".into(), white.clone()));
        let f = TextFormat { italics: true, ..name_fmt };
        push_text(&mut out, m, &f, o, assets);
    } else {
        out.push(Atom::Text(": ".into(), white.clone()));
        push_text(&mut out, m, &white, o, assets);
    }
    out
}

/// Ersatzsymbol, solange es kein Bild gibt (nicht angemeldet)
fn badge_symbol(b: &Badge) -> Option<(&'static str, Color32)> {
    Some(match b.set.as_str() {
        "broadcaster" => ("◆", Color32::from_rgb(233, 25, 22)),
        "moderator" => ("⚔", Color32::from_rgb(0, 173, 3)),
        "vip" => ("♦", Color32::from_rgb(224, 5, 185)),
        "subscriber" | "founder" => ("★", Color32::from_rgb(169, 112, 255)),
        _ => return None,
    })
}

struct Span {
    start: usize,
    end: usize,
    url: String,
}

/// Text mit Emotes: Twitch-Positionen aus dem Chat, dazu Emotes über den Namen
/// (BTTV/FFZ/7TV immer, Twitch nur, wenn Twitch keine Positionen geliefert hat)
fn push_text(out: &mut Vec<Atom>, m: &ChatMessage, f: &TextFormat, o: &Options, assets: &Assets) {
    let text = &m.text;
    if !o.emotes {
        push_words(out, text, f);
        return;
    }
    let chars: Vec<char> = text.chars().collect();
    let spans = emote_spans(&chars, &m.emotes, !m.emotes_from_twitch, o.third_party, assets);
    let h = (o.size * 1.75).round();
    let mut pos = 0;
    for s in spans {
        let before: String = chars[pos..s.start].iter().collect();
        push_words(out, &before, f);
        let name: String = chars[s.start..s.end].iter().collect();
        out.push(Atom::Image { url: s.url, height: h, alt: name, alt_format: f.clone() });
        pos = s.end;
    }
    let rest: String = chars[pos..].iter().collect();
    push_words(out, &rest, f);
}

fn emote_spans(
    chars: &[char],
    twitch: &[EmoteSpan],
    twitch_by_name: bool,
    third: bool,
    assets: &Assets,
) -> Vec<Span> {
    let mut spans: Vec<Span> = twitch
        .iter()
        .filter(|s| s.start < s.end && s.end <= chars.len())
        .map(|s| Span { start: s.start, end: s.end, url: emote_url(&s.id) })
        .collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        if spans.iter().any(|s| s.start < i && start < s.end) {
            continue; // schon ein Twitch-Emote
        }
        let word: String = chars[start..i].iter().collect();
        if let Some(url) = assets.emote_url_by_name(&word, twitch_by_name, third) {
            spans.push(Span { start, end: i, url });
        }
    }
    spans.sort_by_key(|s| s.start);
    spans.dedup_by(|b, a| b.start < a.end);
    spans
}

/// Text in Wörter (mit folgendem Leerzeichen) zerlegen, damit an Wortgrenzen umbrochen wird
fn push_words(out: &mut Vec<Atom>, text: &str, f: &TextFormat) {
    for word in text.split_inclusive(' ') {
        if !word.is_empty() {
            out.push(Atom::Text(word.to_string(), f.clone()));
        }
    }
}

/// Sehr dunkle Namensfarben aufhellen, damit sie auf dunklem Grund lesbar sind
pub fn readable(c: Color32) -> Color32 {
    let lum = 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
    if lum >= 90.0 {
        return c;
    }
    let t = (90.0 - lum) / 90.0 * 0.6;
    let mix = |v: u8| (v as f32 + (255.0 - v as f32) * t) as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(atoms: &[Atom]) -> Vec<String> {
        atoms
            .iter()
            .map(|a| match a {
                Atom::Text(t, _) => t.clone(),
                Atom::Image { alt, .. } => format!("[{alt}]"),
                Atom::Newline => "\\n".into(),
            })
            .collect()
    }

    fn msg(text: &str, emotes: Vec<EmoteSpan>, from_twitch: bool) -> ChatMessage {
        ChatMessage {
            id: String::new(),
            user_login: "x".into(),
            display_name: "X".into(),
            color: Color32::WHITE,
            text: text.into(),
            is_action: false,
            system: false,
            highlight: None,
            reward_id: None,
            badges: Vec::new(),
            emotes,
            emotes_from_twitch: from_twitch,
            received: std::time::Instant::now(),
        }
    }

    #[test]
    fn splits_text_and_emotes() {
        let mut a = Assets::default();
        a.learn_emote("Kappa", "25");
        a.set_third_party_global(std::collections::HashMap::from([("catJAM".into(), "https://x/cat".into())]));
        let o = Options { size: 16.0, badges: false, emotes: true, third_party: true, lang: Lang::De };
        let f = TextFormat::default();
        // Chatnachricht mit Twitch-Positionen + BTTV-Emote über den Namen
        let mut out = Vec::new();
        let spans = vec![EmoteSpan { start: 6, end: 11, id: "25".into() }];
        push_text(&mut out, &msg("Hallo Kappa wie catJAM", spans, true), &f, &o, &a);
        assert_eq!(texts(&out), ["Hallo ", "[Kappa]", " ", "wie ", "[catJAM]"]);
        // Chatnachricht ohne Twitch-Positionen: „Kappa“ nicht raten (Absender hat es evtl. nicht)
        let mut out = Vec::new();
        push_text(&mut out, &msg("Kappa catJAM", vec![], true), &f, &o, &a);
        assert_eq!(texts(&out), ["Kappa ", "[catJAM]"]);
        // eigene Nachricht: Twitch-Emotes über bekannte Namen
        let mut out = Vec::new();
        push_text(&mut out, &msg("Kappa ü Kappa", vec![], false), &f, &o, &a);
        assert_eq!(texts(&out), ["[Kappa]", " ", "ü ", "[Kappa]"]);
        // BTTV/FFZ/7TV abgeschaltet
        let o2 = Options { third_party: false, ..o };
        let mut out = Vec::new();
        push_text(&mut out, &msg("catJAM", vec![], true), &f, &o2, &a);
        assert_eq!(texts(&out), ["catJAM"]);
    }
}
