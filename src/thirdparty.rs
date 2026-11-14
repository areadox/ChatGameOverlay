//! Emotes von BetterTTV, FrankerFaceZ und 7TV.
//!
//! Diese Dienste haben offene Schnittstellen (keine Anmeldung nötig). Ihre
//! Emotes stehen im Chat nur als Wort – erkannt werden sie über den Namen.

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use eframe::egui::Context;
use serde::Deserialize;

use crate::assets::Shared;

fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Option<T> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(format!("ChatGameOverlay/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    agent
        .get(url)
        .call()
        .ok()?
        .body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_json::<T>()
        .ok()
}

/// „//cdn…“ → „https://cdn…“
fn absolute(url: &str) -> String {
    if url.starts_with("//") { format!("https:{url}") } else { url.to_string() }
}

// ---------- BetterTTV ----------

#[derive(Deserialize, Default)]
#[serde(default)]
struct BttvEmote {
    id: String,
    code: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct BttvUser {
    channel_emotes: Vec<BttvEmote>,
    shared_emotes: Vec<BttvEmote>,
}

fn bttv_map(list: Vec<BttvEmote>, out: &mut HashMap<String, String>) {
    for e in list {
        if !e.id.is_empty() && !e.code.is_empty() {
            out.insert(e.code, format!("https://cdn.betterttv.net/emote/{}/2x", e.id));
        }
    }
}

// ---------- FrankerFaceZ ----------

#[derive(Deserialize, Default)]
#[serde(default)]
struct FfzSets {
    sets: HashMap<String, FfzSet>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FfzSet {
    emoticons: Vec<FfzEmote>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FfzEmote {
    name: String,
    urls: HashMap<String, Option<String>>,
}

fn ffz_map(sets: FfzSets, out: &mut HashMap<String, String>) {
    for set in sets.sets.into_values() {
        for e in set.emoticons {
            let url = ["2", "1", "4"].iter().find_map(|k| e.urls.get(*k).cloned().flatten());
            if let Some(u) = url
                && !e.name.is_empty()
            {
                out.insert(e.name, absolute(&u));
            }
        }
    }
}

// ---------- 7TV ----------

#[derive(Deserialize, Default)]
#[serde(default)]
struct SevenSet {
    emotes: Vec<SevenEmote>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SevenUser {
    emote_set: Option<SevenSet>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SevenEmote {
    name: String,
    data: SevenData,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SevenData {
    host: SevenHost,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SevenHost {
    url: String,
    files: Vec<SevenFile>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SevenFile {
    name: String,
}

fn seven_map(set: SevenSet, out: &mut HashMap<String, String>) {
    for e in set.emotes {
        if e.name.is_empty() || e.data.host.url.is_empty() {
            continue;
        }
        let files: Vec<&str> = e.data.host.files.iter().map(|f| f.name.as_str()).collect();
        // PNG bevorzugt, sonst WebP (beides kann das Programm lesen)
        let file = ["2x.png", "2x.webp", "1x.png", "1x.webp"]
            .into_iter()
            .find(|f| files.contains(f))
            .unwrap_or("2x.webp");
        out.insert(e.name, format!("{}/{file}", absolute(&e.data.host.url)));
    }
}

/// Globale Emotes aller drei Dienste
pub fn fetch_global(assets: Shared, ctx: Context) {
    thread::spawn(move || {
        let mut map = HashMap::new();
        if let Some(l) = get_json::<Vec<BttvEmote>>("https://api.betterttv.net/3/cached/emotes/global") {
            bttv_map(l, &mut map);
        }
        if let Some(s) = get_json::<FfzSets>("https://api.frankerfacez.com/v1/set/global") {
            ffz_map(s, &mut map);
        }
        if let Some(s) = get_json::<SevenSet>("https://7tv.io/v3/emote-sets/global") {
            seven_map(s, &mut map);
        }
        if let Ok(mut a) = assets.lock() {
            a.set_third_party_global(map);
        }
        ctx.request_repaint();
    });
}

/// Emotes eines Kanals (`room_id` = Twitch-ID des Kanals)
pub fn fetch_channel(room_id: String, assets: Shared, ctx: Context) {
    thread::spawn(move || {
        let mut map = HashMap::new();
        if let Some(u) = get_json::<BttvUser>(&format!("https://api.betterttv.net/3/cached/users/twitch/{room_id}")) {
            bttv_map(u.channel_emotes, &mut map);
            bttv_map(u.shared_emotes, &mut map);
        }
        if let Some(s) = get_json::<FfzSets>(&format!("https://api.frankerfacez.com/v1/room/id/{room_id}")) {
            ffz_map(s, &mut map);
        }
        if let Some(u) = get_json::<SevenUser>(&format!("https://7tv.io/v3/users/twitch/{room_id}"))
            && let Some(set) = u.emote_set
        {
            seven_map(set, &mut map);
        }
        if let Ok(mut a) = assets.lock() {
            a.set_third_party_channel(&room_id, map);
        }
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bttv() {
        let raw = r#"{"id":"x","bots":[],"channelEmotes":[{"id":"5e76d338d6581c3724c0f0b2","code":"catJAM","imageType":"gif","animated":true}],
                     "sharedEmotes":[{"id":"56e9f494fff3cc5c35e5287e","code":"monkaGIGA","imageType":"png"}]}"#;
        let u: BttvUser = serde_json::from_str(raw).unwrap();
        let mut m = HashMap::new();
        bttv_map(u.channel_emotes, &mut m);
        bttv_map(u.shared_emotes, &mut m);
        assert_eq!(m["catJAM"], "https://cdn.betterttv.net/emote/5e76d338d6581c3724c0f0b2/2x");
        assert!(m.contains_key("monkaGIGA"));
    }

    #[test]
    fn parses_ffz() {
        let raw = r#"{"room":{"set":123},"sets":{"123":{"id":123,"emoticons":[
            {"id":1,"name":"OMEGALUL","urls":{"1":"https://cdn.frankerfacez.com/emote/1/1","2":"https://cdn.frankerfacez.com/emote/1/2","4":null}},
            {"id":2,"name":"Old","urls":{"1":"//cdn.frankerfacez.com/emote/2/1"}}]}}}"#;
        let s: FfzSets = serde_json::from_str(raw).unwrap();
        let mut m = HashMap::new();
        ffz_map(s, &mut m);
        assert_eq!(m["OMEGALUL"], "https://cdn.frankerfacez.com/emote/1/2");
        assert_eq!(m["Old"], "https://cdn.frankerfacez.com/emote/2/1");
    }

    #[test]
    fn parses_7tv() {
        let raw = r#"{"id":"1","emote_set":{"id":"s","emotes":[{"id":"e1","name":"peepoHey","flags":0,
            "data":{"host":{"url":"//cdn.7tv.app/emote/e1","files":[{"name":"1x.avif"},{"name":"1x.webp"},{"name":"2x.avif"},{"name":"2x.webp"}]}}}]}}"#;
        let u: SevenUser = serde_json::from_str(raw).unwrap();
        let mut m = HashMap::new();
        seven_map(u.emote_set.unwrap(), &mut m);
        assert_eq!(m["peepoHey"], "https://cdn.7tv.app/emote/e1/2x.webp");
    }
}
