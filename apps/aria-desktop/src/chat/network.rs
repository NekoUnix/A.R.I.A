//! One cancellable worker per service. HTTP and TLS never run on the render thread.
use super::{
    Account, Feed,
    auth::{self, Session},
    protocol,
};
use anyhow::{Context, Result, bail};
use eframe::egui;
use reqwest::{
    Url,
    blocking::{Client, Response},
};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tungstenite::{Message, stream::MaybeTlsStream};

const GOOGLE_SCOPE: &str = "https://www.googleapis.com/auth/youtube.readonly";
static ACTIVE_WORKERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Slot;
impl Drop for Slot {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::Relaxed);
    }
}
pub struct Worker {
    cancel: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Worker {
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }
    pub fn start(
        index: usize,
        account: Account,
        login: bool,
        feed: Arc<Mutex<Feed>>,
        ctx: egui::Context,
    ) -> Result<Self> {
        // A cancelled HTTP/TLS operation may take a few seconds to return.
        // Bound rapid reconnects without blocking the UI while sockets unwind.
        if ACTIVE_WORKERS
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                (n < 4).then_some(n + 1)
            })
            .is_err()
        {
            bail!(
                "Previous connections are still closing. Wait a few seconds, then Connect again."
            );
        }
        let slot = Slot;
        let cancel = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let stopped = finished.clone();
        let service = Service {
            index,
            account,
            cancel: cancel.clone(),
            feed,
            ctx,
        };
        std::thread::Builder::new()
            .name(format!("aria-chat-{index}"))
            .spawn(move || {
                let _slot = slot;
                let result = service.run(login);
                if let Err(error) = result {
                    service.status(&format!("{error}"));
                }
                service.edit(|feed| {
                    feed.connected = false;
                    feed.auth_url = None;
                    feed.code.clear();
                });
                stopped.store(true, Ordering::Relaxed);
            })
            .context("Could not start the chat connection worker.")?;
        Ok(Self { cancel, finished })
    }
}
struct Service {
    index: usize,
    account: Account,
    cancel: Arc<AtomicBool>,
    feed: Arc<Mutex<Feed>>,
    ctx: egui::Context,
}
#[derive(Debug)]
struct ApiError {
    status: u16,
    reason: String,
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Service returned HTTP {} ({}). {}",
            self.status,
            self.reason,
            match self.reason.as_str() {
                "quotaExceeded" | "dailyLimitExceeded" =>
                    "YouTube project quota is exhausted. Resume after its quota resets.",
                "liveChatEnded" => "This broadcast has ended; select the next live video.",
                "liveChatDisabled" => "Chat is disabled for this broadcast.",
                "accessNotConfigured" => "Enable YouTube Data API v3 in the Google project.",
                _ if self.status == 401 => "Sign in again.",
                _ if self.status == 403 => "Check account permission and the OAuth app setup.",
                _ => "Check the connection and retry.",
            }
        )
    }
}
impl std::error::Error for ApiError {}
fn response(response: Response) -> Result<Value> {
    let status = response.status();
    let mut bytes = Vec::new();
    response
        .take(4_194_305)
        .read_to_end(&mut bytes)
        .context("Could not read the service response.")?;
    if bytes.len() > 4_194_304 {
        bail!("Chat response exceeded the 4 MiB limit.");
    }
    let value: Value =
        serde_json::from_slice(&bytes).context("The service returned an invalid response.")?;
    if !status.is_success() {
        let reason = value["error"]["errors"][0]["reason"]
            .as_str()
            .or_else(|| value["error"].as_str())
            .or_else(|| {
                value["message"].as_str().filter(|s| {
                    matches!(
                        *s,
                        "authorization_pending" | "slow_down" | "access_denied" | "expired_token"
                    )
                })
            })
            .unwrap_or("request_failed");
        // Never display raw bodies, URLs, authorization headers or token values.
        let reason: String = reason
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .take(64)
            .collect();
        return Err(ApiError {
            status: status.as_u16(),
            reason,
        }
        .into());
    }
    Ok(value)
}
fn http(result: std::result::Result<Response, reqwest::Error>) -> Result<Value> {
    response(result.map_err(|_| {
        anyhow::anyhow!(
            "Could not reach the service. Check internet access, proxy and Windows clock."
        )
    })?)
}
fn permanent(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<ApiError>()
        .is_some_and(|e| (400..500).contains(&e.status) && e.status != 429)
}
impl Service {
    fn edit(&self, f: impl FnOnce(&mut Feed)) {
        if self.cancel.load(Ordering::Relaxed) {
            return;
        }
        f(&mut self.feed.lock().unwrap());
        self.ctx.request_repaint();
        self.ctx.request_repaint_of(super::viewport_id());
    }
    fn status(&self, status: &str) {
        self.edit(|f| f.status = status.into());
    }
    fn wait(&self, duration: Duration) -> Result<()> {
        let end = Instant::now() + duration;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                bail!("Disconnected.");
            }
            if Instant::now() >= end {
                return Ok(());
            }
            std::thread::sleep((end - Instant::now().min(end)).min(Duration::from_millis(100)));
        }
    }
    fn save(&self, session: &Session) -> Result<()> {
        let sealed = auth::seal(&serde_json::to_vec(session)?)?;
        self.edit(|f| f.sealed_session = sealed);
        Ok(())
    }
    fn run(&self, login: bool) -> Result<()> {
        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .connect_timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("ARIA-desktop-chat/0.16")
            .build()?;
        let mut session = if login {
            self.status("Waiting for browser sign-in…");
            if self.index == 0 {
                self.twitch_login(&client)?
            } else {
                self.google_login(&client)?
            }
        } else {
            let session: Session =
                serde_json::from_slice(&auth::unseal(&self.account.sealed_session)?)
                    .context("Saved login is invalid; sign in again.")?;
            if session.client_id != self.account.client_id || session.provider != self.index {
                bail!("OAuth app changed. Sign in again for this app.");
            }
            session
        };
        self.edit(|f| {
            f.auth_url = None;
            f.code.clear();
        });
        self.save(&session)?;
        if self.index == 0 {
            self.twitch(&client, &mut session)
        } else {
            self.youtube(&client, &mut session)
        }
    }
    fn token(&self, value: &Value, old: Option<&Session>) -> Result<Session> {
        let access = value["access_token"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 8192)
            .context("Service did not return an access token.")?;
        if access.chars().any(char::is_control) {
            bail!("Invalid access token returned by service.");
        }
        let refresh = value["refresh_token"]
            .as_str()
            .unwrap_or_else(|| old.map_or("", |s| s.refresh_token.as_str()));
        if refresh.len() > 8192 {
            bail!("Invalid refresh token returned by service.");
        }
        Ok(Session {
            provider: self.index,
            client_id: self.account.client_id.clone(),
            access_token: access.into(),
            refresh_token: refresh.into(),
            expires_at: auth::now()
                .saturating_add(value["expires_in"].as_u64().unwrap_or(3600).min(31_536_000)),
        })
    }
    fn secret(&self) -> Result<String> {
        if self.account.sealed_secret.is_empty() {
            return Ok(String::new());
        }
        String::from_utf8(auth::unseal(&self.account.sealed_secret)?)
            .context("Google desktop client secret is invalid. Import the credentials JSON again.")
    }
    fn refresh(&self, client: &Client, session: &mut Session, force: bool) -> Result<()> {
        if !force && session.expires_at > auth::now() + 90 {
            return Ok(());
        }
        if session.refresh_token.is_empty() {
            bail!("Login expired. Sign in again.");
        }
        let secret = if self.index == 1 {
            self.secret()?
        } else {
            String::new()
        };
        let mut form = vec![
            ("client_id", self.account.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", session.refresh_token.as_str()),
        ];
        if !secret.is_empty() {
            form.push(("client_secret", &secret));
        }
        let endpoint = if self.index == 0 {
            "https://id.twitch.tv/oauth2/token"
        } else {
            "https://oauth2.googleapis.com/token"
        };
        let value = http(client.post(endpoint).form(&form).send())?;
        *session = self.token(&value, Some(session))?;
        // Twitch device refresh tokens rotate; persist each replacement immediately.
        self.save(session)
    }
    fn twitch_login(&self, client: &Client) -> Result<Session> {
        let value = http(
            client
                .post("https://id.twitch.tv/oauth2/device")
                .form(&[
                    ("client_id", self.account.client_id.as_str()),
                    ("scopes", "chat:read"),
                ])
                .send(),
        )?;
        let device = value["device_code"]
            .as_str()
            .context("Twitch did not return a device code.")?;
        let url = value["verification_uri"]
            .as_str()
            .context("Twitch did not return a sign-in URL.")?;
        let parsed = Url::parse(url)?;
        if parsed.scheme() != "https"
            || !matches!(parsed.host_str(), Some("twitch.tv" | "www.twitch.tv"))
        {
            bail!("Unexpected Twitch sign-in URL.");
        }
        self.edit(|f| {
            f.auth_url = Some(url.into());
            f.code = protocol::clean(value["user_code"].as_str().unwrap_or_default(), 32);
        });
        let deadline = Instant::now()
            + Duration::from_secs(value["expires_in"].as_u64().unwrap_or(600).min(1800));
        let mut interval = value["interval"].as_u64().unwrap_or(5).max(5);
        while Instant::now() < deadline {
            self.wait(Duration::from_secs(interval))?;
            let result = http(
                client
                    .post("https://id.twitch.tv/oauth2/token")
                    .form(&[
                        ("client_id", self.account.client_id.as_str()),
                        ("scopes", "chat:read"),
                        ("device_code", device),
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ])
                    .send(),
            );
            match result {
                Ok(value) => return self.token(&value, None),
                Err(error) => {
                    // Twitch uses the message field for pending/slow-down responses.
                    if let Some(e) = error.downcast_ref::<ApiError>() {
                        if e.reason == "authorization_pending" {
                            continue;
                        }
                        if e.reason == "slow_down" {
                            interval = interval.saturating_add(5);
                            continue;
                        }
                    }
                    return Err(error);
                }
            }
        }
        bail!("Twitch sign-in expired. Click Sign in to request a new code.")
    }
    fn google_login(&self, client: &Client) -> Result<Session> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .context("Could not open Google's local sign-in callback.")?;
        listener.set_nonblocking(true)?;
        let host = listener.local_addr()?.to_string();
        let redirect = format!("http://{host}/oauth/callback");
        let state = auth::nonce()?;
        let verifier = auth::nonce()?;
        let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth")?;
        url.query_pairs_mut().extend_pairs(&[
            ("client_id", self.account.client_id.as_str()),
            ("redirect_uri", &redirect),
            ("response_type", "code"),
            ("scope", GOOGLE_SCOPE),
            ("state", &state),
            ("code_challenge", &auth::challenge(&verifier)),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ]);
        self.edit(|f| f.auth_url = Some(url.to_string()));
        let deadline = Instant::now() + Duration::from_secs(300);
        while Instant::now() < deadline {
            self.wait(Duration::from_millis(100))?;
            let Ok((mut stream, _)) = listener.accept() else {
                continue;
            };
            stream.set_read_timeout(Some(Duration::from_millis(500)))?;
            stream.set_write_timeout(Some(Duration::from_millis(500)))?;
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 1024];
            let header_deadline = Instant::now() + Duration::from_secs(2);
            while bytes.len() < 16_384
                && Instant::now() < header_deadline
                && !bytes.windows(4).any(|w| w == b"\r\n\r\n")
            {
                self.wait(Duration::ZERO)?;
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                }
            }
            if bytes.len() > 16_384 || !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                continue;
            }
            let Some(code) = auth::callback(&String::from_utf8_lossy(&bytes), &host, &state)?
            else {
                let _ = stream.write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                continue;
            };
            let page =
                "Sign-in received. Return to ARIA to finish connecting. You can close this tab.";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                page.len()
            );
            let secret = self.secret()?;
            let mut form = vec![
                ("client_id", self.account.client_id.as_str()),
                ("code", &code),
                ("redirect_uri", &redirect),
                ("grant_type", "authorization_code"),
                ("code_verifier", &verifier),
            ];
            if !secret.is_empty() {
                form.push(("client_secret", &secret));
            }
            let value = http(
                client
                    .post("https://oauth2.googleapis.com/token")
                    .form(&form)
                    .send(),
            )?;
            return self.token(&value, None);
        }
        bail!("Google sign-in timed out after five minutes. Try again.")
    }
    fn twitch_identity(&self, client: &Client, session: &mut Session) -> Result<String> {
        self.refresh(client, session, false)?;
        let validate = |s: &Session| {
            http(
                client
                    .get("https://id.twitch.tv/oauth2/validate")
                    .header("Authorization", format!("OAuth {}", s.access_token))
                    .send(),
            )
        };
        let value = match validate(session) {
            Err(e)
                if e.downcast_ref::<ApiError>()
                    .is_some_and(|e| e.status == 401) =>
            {
                self.refresh(client, session, true)?;
                validate(session)?
            }
            result => result?,
        };
        if value["client_id"].as_str() != Some(&self.account.client_id)
            || !value["scopes"]
                .as_array()
                .is_some_and(|s| s.iter().any(|v| v == "chat:read"))
        {
            bail!(
                "Twitch login is missing chat read permission or belongs to another app. Sign in again."
            );
        }
        let login = protocol::channel(
            value["login"]
                .as_str()
                .context("Twitch account is unavailable.")?,
        )?;
        self.edit(|f| f.account_name = login.clone());
        Ok(login)
    }
    fn twitch(&self, client: &Client, session: &mut Session) -> Result<()> {
        let mut failures = 0u32;
        loop {
            self.wait(Duration::ZERO)?;
            let login = self.twitch_identity(client, session)?;
            let channel = if self.account.target.trim().is_empty() {
                login.clone()
            } else {
                protocol::channel(&self.account.target)?
            };
            self.edit(|f| {
                f.official_url = Some(format!(
                    "https://www.twitch.tv/popout/{channel}/chat?popout="
                ))
            });
            self.status(&format!("Connecting to #{channel}…"));
            let started = Instant::now();
            let result = self.twitch_socket(&login, &channel, session);
            self.edit(|f| f.connected = false);
            match result {
                Ok(()) => {
                    failures = 0;
                } // Hourly token validation / planned refresh.
                Err(error) => {
                    if permanent(&error) {
                        return Err(error);
                    }
                    if started.elapsed() > Duration::from_secs(60) {
                        failures = 0;
                    }
                    failures += 1;
                    if failures >= 6 {
                        return Err(error.context("Twitch connection stopped after repeated failures. Click Connect to retry."));
                    }
                    let seconds = (1u64 << failures).min(30);
                    self.status(&format!(
                        "Twitch connection interrupted. Retrying in {seconds}s…"
                    ));
                    self.wait(Duration::from_secs(seconds))?;
                }
            }
        }
    }
    fn twitch_socket(&self, login: &str, channel: &str, session: &Session) -> Result<()> {
        let addresses = ("irc-ws.chat.twitch.tv", 443)
            .to_socket_addrs()
            .context("Could not resolve Twitch chat.")?;
        let mut stream = None;
        for address in addresses.take(4) {
            self.wait(Duration::ZERO)?;
            if let Ok(s) = TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
                stream = Some(s);
                break;
            }
        }
        let stream = stream.context("Could not reach Twitch chat.")?;
        stream.set_read_timeout(Some(Duration::from_secs(8)))?;
        stream.set_write_timeout(Some(Duration::from_secs(8)))?;
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(262_144))
            .max_frame_size(Some(262_144));
        let (mut socket, _) = tungstenite::client_tls_with_config(
            "wss://irc-ws.chat.twitch.tv:443",
            stream,
            Some(config),
            None,
        )
        .map_err(|_| anyhow::anyhow!("Twitch TLS connection failed."))?;
        match socket.get_mut() {
            MaybeTlsStream::NativeTls(s) => s
                .get_mut()
                .set_read_timeout(Some(Duration::from_millis(250)))?,
            _ => bail!("Twitch chat requires a secure TLS connection."),
        }
        socket.send(Message::Text(format!("PASS oauth:{}\r\nNICK {login}\r\nCAP REQ :twitch.tv/tags twitch.tv/commands\r\nJOIN #{channel}\r\n",session.access_token).into())).map_err(|_|anyhow::anyhow!("Could not authenticate to Twitch chat."))?;
        let start = Instant::now();
        let mut last_message = Instant::now();
        let mut joined = false;
        loop {
            self.wait(Duration::ZERO)?;
            if start.elapsed() > Duration::from_secs(3500) || session.expires_at < auth::now() + 90
            {
                return Ok(());
            }
            if (!joined && start.elapsed() > Duration::from_secs(20))
                || last_message.elapsed() > Duration::from_secs(360)
            {
                bail!("Twitch chat stopped responding.");
            }
            match socket.read() {
                Ok(Message::Text(text)) => {
                    last_message = Instant::now();
                    for line in text.split("\r\n") {
                        if let Some(payload) = line.strip_prefix("PING ") {
                            socket
                                .send(Message::Text(format!("PONG {payload}\r\n").into()))
                                .map_err(|_| anyhow::anyhow!("Twitch keepalive failed."))?;
                        }
                        if protocol::command(line) == "RECONNECT" {
                            bail!("Twitch requested reconnection.");
                        }
                        if protocol::command(line) == "NOTICE"
                            && (line.contains("Login authentication failed")
                                || line.contains("Improperly formatted auth"))
                        {
                            return Err(ApiError {
                                status: 401,
                                reason: "invalid_login".into(),
                            }
                            .into());
                        }
                        if protocol::command(line) == "ROOMSTATE"
                            && line.contains(&format!(" ROOMSTATE #{channel}"))
                        {
                            joined = true;
                            self.edit(|f| {
                                f.connected = true;
                                f.status = format!("Live · #{channel}");
                            });
                        }
                        if let Some(change) = protocol::irc(line) {
                            self.edit(|f| protocol::apply(&mut f.messages, change));
                        }
                    }
                }
                Ok(Message::Ping(_)) => {
                    socket
                        .flush()
                        .map_err(|_| anyhow::anyhow!("Twitch keepalive failed."))?;
                }
                Ok(Message::Close(_)) => bail!("Twitch closed the connection."),
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => bail!("Twitch connection interrupted."),
                _ => {}
            }
        }
    }
    fn google_get(
        &self,
        client: &Client,
        session: &mut Session,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        self.refresh(client, session, false)?;
        let request = |s: &Session| {
            http(
                client
                    .get(format!("https://www.googleapis.com/youtube/v3/{path}"))
                    .bearer_auth(&s.access_token)
                    .query(query)
                    .send(),
            )
        };
        match request(session) {
            Err(error)
                if error
                    .downcast_ref::<ApiError>()
                    .is_some_and(|e| e.status == 401) =>
            {
                self.refresh(client, session, true)?;
                request(session)
            }
            result => result,
        }
    }
    fn youtube(&self, client: &Client, session: &mut Session) -> Result<()> {
        let identity = self.google_get(
            client,
            session,
            "channels",
            &[("part", "snippet"), ("mine", "true")],
        )?;
        self.edit(|f| {
            f.account_name = protocol::clean(
                identity["items"][0]["snippet"]["title"]
                    .as_str()
                    .unwrap_or("YouTube account"),
                80,
            )
        });
        let (chat_id, video) = if self.account.target.trim().is_empty() {
            let value = self.google_get(
                client,
                session,
                "liveBroadcasts",
                &[
                    ("part", "snippet"),
                    ("broadcastStatus", "active"),
                    ("broadcastType", "all"),
                    ("maxResults", "50"),
                ],
            )?;
            let items = value["items"]
                .as_array()
                .context("YouTube returned no broadcasts.")?;
            if items.len() > 1 {
                bail!(
                    "Multiple active broadcasts found. Paste the desired live video URL and connect again."
                );
            }
            let item=items.first().context("No active broadcast found on this account. Go live, or paste a live video URL and reconnect.")?;
            (
                item["snippet"]["liveChatId"]
                    .as_str()
                    .context("The active broadcast has no chat.")?
                    .to_owned(),
                item["id"].as_str().unwrap_or_default().to_owned(),
            )
        } else {
            let video = protocol::video_id(&self.account.target)?;
            let value = self.google_get(
                client,
                session,
                "videos",
                &[("part", "liveStreamingDetails"), ("id", &video)],
            )?;
            let chat=value["items"][0]["liveStreamingDetails"]["activeLiveChatId"].as_str().context("This video has no active live chat. Check the URL and that chat is enabled on a live broadcast.")?;
            (chat.to_owned(), video)
        };
        self.edit(|f| {
            f.official_url = Some(format!(
                "https://www.youtube.com/live_chat?v={video}&is_popout=1"
            ))
        });
        let mut page = String::new();
        let mut failures = 0u32;
        loop {
            self.wait(Duration::ZERO)?;
            let mut query = vec![
                ("part", "snippet,authorDetails"),
                ("liveChatId", chat_id.as_str()),
                ("maxResults", "200"),
            ];
            if !page.is_empty() {
                query.push(("pageToken", &page));
            }
            let value = match self.google_get(client, session, "liveChat/messages", &query) {
                Ok(v) => {
                    failures = 0;
                    v
                }
                Err(e) => {
                    if permanent(&e) || failures >= 5 {
                        return Err(e);
                    }
                    failures += 1;
                    let seconds = (1u64 << failures).min(60);
                    self.edit(|f| {
                        f.connected = false;
                        f.status =
                            format!("YouTube connection interrupted. Retrying in {seconds}s…");
                    });
                    self.wait(Duration::from_secs(seconds))?;
                    continue;
                }
            };
            let changes = protocol::youtube(&value);
            self.edit(|f| {
                f.connected = true;
                f.status = "Live · YouTube chat".into();
                for change in changes {
                    protocol::apply(&mut f.messages, change);
                }
            });
            if value["offlineAt"].is_string() {
                bail!("YouTube broadcast ended. Select the next live video to reconnect.");
            }
            page = value["nextPageToken"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            // Never poll faster than the server interval. A 5s floor reduces quota use.
            let millis = value["pollingIntervalMillis"]
                .as_u64()
                .unwrap_or(5000)
                .max(5000);
            self.wait(Duration::from_millis(millis.min(86_400_000)))?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_and_auth_errors_stop_retries() {
        assert!(permanent(
            &ApiError {
                status: 403,
                reason: "quotaExceeded".into()
            }
            .into()
        ));
        assert!(!permanent(
            &ApiError {
                status: 503,
                reason: "unavailable".into()
            }
            .into()
        ));
        assert!(!permanent(
            &ApiError {
                status: 429,
                reason: "rateLimitExceeded".into()
            }
            .into()
        ));
    }
    #[test]
    fn oauth_tokens_rotate_refresh_tokens_without_losing_google_refresh() {
        let service = Service {
            index: 0,
            account: Account {
                client_id: "test-client".into(),
                ..Default::default()
            },
            cancel: Arc::new(AtomicBool::new(false)),
            feed: Arc::new(Mutex::new(Feed::default())),
            ctx: egui::Context::default(),
        };
        let old = service
            .token(
                &serde_json::json!({"access_token":"a","refresh_token":"r1","expires_in":3600}),
                None,
            )
            .unwrap();
        let refreshed = service
            .token(
                &serde_json::json!({"access_token":"b","refresh_token":"r2","expires_in":3600}),
                Some(&old),
            )
            .unwrap();
        assert_eq!(refreshed.refresh_token, "r2");
        assert_eq!(
            service
                .token(&serde_json::json!({"access_token":"c"}), Some(&refreshed))
                .unwrap()
                .refresh_token,
            "r2"
        );
        assert!(
            service
                .token(&serde_json::json!({"access_token":"bad\r\nJOIN"}), None)
                .is_err()
        );
    }
    #[test]
    fn cancelled_worker_cannot_publish_login_or_messages() {
        let feed = Arc::new(Mutex::new(Feed::default()));
        let service = Service {
            index: 0,
            account: Account::default(),
            cancel: Arc::new(AtomicBool::new(true)),
            feed: feed.clone(),
            ctx: egui::Context::default(),
        };
        service.edit(|f| f.sealed_session = "stale token".into());
        assert!(feed.lock().unwrap().sealed_session.is_empty());
        assert!(service.wait(Duration::from_secs(30)).is_err());
    }
    #[test]
    #[ignore = "Requires a local TCP listener"]
    fn http_fixture_preserves_twitch_pending_and_youtube_quota() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for body in [
                r#"{"status":400,"message":"authorization_pending"}"#,
                r#"{"error":{"errors":[{"reason":"quotaExceeded"}]}}"#,
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buffer = [0; 4096];
                let _ = stream.read(&mut buffer);
                write!(stream,"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        for expected in ["authorization_pending", "quotaExceeded"] {
            let error = http(client.get(format!("http://{addr}/fixture")).send()).unwrap_err();
            assert_eq!(error.downcast_ref::<ApiError>().unwrap().reason, expected);
        }
        server.join().unwrap();
    }
}
