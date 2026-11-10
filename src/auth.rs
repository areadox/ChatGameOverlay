//! Anmeldung bei Twitch über den „Device Code Flow“.
//!
//! Das Programm fragt bei Twitch einen Code an, der Benutzer bestätigt ihn im
//! Browser unter twitch.tv/activate – ein Passwort wird nie im Programm eingegeben.
//! Als „öffentliche“ Twitch-Anwendung braucht das Programm kein Client-Secret.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use eframe::egui::{Context, OpenUrl};
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;

/// Client-ID der Twitch-Anwendung (dev.twitch.tv/console, Typ „Public“).
/// Nicht geheim – sie steckt in jedem Fall in der fertigen .exe.
pub const CLIENT_ID: &str = "7n6qp3csdo5gf1n0nhk3hmgj73lp9e";

const SCOPES: &str = "chat:read chat:edit";
const DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";
const REVOKE_URL: &str = "https://id.twitch.tv/oauth2/revoke";

pub fn available() -> bool {
    !CLIENT_ID.is_empty()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Login-Name (klein geschrieben), wie ihn der Chat braucht
    pub login: String,
    /// Ablaufzeitpunkt des Zugangs-Tokens (Unix-Sekunden)
    pub expires_at: u64,
    /// Letzte Prüfung bei Twitch (Unix-Sekunden)
    #[serde(default)]
    pub validated_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthState {
    LoggedOut,
    Starting,
    /// Benutzer muss den Code im Browser bestätigen
    WaitingForUser { user_code: String, uri: String },
    LoggedIn { login: String },
    Error(String),
}

pub struct Inner {
    pub state: AuthState,
    pub tokens: Option<Tokens>,
    /// wird bei jeder Änderung der Zugangsdaten erhöht → Chat neu verbinden
    pub generation: u64,
}

#[derive(Clone)]
pub struct Auth {
    inner: Arc<Mutex<Inner>>,
    cancel: Arc<AtomicBool>,
    force_refresh: Arc<AtomicBool>,
    ctx: Context,
}

#[derive(Deserialize, Default)]
struct TwitchResp {
    // Device-Code
    device_code: Option<String>,
    user_code: Option<String>,
    verification_uri: Option<String>,
    interval: Option<u64>,
    // Token
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    // Validate
    login: Option<String>,
    // Fehler
    message: Option<String>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(format!("ChatGameOverlay/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// POST mit Formulardaten; liefert Statuscode und Antwort
fn post(url: &str, form: &[(&str, &str)]) -> Result<(u16, TwitchResp), String> {
    let mut resp = agent()
        .post(url)
        .send_form(form.iter().copied())
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_json::<TwitchResp>().unwrap_or_default();
    Ok((status, body))
}

/// Prüft ein Token bei Twitch; liefert den Login-Namen oder `None`, wenn ungültig
fn validate(token: &str) -> Result<Option<(String, u64)>, String> {
    let mut resp = agent()
        .get(VALIDATE_URL)
        .header("Authorization", &format!("OAuth {token}"))
        .call()
        .map_err(|e| e.to_string())?;
    if resp.status().as_u16() == 401 {
        return Ok(None);
    }
    let body = resp.body_mut().read_json::<TwitchResp>().unwrap_or_default();
    match body.login {
        Some(login) => Ok(Some((login, body.expires_in.unwrap_or(0)))),
        None => Err(body.message.unwrap_or_else(|| "validate".into())),
    }
}

impl Auth {
    pub fn new(ctx: Context, tokens: Option<Tokens>) -> Self {
        let state = match &tokens {
            Some(t) => AuthState::LoggedIn { login: t.login.clone() },
            None => AuthState::LoggedOut,
        };
        let auth = Self {
            inner: Arc::new(Mutex::new(Inner { state, tokens, generation: 0 })),
            cancel: Arc::new(AtomicBool::new(false)),
            force_refresh: Arc::new(AtomicBool::new(false)),
            ctx,
        };
        auth.spawn_keeper();
        auth
    }

    pub fn state(&self) -> AuthState {
        self.inner.lock().map(|g| g.state.clone()).unwrap_or(AuthState::LoggedOut)
    }

    pub fn generation(&self) -> u64 {
        self.inner.lock().map(|g| g.generation).unwrap_or(0)
    }

    /// Login-Name und Token für den Chat (falls angemeldet)
    pub fn credentials(&self) -> Option<(String, String)> {
        let g = self.inner.lock().ok()?;
        g.tokens.as_ref().map(|t| (t.login.clone(), t.access_token.clone()))
    }

    pub fn tokens(&self) -> Option<Tokens> {
        self.inner.lock().ok()?.tokens.clone()
    }

    fn set_state(&self, s: AuthState) {
        if let Ok(mut g) = self.inner.lock() {
            g.state = s;
        }
        self.ctx.request_repaint();
    }

    fn set_tokens(&self, tokens: Option<Tokens>) {
        if let Ok(mut g) = self.inner.lock() {
            g.state = match &tokens {
                Some(t) => AuthState::LoggedIn { login: t.login.clone() },
                None => AuthState::LoggedOut,
            };
            g.tokens = tokens;
            g.generation += 1;
        }
        self.ctx.request_repaint();
    }

    /// Chat meldet „Anmeldung fehlgeschlagen“ → Token sofort erneuern
    pub fn report_auth_failed(&self) {
        self.force_refresh.store(true, Ordering::Relaxed);
    }

    pub fn cancel_login(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.set_state(AuthState::LoggedOut);
    }

    pub fn start_login(&self, l: Lang) {
        if !available() {
            return;
        }
        self.cancel.store(false, Ordering::Relaxed);
        self.set_state(AuthState::Starting);
        let me = self.clone();
        thread::spawn(move || {
            if let Err(e) = me.login_flow(l)
                && !me.cancel.load(Ordering::Relaxed)
            {
                me.set_state(AuthState::Error(e));
            }
        });
    }

    fn login_flow(&self, l: Lang) -> Result<(), String> {
        let (status, dev) = post(DEVICE_URL, &[("client_id", CLIENT_ID), ("scopes", SCOPES)])
            .map_err(|e| format!("{} {e}", l.t("Twitch nicht erreichbar:", "Cannot reach Twitch:")))?;
        let (Some(device_code), Some(user_code), Some(uri)) =
            (dev.device_code, dev.user_code, dev.verification_uri)
        else {
            return Err(format!(
                "{} {} {}",
                l.t("Twitch lehnt die Anmeldung ab:", "Twitch refused the login:"),
                status,
                dev.message.unwrap_or_default()
            ));
        };
        let interval = dev.interval.unwrap_or(5).max(1);
        self.set_state(AuthState::WaitingForUser { user_code, uri: uri.clone() });
        self.ctx.open_url(OpenUrl::new_tab(uri));

        // warten, bis der Benutzer im Browser bestätigt (max. ~30 Minuten)
        let started = now();
        loop {
            for _ in 0..interval * 10 {
                if self.cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(100));
            }
            if now() - started > 1800 {
                return Err(l.t("Code abgelaufen – bitte neu anmelden.", "Code expired – please log in again.").into());
            }
            let (status, tok) = post(
                TOKEN_URL,
                &[
                    ("client_id", CLIENT_ID),
                    ("scopes", SCOPES),
                    ("device_code", &device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ],
            )?;
            if let (Some(access), Some(refresh)) = (tok.access_token, tok.refresh_token) {
                let (login, _) = validate(&access)?
                    .ok_or_else(|| l.t("Token ungültig", "Token invalid").to_string())?;
                self.set_tokens(Some(Tokens {
                    access_token: access,
                    refresh_token: refresh,
                    login,
                    expires_at: now() + tok.expires_in.unwrap_or(3600),
                    validated_at: now(),
                }));
                return Ok(());
            }
            let msg = tok.message.unwrap_or_default();
            match msg.as_str() {
                "authorization_pending" | "slow_down" => continue,
                "access_denied" => {
                    return Err(l.t("Anmeldung abgelehnt.", "Login was denied.").into());
                }
                "expired_token" | "invalid device code" => {
                    return Err(l.t("Code abgelaufen – bitte neu anmelden.", "Code expired – please log in again.").into());
                }
                _ => return Err(format!("Twitch: {status} {msg}")),
            }
        }
    }

    pub fn logout(&self) {
        if let Some(t) = self.tokens() {
            thread::spawn(move || {
                let _ = post(REVOKE_URL, &[("client_id", CLIENT_ID), ("token", &t.access_token)]);
            });
        }
        self.set_tokens(None);
    }

    /// Erneuert das Token. `false`, wenn der Benutzer sich neu anmelden muss.
    fn refresh(&self) -> bool {
        let Some(t) = self.tokens() else { return false };
        match post(
            TOKEN_URL,
            &[
                ("client_id", CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", &t.refresh_token),
            ],
        ) {
            Ok((_, r)) => match r.access_token {
                Some(access) => {
                    self.set_tokens(Some(Tokens {
                        access_token: access,
                        refresh_token: r.refresh_token.unwrap_or(t.refresh_token),
                        login: t.login,
                        expires_at: now() + r.expires_in.unwrap_or(3600),
                        validated_at: now(),
                    }));
                    true
                }
                None => false,
            },
            // Netzwerkfehler: Token behalten, später erneut versuchen
            Err(_) => true,
        }
    }

    /// Hintergrund-Wächter: prüft das Token stündlich (Vorgabe von Twitch)
    /// und erneuert es rechtzeitig vor Ablauf.
    fn spawn_keeper(&self) {
        let me = self.clone();
        thread::spawn(move || {
            loop {
                if let Some(t) = me.tokens() {
                    let forced = me.force_refresh.swap(false, Ordering::Relaxed);
                    let expiring = t.expires_at.saturating_sub(now()) < 600;
                    let validate_due = now().saturating_sub(t.validated_at) >= 3600;
                    let mut ok = true;
                    if forced || expiring {
                        ok = me.refresh();
                    } else if validate_due {
                        match validate(&t.access_token) {
                            Ok(Some(_)) => {
                                if let Ok(mut g) = me.inner.lock()
                                    && let Some(tk) = g.tokens.as_mut()
                                {
                                    tk.validated_at = now();
                                }
                            }
                            Ok(None) => ok = me.refresh(),
                            Err(_) => {} // offline – später erneut
                        }
                    }
                    if !ok {
                        // Erneuern nicht möglich → abmelden, Chat liest anonym weiter
                        me.set_tokens(None);
                        me.set_state(AuthState::Error(
                            "Twitch-Anmeldung abgelaufen – bitte neu anmelden. / \
                             Twitch login expired – please log in again."
                                .into(),
                        ));
                    }
                }
                for _ in 0..300 {
                    if me.force_refresh.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::Tokens;

    /// So speichert und lädt eframe die Anmeldung (siehe `save` / `OverlayApp::new`)
    #[test]
    fn tokens_survive_save_and_load() {
        let saved = Some(Tokens {
            access_token: "a".into(),
            refresh_token: "r".into(),
            login: "areadox".into(),
            expires_at: 9_999_999_999,
            validated_at: 1,
        });
        let text = ron::to_string(&saved).unwrap();
        let loaded: Option<Option<Tokens>> = ron::from_str(&text).ok();
        assert_eq!(loaded.flatten().unwrap().login, "areadox");
    }
}
