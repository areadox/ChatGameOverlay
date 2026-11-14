//! Lädt Bilder (Emotes, Abzeichen) im Hintergrund und hält sie als Texturen bereit.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions, Vec2};

/// Größte Kantenlänge, die geladen wird (schützt vor Riesenbildern)
const MAX_SIDE: u32 = 512;
const WORKERS: usize = 4;

enum Slot {
    Loading,
    Ready(TextureHandle),
    Failed,
}

pub struct Images {
    ctx: Context,
    slots: HashMap<String, Slot>,
    requests: Sender<String>,
    done: Receiver<(String, Option<ColorImage>)>,
}

impl Images {
    pub fn new(ctx: Context) -> Self {
        let (req_tx, req_rx) = mpsc::channel::<String>();
        let (done_tx, done_rx) = mpsc::channel();
        let req_rx = Arc::new(Mutex::new(req_rx));
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(format!("ChatGameOverlay/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        for _ in 0..WORKERS {
            let rx = req_rx.clone();
            let tx = done_tx.clone();
            let agent = agent.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                loop {
                    let url = match rx.lock() {
                        Ok(r) => match r.recv() {
                            Ok(u) => u,
                            Err(_) => return,
                        },
                        Err(_) => return,
                    };
                    let img = fetch(&agent, &url);
                    if tx.send((url, img)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        }
        Self { ctx, slots: HashMap::new(), requests: req_tx, done: done_rx }
    }

    /// Fertig geladene Bilder übernehmen (einmal pro Bild aufrufen)
    pub fn poll(&mut self) {
        while let Ok((url, img)) = self.done.try_recv() {
            let slot = match img {
                Some(img) => Slot::Ready(self.ctx.load_texture(&url, img, TextureOptions::LINEAR)),
                None => Slot::Failed,
            };
            self.slots.insert(url, slot);
        }
    }

    /// Textur zu einer Adresse; startet beim ersten Aufruf den Download.
    pub fn get(&mut self, url: &str) -> Option<&TextureHandle> {
        if !self.slots.contains_key(url) {
            self.slots.insert(url.to_string(), Slot::Loading);
            let _ = self.requests.send(url.to_string());
            return None;
        }
        match self.slots.get(url) {
            Some(Slot::Ready(t)) => Some(t),
            _ => None,
        }
    }

    /// Seitenverhältnis (Breite/Höhe), sobald bekannt
    pub fn aspect(&mut self, url: &str) -> Option<f32> {
        self.get(url).map(|t| {
            let s: Vec2 = t.size_vec2();
            if s.y > 0.0 { s.x / s.y } else { 1.0 }
        })
    }

    pub fn failed(&self, url: &str) -> bool {
        matches!(self.slots.get(url), Some(Slot::Failed))
    }
}

fn fetch(agent: &ureq::Agent, url: &str) -> Option<ColorImage> {
    let bytes = agent
        .get(url)
        .call()
        .ok()?
        .body_mut()
        .with_config()
        .limit(8 * 1024 * 1024)
        .read_to_vec()
        .ok()?;
    decode(&bytes)
}

fn decode(bytes: &[u8]) -> Option<ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        img.thumbnail(MAX_SIDE, MAX_SIDE)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Some(ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

/// Adresse eines Twitch-Emotes (statische PNG-Variante, doppelte Größe)
pub fn emote_url(id: &str) -> String {
    format!("https://static-cdn.jtvnw.net/emoticons/v2/{id}/static/dark/2.0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png() {
        // 2×1 Pixel PNG erzeugen und wieder lesen
        let mut buf = Vec::new();
        let img = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let c = decode(&buf).unwrap();
        assert_eq!(c.size, [2, 1]);
        assert!(decode(b"kein bild").is_none());
        assert_eq!(
            emote_url("25"),
            "https://static-cdn.jtvnw.net/emoticons/v2/25/static/dark/2.0"
        );
    }
}
