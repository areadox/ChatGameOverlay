//! Verbindung zum Twitch-Chat (IRC).
//!
//! Ohne Anmeldung liest das Programm anonym als „justinfan…“ mit.
//! Mit Anmeldung (Token mit chat:read/chat:edit) kann es auch schreiben.

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{Color32, Context};

const SERVER: &str = "irc.chat.twitch.tv:6667";

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub user_login: String,
    pub display_name: String,
    pub color: Color32,
    pub text: String,
    pub is_action: bool,
    /// Hinweis von Twitch (z. B. Slow-Mode), kein Benutzertext
    pub system: bool,
    /// farbig hervorheben (Kanalpunkte)
    pub highlight: Option<Highlight>,
    /// Chatnachricht, die zu einer eigenen Kanalpunkte-Belohnung gehört (Texteingabe)
    pub reward_id: Option<String>,
    pub badges: Vec<Badge>,
    pub received: Instant,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Highlight {
    /// Kanalpunkte-Belohnung eingelöst. `title` beginnt mit „#auto:“ bei
    /// automatischen Belohnungen (z. B. Emote freischalten), leer = unbekannt.
    Reward { title: String, cost: u64 },
    /// „Nachricht hervorheben“ über Kanalpunkte
    Highlighted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge {
    Broadcaster,
    Moderator,
    Vip,
    Subscriber,
}

pub enum ChatEvent {
    Message(ChatMessage),
    /// Einzelne Nachricht gelöscht (Moderation).
    Delete(String),
    /// Alle Nachrichten eines Users gelöscht (Timeout/Bann) – `None` = ganzer Chat geleert.
    ClearUser(Option<String>),
    Status(Status),
    /// eigener Anzeigename, Farbe und Abzeichen (für selbst geschriebene Nachrichten)
    OwnState { display_name: String, color: Option<Color32>, badges: Vec<Badge> },
    /// Hinweis von Twitch, z. B. „Slow-Mode aktiv“
    Notice(String),
    /// Token ungültig – Verbindung wurde beendet
    AuthFailed,
    /// Zustand des Kanalpunkte-Empfangs (EventSub)
    Redemptions(crate::eventsub::EsStatus),
}

/// Zugangsdaten für den Chat: (Login-Name, Token)
pub type Credentials = Option<(String, String)>;

enum End {
    Retry,
    AuthFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Connected,
    Disconnected,
}

/// Läuft im Hintergrund, bis `stop` gesetzt wird. Verbindet bei Abbruch automatisch neu.
/// Über `outgoing` geschickte Texte werden als Chatnachricht gesendet (nur mit Anmeldung).
pub fn spawn(
    channel: String,
    creds: Credentials,
    tx: Sender<ChatEvent>,
    outgoing: Receiver<String>,
    ctx: Context,
    stop: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let channel = channel.trim().trim_start_matches('#').to_lowercase();
        while !stop.load(Ordering::Relaxed) {
            let _ = tx.send(ChatEvent::Status(Status::Connecting));
            ctx.request_repaint();
            let end = run_connection(&channel, &creds, &tx, &outgoing, &ctx, &stop);
            if let Ok(End::AuthFailed) = end {
                let _ = tx.send(ChatEvent::AuthFailed);
                let _ = tx.send(ChatEvent::Status(Status::Disconnected));
                ctx.request_repaint();
                return;
            }
            let _ = tx.send(ChatEvent::Status(Status::Disconnected));
            ctx.request_repaint();
            // kurz warten, dann neu verbinden
            for _ in 0..50 {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    });
}

fn run_connection(
    channel: &str,
    creds: &Credentials,
    tx: &Sender<ChatEvent>,
    outgoing: &Receiver<String>,
    ctx: &Context,
    stop: &AtomicBool,
) -> std::io::Result<End> {
    let mut stream = TcpStream::connect(SERVER)?;
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let (pass, nick) = match creds {
        Some((login, token)) => (format!("oauth:{token}"), login.to_lowercase()),
        None => (
            "SCHMOOPIIE".to_string(),
            format!("justinfan{}", 10000 + (std::process::id() % 80000)),
        ),
    };
    write!(
        stream,
        "CAP REQ :twitch.tv/tags twitch.tv/commands\r\nPASS {pass}\r\nNICK {nick}\r\nJOIN #{channel}\r\n"
    )?;
    stream.flush()?;

    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut last_data = Instant::now();

    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = writer.write_all(format!("PART #{channel}\r\n").as_bytes());
            return Ok(End::Retry);
        }
        // eigene Nachrichten senden
        while let Ok(text) = outgoing.try_recv() {
            if creds.is_some() {
                let text: String = text.replace(['\r', '\n'], " ");
                writer.write_all(format!("PRIVMSG #{channel} :{text}\r\n").as_bytes())?;
            }
        }
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(End::Retry), // Verbindung geschlossen
            Ok(_) => {
                if !line.ends_with('\n') {
                    continue;
                }
                last_data = Instant::now();
                let msg = line.trim_end().to_string();
                line.clear();
                if let Some(rest) = msg.strip_prefix("PING") {
                    writer.write_all(format!("PONG{rest}\r\n").as_bytes())?;
                    continue;
                }
                if let Some(ev) = parse_line(&msg) {
                    let auth_failed = matches!(ev, ChatEvent::AuthFailed);
                    let _ = tx.send(ev);
                    ctx.request_repaint();
                    if auth_failed {
                        return Ok(End::AuthFailed);
                    }
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                // Twitch schickt alle ~5 Minuten ein PING. Kommt 7 Minuten lang nichts,
                // ist die Verbindung tot.
                if last_data.elapsed() > Duration::from_secs(420) {
                    return Ok(End::Retry);
                }
            }
            Err(e) => return Err(e),
        }
    }
}

