//! Kanalpunkte-Einlösungen über Twitch EventSub (WebSocket).
//!
//! Einlösungen ohne Texteingabe tauchen im IRC-Chat gar nicht auf – deshalb
//! eine zweite Verbindung zu EventSub. Twitch erlaubt das nur für den eigenen
//! Kanal und mit der Berechtigung `channel:read:redemptions`.

use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::Context;
use serde::Deserialize;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::auth::CLIENT_ID;
use crate::irc::{ChatEvent, ChatMessage, Highlight, color_for};

const WS_URL: &str = "wss://eventsub.wss.twitch.tv/ws";
const SUBSCRIBE_URL: &str = "https://api.twitch.tv/helix/eventsub/subscriptions";

#[derive(Clone, Debug, PartialEq)]
pub enum EsStatus {
    Connecting,
    Active,
    /// Token hat die Berechtigung nicht (neu anmelden)
    MissingScope,
    Error(String),
}

type Ws = WebSocket<MaybeTlsStream<TcpStream>>;

/// Startet den Empfang. `token`/`user_id` gehören dem angemeldeten Kanal-Inhaber.
pub fn spawn(token: String, user_id: String, tx: Sender<ChatEvent>, ctx: Context, stop: Arc<AtomicBool>) {
    thread::spawn(move || {
        let send = |ev: ChatEvent| {
            let _ = tx.send(ev);
            ctx.request_repaint();
        };
        while !stop.load(Ordering::Relaxed) {
            send(ChatEvent::Redemptions(EsStatus::Connecting));
            match run(&token, &user_id, &send, &stop) {
                Ok(()) => return, // gestoppt
                Err(f) => {
                    send(ChatEvent::Redemptions(f.status));
                    if !f.retry {
                        return;
                    }
                }
            }
            // kurz warten, dann neu verbinden
            for _ in 0..100 {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    });
}

/// Fehler, der die aktuelle Verbindung beendet
struct Fatal {
    status: EsStatus,
    /// später erneut versuchen (Netzwerk) oder aufgeben (z. B. Berechtigung fehlt)
    retry: bool,
}

fn fatal_stop(status: EsStatus) -> Fatal {
    Fatal { status, retry: false }
}

fn fatal_retry(status: EsStatus) -> Fatal {
    Fatal { status, retry: true }
}

fn open(url: &str) -> Result<Ws, Fatal> {
    let (mut ws, _) = tungstenite::connect(url)
        .map_err(|e| fatal_retry(EsStatus::Error(e.to_string())))?;
    let timeout = Some(Duration::from_millis(250));
    match ws.get_mut() {
        MaybeTlsStream::Rustls(s) => {
            let _ = s.get_mut().set_read_timeout(timeout);
        }
        MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(timeout);
        }
        _ => {}
    }
    Ok(ws)
}

/// Liest die nächste Textnachricht; `None` bei Zeitüberschreitung des Lesevorgangs
fn next_text(ws: &mut Ws) -> Result<Option<String>, Fatal> {
    match ws.read() {
        Ok(Message::Text(t)) => Ok(Some(t.to_string())),
        Ok(Message::Close(_)) => Err(fatal_retry(EsStatus::Error("closed".into()))),
        Ok(_) => Ok(None), // Ping/Pong beantwortet tungstenite selbst
        Err(tungstenite::Error::Io(e))
            if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) =>
        {
            Ok(None)
        }
        Err(e) => Err(fatal_retry(EsStatus::Error(e.to_string()))),
    }
}

/// Wartet auf `session_welcome` und liefert (Session-ID, Keepalive-Sekunden)
fn wait_welcome(ws: &mut Ws, stop: &AtomicBool) -> Result<(String, u64), Fatal> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15) {
        if stop.load(Ordering::Relaxed) {
            return Err(fatal_stop(EsStatus::Error(String::new())));
        }
        if let Some(text) = next_text(ws)? {
            let msg: Envelope = serde_json::from_str(&text).unwrap_or_default();
            if msg.metadata.message_type == "session_welcome" {
                let s = msg.payload.session.unwrap_or_default();
                return Ok((s.id, s.keepalive_timeout_seconds.unwrap_or(10)));
            }
        }
    }
    Err(fatal_retry(EsStatus::Error("no welcome".into())))
}

