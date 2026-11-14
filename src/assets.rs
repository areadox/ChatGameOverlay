//! Abzeichen- und Emote-Listen von Twitch (Helix-API).
//!
//! Die Bilder der Abzeichen kennt nur die API – dafür braucht es ein Token,
//! also die Anmeldung. Ohne Anmeldung zeigt das Programm einfache Symbole.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eframe::egui::Context;
use serde::Deserialize;

use crate::auth::CLIENT_ID;
use crate::irc::Badge;

#[derive(Default)]
pub struct Assets {
    global_badges: HashMap<(String, String), BadgeInfo>,
    channel_badges: HashMap<(String, String), BadgeInfo>,
    /// Emote-Name → Twitch-Emote-ID (global, Kanal und im Chat gesehene)
    emotes: HashMap<String, String>,
}

#[derive(Clone)]
pub struct BadgeInfo {
    pub url: String,
}

pub type Shared = Arc<Mutex<Assets>>;

impl Assets {
    /// Kanal-Abzeichen (z. B. eigene Abo-Abzeichen) gehen vor den globalen
    pub fn badge(&self, b: &Badge) -> Option<BadgeInfo> {
        let key = (b.set.clone(), b.version.clone());
        self.channel_badges.get(&key).or_else(|| self.global_badges.get(&key)).cloned()
    }

    pub fn emote_id(&self, name: &str) -> Option<&String> {
        self.emotes.get(name)
    }

    /// Emote aus einer Chatnachricht merken (damit es auch in eigenen Nachrichten erscheint)
    pub fn learn_emote(&mut self, name: &str, id: &str) {
        if !name.is_empty() && !self.emotes.contains_key(name) {
            self.emotes.insert(name.to_string(), id.to_string());
        }
    }

    pub fn clear_channel(&mut self) {
        self.channel_badges.clear();
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BadgeResp {
    data: Vec<BadgeSet>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BadgeSet {
    set_id: String,
    versions: Vec<BadgeVersion>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BadgeVersion {
    id: String,
    image_url_2x: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EmoteResp {
    data: Vec<EmoteEntry>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EmoteEntry {
    id: String,
    name: String,
}

fn helix<T: for<'de> Deserialize<'de> + Default>(token: &str, url: &str) -> Option<T> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    agent
        .get(url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Client-Id", CLIENT_ID)
        .call()
        .ok()?
        .body_mut()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_json::<T>()
        .ok()
}

fn badge_map(resp: BadgeResp) -> HashMap<(String, String), BadgeInfo> {
    let mut map = HashMap::new();
    for set in resp.data {
        for v in set.versions {
            if !v.image_url_2x.is_empty() {
                map.insert((set.set_id.clone(), v.id), BadgeInfo { url: v.image_url_2x });
            }
        }
    }
    map
}

/// Globale Abzeichen und Emotes laden (einmal pro Anmeldung)
pub fn fetch_global(token: String, assets: Shared, ctx: Context) {
    thread::spawn(move || {
        if let Some(r) = helix::<BadgeResp>(&token, "https://api.twitch.tv/helix/chat/badges/global")
            && let Ok(mut a) = assets.lock()
        {
            a.global_badges = badge_map(r);
        }
        if let Some(r) = helix::<EmoteResp>(&token, "https://api.twitch.tv/helix/chat/emotes/global")
            && let Ok(mut a) = assets.lock()
        {
            for e in r.data {
                a.emotes.insert(e.name, e.id);
            }
        }
        ctx.request_repaint();
    });
}

/// Abzeichen und Emotes eines Kanals laden
pub fn fetch_channel(token: String, room_id: String, assets: Shared, ctx: Context) {
    thread::spawn(move || {
        let url = format!("https://api.twitch.tv/helix/chat/badges?broadcaster_id={room_id}");
        if let Some(r) = helix::<BadgeResp>(&token, &url)
            && let Ok(mut a) = assets.lock()
        {
            a.channel_badges = badge_map(r);
        }
        let url = format!("https://api.twitch.tv/helix/chat/emotes?broadcaster_id={room_id}");
        if let Some(r) = helix::<EmoteResp>(&token, &url)
            && let Ok(mut a) = assets.lock()
        {
            for e in r.data {
                a.emotes.insert(e.name, e.id);
            }
        }
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_badge_response() {
        let raw = r#"{"data":[{"set_id":"subscriber","versions":[
            {"id":"0","image_url_1x":"a1","image_url_2x":"a2","image_url_4x":"a4","title":"Subscriber"},
            {"id":"12","image_url_1x":"b1","image_url_2x":"b2","image_url_4x":"b4","title":"1-Year Subscriber"}]}]}"#;
        let r: BadgeResp = serde_json::from_str(raw).unwrap();
        let mut a = Assets { channel_badges: badge_map(r), ..Default::default() };
        let info = a.badge(&Badge::new("subscriber", "12")).unwrap();
        assert_eq!(info.url, "b2");
        assert!(a.badge(&Badge::new("moderator", "1")).is_none());
        a.learn_emote("Kappa", "25");
        a.learn_emote("Kappa", "999"); // bekannte bleiben
        assert_eq!(a.emote_id("Kappa").map(String::as_str), Some("25"));
    }
}