/// Zerlegt eine IRC-Zeile mit Twitch-Tags.
fn parse_line(raw: &str) -> Option<ChatEvent> {
    let (tags, rest) = match raw.strip_prefix('@') {
        Some(r) => {
            let (t, rest) = r.split_once(' ')?;
            (t, rest)
        }
        None => ("", raw),
    };
    let tag = |key: &str| -> Option<String> {
        tags.split(';').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k == key).then(|| unescape_tag(v))
        })
    };

    let (prefix, rest) = match rest.strip_prefix(':') {
        Some(r) => r.split_once(' ')?,
        None => ("", rest),
    };
    let (command, params) = rest.split_once(' ').unwrap_or((rest, ""));

    match command {
        "001" | "ROOMSTATE" => Some(ChatEvent::Status(Status::Connected)),
        "PRIVMSG" => {
            let text = params.split_once(" :").map(|(_, t)| t).unwrap_or("");
            let login = prefix.split('!').next().unwrap_or("").to_string();
            let (text, is_action) = match text
                .strip_prefix("\u{1}ACTION ")
                .and_then(|t| t.strip_suffix('\u{1}'))
            {
                Some(t) => (t.to_string(), true),
                None => (text.to_string(), false),
            };
            let display_name = tag("display-name")
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| login.clone());
            let color = tag("color")
                .and_then(|c| Color32::from_hex(&c).ok())
                .unwrap_or_else(|| fallback_color(&login));
            let badges = parse_badges(&tag("badges").unwrap_or_default());
            let highlight = match tag("msg-id").as_deref() {
                Some("highlighted-message") => Some(Highlight::Highlighted),
                _ => None,
            };
            let reward_id = tag("custom-reward-id").filter(|r| !r.is_empty());
            Some(ChatEvent::Message(ChatMessage {
                id: tag("id").unwrap_or_default(),
                user_login: login,
                display_name,
                color,
                text,
                is_action,
                system: false,
                highlight,
                reward_id,
                badges,
                received: Instant::now(),
            }))
        }
        "CLEARMSG" => tag("target-msg-id").map(ChatEvent::Delete),
        "CLEARCHAT" => {
            let user = params.split_once(" :").map(|(_, u)| u.to_lowercase());
            Some(ChatEvent::ClearUser(user))
        }
        "USERSTATE" | "GLOBALUSERSTATE" => Some(ChatEvent::OwnState {
            display_name: tag("display-name").unwrap_or_default(),
            color: tag("color").and_then(|c| Color32::from_hex(&c).ok()),
            badges: parse_badges(&tag("badges").unwrap_or_default()),
        }),
        "NOTICE" => {
            let text = params.split_once(" :").map(|(_, t)| t).unwrap_or("").to_string();
            if text.contains("Login authentication failed")
                || text.contains("Improperly formatted auth")
            {
                Some(ChatEvent::AuthFailed)
            } else if text.is_empty() {
                None
            } else {
                Some(ChatEvent::Notice(text))
            }
        }
        "RECONNECT" => None, // Server trennt gleich – Neuverbindung passiert automatisch
        _ => None,
    }
}

fn parse_badges(raw: &str) -> Vec<Badge> {
    raw.split(',')
        .filter_map(|b| match b.split('/').next()? {
            "broadcaster" => Some(Badge::Broadcaster),
            "moderator" => Some(Badge::Moderator),
            "vip" => Some(Badge::Vip),
            "subscriber" | "founder" => Some(Badge::Subscriber),
            _ => None,
        })
        .collect()
}