fn subscribe(token: &str, user_id: &str, session: &str) -> Result<(), Fatal> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    for (kind, version) in [
        ("channel.channel_points_custom_reward_redemption.add", "1"),
        ("channel.channel_points_automatic_reward_redemption.add", "2"),
    ] {
        let body = serde_json::json!({
            "type": kind,
            "version": version,
            "condition": { "broadcaster_user_id": user_id },
            "transport": { "method": "websocket", "session_id": session },
        });
        let mut resp = agent
            .post(SUBSCRIBE_URL)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Client-Id", CLIENT_ID)
            .send_json(&body)
            .map_err(|e| fatal_retry(EsStatus::Error(e.to_string())))?;
        let status = resp.status().as_u16();
        match status {
            200..=299 | 409 => {} // 409 = besteht schon
            401 | 403 => return Err(fatal_stop(EsStatus::MissingScope)),
            _ => {
                let text = resp.body_mut().read_to_string().unwrap_or_default();
                return Err(fatal_stop(EsStatus::Error(format!("{status} {text}"))));
            }
        }
    }
    Ok(())
}

fn run(
    token: &str,
    user_id: &str,
    send: &dyn Fn(ChatEvent),
    stop: &AtomicBool,
) -> Result<(), Fatal> {
    let mut ws = open(WS_URL)?;
    let (session, mut keepalive) = wait_welcome(&mut ws, stop)?;
    subscribe(token, user_id, &session)?;
    send(ChatEvent::Redemptions(EsStatus::Active));

    let mut last = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = ws.close(None);
            return Ok(());
        }
        if last.elapsed() > Duration::from_secs(keepalive + 5) {
            return Err(fatal_retry(EsStatus::Error("keepalive timeout".into())));
        }
        let Some(text) = next_text(&mut ws)? else { continue };
        last = Instant::now();
        let msg: Envelope = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(_) => continue,
        };
        match msg.metadata.message_type.as_str() {
            "notification" => {
                if let Some(m) = to_message(&msg) {
                    send(ChatEvent::Message(m));
                }
            }
            "session_reconnect" => {
                // neue Verbindung aufbauen; Abos bleiben bei Twitch bestehen
                let url = msg.payload.session.and_then(|s| s.reconnect_url).unwrap_or_default();
                if url.is_empty() {
                    return Err(fatal_retry(EsStatus::Error("reconnect".into())));
                }
                let mut new_ws = open(&url)?;
                let (_, ka) = wait_welcome(&mut new_ws, stop)?;
                let _ = ws.close(None);
                ws = new_ws;
                keepalive = ka;
                last = Instant::now();
            }
            "revocation" => return Err(fatal_stop(EsStatus::MissingScope)),
            _ => {} // session_keepalive
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Envelope {
    metadata: Metadata,
    payload: Payload,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Metadata {
    message_type: String,
    subscription_type: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Payload {
    session: Option<Session>,
    event: Option<Event>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Session {
    id: String,
    keepalive_timeout_seconds: Option<u64>,
    reconnect_url: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Event {
    id: String,
    user_login: String,
    user_name: String,
    user_input: String,
    reward: Reward,
    message: Option<AutoMessage>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Reward {
    // eigene Belohnung
    title: String,
    cost: u64,
    // automatische Belohnung
    #[serde(rename = "type")]
    kind: String,
    channel_points: u64,
    emote: Option<Emote>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Emote {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AutoMessage {
    text: String,
}

/// Macht aus einer EventSub-Benachrichtigung eine Chatzeile
fn to_message(msg: &Envelope) -> Option<ChatMessage> {
    let ev = msg.payload.event.as_ref()?;
    let (title, cost, text) = match msg.metadata.subscription_type.as_str() {
        "channel.channel_points_custom_reward_redemption.add" => {
            (ev.reward.title.clone(), ev.reward.cost, ev.user_input.clone())
        }
        "channel.channel_points_automatic_reward_redemption.add" => {
            match ev.reward.kind.as_str() {
                // erscheinen ohnehin im Chat (hervorgehoben bzw. trotz Abo-Modus)
                "send_highlighted_message" | "single_message_bypass_sub_mode" => return None,
                _ => {}
            }
            let emote = ev.reward.emote.as_ref().map(|e| e.name.clone()).unwrap_or_default();
            let title = format!("#auto:{}", ev.reward.kind);
            let text = match &ev.message {
                Some(m) if !m.text.is_empty() => m.text.clone(),
                _ => emote,
            };
            (title, ev.reward.channel_points, text)
        }
        _ => return None,
    };
    let name = if ev.user_name.is_empty() { ev.user_login.clone() } else { ev.user_name.clone() };
    Some(ChatMessage {
        id: ev.id.clone(),
        user_login: ev.user_login.clone(),
        display_name: name,
        color: color_for(&ev.user_login),
        text,
        is_action: false,
        system: false,
        badges: Vec::new(),
        highlight: Some(Highlight::Reward { title, cost }),
        reward_id: None,
        received: Instant::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_redemption() {
        let raw = r#"{"metadata":{"message_type":"notification","subscription_type":"channel.channel_points_custom_reward_redemption.add"},
        "payload":{"subscription":{"type":"channel.channel_points_custom_reward_redemption.add"},
        "event":{"id":"17fa","user_id":"9001","user_login":"cooler_user","user_name":"Cooler_User","user_input":"pogchamp",
        "status":"unfulfilled","reward":{"id":"92af","title":"Hydrate!","cost":500,"prompt":"p"},"redeemed_at":"2020-07-15T17:16:03Z"}}}"#;
        let env: Envelope = serde_json::from_str(raw).unwrap();
        let m = to_message(&env).unwrap();
        assert_eq!(m.display_name, "Cooler_User");
        assert_eq!(m.text, "pogchamp");
        assert_eq!(m.highlight, Some(Highlight::Reward { title: "Hydrate!".into(), cost: 500 }));
    }

    #[test]
    fn automatic_redemption() {
        let raw = r#"{"metadata":{"message_type":"notification","subscription_type":"channel.channel_points_automatic_reward_redemption.add"},
        "payload":{"event":{"id":"f024","user_login":"twitchdev","user_name":"TwitchDev",
        "reward":{"type":"random_sub_emote_unlock","channel_points":200,"emote":{"id":"1","name":"VoHiYo"}},
        "message":null,"redeemed_at":"2024-08-12T21:14:34Z"}}}"#;
        let env: Envelope = serde_json::from_str(raw).unwrap();
        let m = to_message(&env).unwrap();
        assert_eq!(m.text, "VoHiYo");
        assert_eq!(
            m.highlight,
            Some(Highlight::Reward { title: "#auto:random_sub_emote_unlock".into(), cost: 200 })
        );

        // „Nachricht hervorheben“ kommt schon über den Chat → nicht doppelt
        let raw2 = raw.replace("random_sub_emote_unlock", "send_highlighted_message");
        let env2: Envelope = serde_json::from_str(&raw2).unwrap();
        assert!(to_message(&env2).is_none());
    }

    #[test]
    fn welcome_parses() {
        let raw = r#"{"metadata":{"message_type":"session_welcome"},"payload":{"session":{"id":"abc","status":"connected","keepalive_timeout_seconds":10,"reconnect_url":null}}}"#;
        let env: Envelope = serde_json::from_str(raw).unwrap();
        assert_eq!(env.payload.session.unwrap().id, "abc");
    }
}