/// Twitch-Standardfarbe für einen Namen (wenn er keine eigene hat)
pub fn color_for(login: &str) -> Color32 {
    fallback_color(login)
}

fn unescape_tag(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('s') => out.push(' '),
                Some(':') => out.push(';'),
                Some('\\') => out.push('\\'),
                Some('r') => out.push('\r'),
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Twitch-Standardfarben für User ohne eigene Namensfarbe.
fn fallback_color(login: &str) -> Color32 {
    const COLORS: [(u8, u8, u8); 15] = [
        (0xFF, 0x00, 0x00), (0x00, 0x00, 0xFF), (0x00, 0x80, 0x00), (0xB2, 0x22, 0x22),
        (0xFF, 0x7F, 0x50), (0x9A, 0xCD, 0x32), (0xFF, 0x45, 0x00), (0x2E, 0x8B, 0x57),
        (0xDA, 0xA5, 0x20), (0xD2, 0x69, 0x1E), (0x5F, 0x9E, 0xA0), (0x1E, 0x90, 0xFF),
        (0xFF, 0x69, 0xB4), (0x8A, 0x2B, 0xE2), (0x00, 0xFF, 0x7F),
    ];
    let h = login.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let (r, g, b) = COLORS[(h as usize) % COLORS.len()];
    Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_privmsg() {
        let raw = "@badges=moderator/1,subscriber/12;color=#1E90FF;display-name=Test\\sUser;id=abc-123 :testuser!testuser@testuser.tmi.twitch.tv PRIVMSG #areadox :Hallo Chat :)";
        match parse_line(raw) {
            Some(ChatEvent::Message(m)) => {
                assert_eq!(m.display_name, "Test User");
                assert_eq!(m.user_login, "testuser");
                assert_eq!(m.text, "Hallo Chat :)");
                assert_eq!(m.id, "abc-123");
                assert_eq!(m.color, Color32::from_rgb(0x1E, 0x90, 0xFF));
                assert_eq!(m.badges, vec![Badge::Moderator, Badge::Subscriber]);
                assert!(!m.is_action);
            }
            _ => panic!("keine Nachricht erkannt"),
        }
    }

    #[test]
    fn parses_channel_point_tags() {
        let raw = "@color=#FF0000;display-name=Max;id=1;msg-id=highlighted-message :max!max@max PRIVMSG #areadox :Seht her!";
        match parse_line(raw) {
            Some(ChatEvent::Message(m)) => assert_eq!(m.highlight, Some(Highlight::Highlighted)),
            _ => panic!(),
        }
        let raw = "@color=;custom-reward-id=92af-127c;display-name=Max;id=2 :max!max@max PRIVMSG #areadox :Song: Darude";
        match parse_line(raw) {
            Some(ChatEvent::Message(m)) => {
                assert_eq!(m.reward_id.as_deref(), Some("92af-127c"));
                assert_eq!(m.highlight, None);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parses_action_and_clear() {
        let raw = "@color=;display-name=x :x!x@x PRIVMSG #a :\u{1}ACTION winkt\u{1}";
        match parse_line(raw) {
            Some(ChatEvent::Message(m)) => {
                assert!(m.is_action);
                assert_eq!(m.text, "winkt");
            }
            _ => panic!(),
        }
        assert!(matches!(
            parse_line("@ban-duration=600 :tmi.twitch.tv CLEARCHAT #a :Troll"),
            Some(ChatEvent::ClearUser(Some(u))) if u == "troll"
        ));
        assert!(matches!(
            parse_line("@target-msg-id=xyz :tmi.twitch.tv CLEARMSG #a :text"),
            Some(ChatEvent::Delete(id)) if id == "xyz"
        ));
        assert!(matches!(
            parse_line(":tmi.twitch.tv NOTICE * :Login authentication failed"),
            Some(ChatEvent::AuthFailed)
        ));
        assert!(matches!(
            parse_line("@msg-id=slow_on :tmi.twitch.tv NOTICE #a :This room is now in slow mode."),
            Some(ChatEvent::Notice(t)) if t.starts_with("This room")
        ));
        assert!(matches!(
            parse_line("@badges=broadcaster/1;color=#9146FF;display-name=Areadox :tmi.twitch.tv USERSTATE #areadox"),
            Some(ChatEvent::OwnState { display_name, badges, .. })
                if display_name == "Areadox" && badges == vec![Badge::Broadcaster]
        ));
    }
}
