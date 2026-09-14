//! Opt-in authenticated loopback bridge for Streamer.bot, Touch Portal and custom plugins.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 39421,
            token: String::new(),
        }
    }
}
impl Settings {
    pub fn regenerate(&mut self) -> anyhow::Result<()> {
        let mut bytes = [0_u8; 24];
        getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("Cannot create API key: {e}"))?;
        self.token = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Ok(())
    }
}
#[derive(Clone, PartialEq, Serialize)]
struct Entry {
    id: u64,
    name: String,
    kind: aria_core::effects::Kind,
}
#[derive(Default)]
struct Catalog {
    profile: String,
    entries: Vec<Entry>,
    state: serde_json::Value,
    generation: u64,
    next_ticket: u64,
    results: std::collections::BTreeMap<u64, serde_json::Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    SetParameters {
        values: std::collections::BTreeMap<String, f32>,
    },
    ReleaseParameters {
        ids: Vec<String>,
    },
    Pose {
        frozen: bool,
    },
    Preset {
        index: usize,
    },
    Expression {
        id: String,
        enabled: bool,
    },
    Output {
        index: usize,
        open: bool,
        zoom: Option<f32>,
        position: Option<[f32; 2]>,
    },
    Theme {
        name: String,
    },
    SaveProfile,
    #[serde(rename = "imported_action")]
    Imported {
        id: String,
    },
}
pub struct Command {
    pub profile: String,
    pub id: u64,
    pub action: Option<Action>,
    pub ticket: u64,
    pub generation: u64,
}
struct Server {
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    rx: mpsc::Receiver<Command>,
    catalog: Arc<Mutex<Catalog>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
#[derive(Default)]
pub struct Api {
    settings: Option<Settings>,
    server: Option<Server>,
    pub error: Option<String>,
}
impl Api {
    pub fn invalidate_model(&self) {
        if let Some(server) = &self.server {
            let mut c = server.catalog.lock().unwrap();
            c.generation = c.generation.wrapping_add(1);
            c.state = serde_json::Value::Null;
        }
    }
    pub fn publish(&self, state: serde_json::Value) {
        if let Some(server) = &self.server {
            server.catalog.lock().unwrap().state = state;
        }
    }
    pub fn is_current(&self, command: &Command) -> bool {
        self.server
            .as_ref()
            .is_some_and(|s| s.catalog.lock().unwrap().generation == command.generation)
    }
    pub fn complete(&self, ticket: u64, result: Result<(), String>) {
        if let Some(server) = &self.server {
            let mut c = server.catalog.lock().unwrap();
            c.results.insert(
                ticket,
                match result {
                    Ok(()) => serde_json::json!({"status":"applied"}),
                    Err(error) => serde_json::json!({"status":"rejected","error":error}),
                },
            );
            while c.results.len() > 128 {
                c.results.pop_first();
            }
        }
    }
    pub fn ui(&mut self, ui: &mut eframe::egui::Ui, settings: &mut Settings) {
        crate::help::label(ui, "Local control API", "api");
        if ui
            .checkbox(&mut settings.enabled, "Enable API on this PC")
            .changed()
            && settings.enabled
            && settings.token.is_empty()
            && let Err(e) = settings.regenerate()
        {
            settings.enabled = false;
            self.error = Some(e.to_string());
        }
        ui.horizontal(|ui| {
            ui.label("Port");
            ui.add(eframe::egui::DragValue::new(&mut settings.port).range(1024..=65535));
        });
        ui.monospace(format!("http://127.0.0.1:{}/v1/state", settings.port));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !settings.token.is_empty(),
                    eframe::egui::Button::new("Copy API key"),
                )
                .clicked()
            {
                ui.ctx().copy_text(settings.token.clone());
            }
            if ui.button("Rotate key").clicked()
                && let Err(e) = settings.regenerate()
            {
                self.error = Some(e.to_string());
            }
        });
        crate::theme::caption(
            ui,
            "Control parameters, freeze poses, apply presets and expressions, open outputs, switch themes, and trigger effects. Requests are limited to this PC and require your key. Rotating it disconnects existing integrations.",
        );
        if let Some(error) = &self.error {
            ui.label(error);
        }
        crate::theme::caption(
            ui,
            "GET /v1/state lists current IDs and a model generation. POST /v1/commands queues an action; GET /v1/commands/{ticket} reports whether it was applied. Examples are in templates/api and docs/api.md.",
        );
    }
    pub fn update(
        &mut self,
        settings: &Settings,
        profile: &str,
        library: &aria_core::effects::Library,
        ctx: &eframe::egui::Context,
    ) -> Vec<Command> {
        if self.settings.as_ref() != Some(settings) {
            self.server = None;
            self.settings = Some(settings.clone());
            self.error = None;
            if settings.enabled {
                match start(settings, ctx.clone()) {
                    Ok(s) => self.server = Some(s),
                    Err(e) => self.error = Some(format!("Plugin API: {e}")),
                }
            }
        }
        if let Some(server) = &self.server {
            let entries: Vec<_> = library
                .designs
                .iter()
                .map(|d| Entry {
                    id: d.id,
                    name: d.name.clone(),
                    kind: d.kind,
                })
                .collect();
            let mut cat = server.catalog.lock().unwrap();
            if cat.profile != profile {
                cat.generation = cat.generation.wrapping_add(1);
                cat.state = serde_json::Value::Null;
            }
            if cat.profile != profile || cat.entries != entries {
                cat.profile = profile.into();
                cat.entries = entries;
            }
            drop(cat);
            return server.rx.try_iter().collect();
        }
        vec![]
    }
}
fn start(settings: &Settings, ctx: eframe::egui::Context) -> anyhow::Result<Server> {
    anyhow::ensure!(
        settings.token.len() >= 32,
        "Generate an API key before enabling the bridge"
    );
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, settings.port))?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let catalog = Arc::new(Mutex::new(Catalog::default()));
    let (tx, rx) = mpsc::sync_channel(32);
    let key = settings.token.clone();
    let quit = stop.clone();
    let cat = catalog.clone();
    let worker = thread::Builder::new()
        .name("aria-effect-api".into())
        .spawn(move || {
            let mut recent = std::collections::VecDeque::new();
            while !quit.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        let now = Instant::now();
                        while recent.front().is_some_and(|t: &Instant| {
                            now.duration_since(*t) > Duration::from_secs(1)
                        }) {
                            recent.pop_front();
                        }
                        let result = if recent.len() >= 20 {
                            (429, serde_json::json!({"error": "Rate limit: 20 requests per second"}))
                        } else {
                            recent.push_back(now);
                            handle(&mut socket, &key, &cat, &tx)
                        };
                        let body = result.1.to_string();
                        let _ = socket.set_write_timeout(Some(Duration::from_millis(200)));
                        let _ = write!(socket,
                            "HTTP/1.1 {} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{}",
                            result.0, body.len(), body
                        );
                        if result.0 == 202 {
                            ctx.request_repaint();
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(15));
                    }
                    Err(_) => break,
                }
            }
        })?;
    Ok(Server {
        stop,
        worker: Some(worker),
        rx,
        catalog,
    })
}
fn handle(
    socket: &mut TcpStream,
    key: &str,
    catalog: &Mutex<Catalog>,
    tx: &mpsc::SyncSender<Command>,
) -> (u16, serde_json::Value) {
    match request(socket, key) {
        Err(e) => (
            400,
            serde_json::json!({
            "error":e.to_string()}
            ),
        ),
        Ok((method, path, body)) => {
            let mut cat = catalog.lock().unwrap();
            if method == "GET" && path == "/v1/capabilities" {
                return (
                    200,
                    serde_json::json!({"version":1,"app_version":env!("CARGO_PKG_VERSION"),"actions":["set_parameters","release_parameters","pose","preset","expression","output","theme","save_profile","imported_action"],"max_requests_per_second":20,"max_body_bytes":4096}),
                );
            }
            if method == "GET" && path == "/v1/state" {
                if cat.state.is_null() {
                    return (
                        503,
                        serde_json::json!({"error":"State is initializing; retry shortly"}),
                    );
                }
                return (
                    200,
                    serde_json::json!({"version":1,"generation":cat.generation,"state":cat.state}),
                );
            }
            if method == "GET"
                && let Some(ticket) = path
                    .strip_prefix("/v1/commands/")
                    .and_then(|s| s.parse::<u64>().ok())
            {
                return cat
                    .results
                    .get(&ticket)
                    .map(|r| (200, r.clone()))
                    .unwrap_or_else(|| {
                        (
                            404,
                            serde_json::json!({"error":"Unknown or expired ticket"}),
                        )
                    });
            }
            if method == "POST" && path == "/v1/commands" {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Request {
                    version: u32,
                    generation: u64,
                    action: Action,
                }
                let Ok(request) = serde_json::from_slice::<Request>(&body) else {
                    return (
                        400,
                        serde_json::json!({"error":"Expected version, generation and a supported action"}),
                    );
                };
                if request.version != 1
                    || request.generation != cat.generation
                    || cat.state.is_null()
                {
                    return (
                        409,
                        serde_json::json!({"error":"Model changed or state unavailable. Fetch /v1/state again."}),
                    );
                }
                cat.next_ticket += 1;
                let ticket = cat.next_ticket;
                let command = Command {
                    profile: cat.profile.clone(),
                    id: 0,
                    action: Some(request.action),
                    ticket,
                    generation: cat.generation,
                };
                return queue(&mut cat, tx, command);
            }
            if method == "GET" && path == "/v1/effects" {
                return (
                    200,
                    serde_json::json!({
                    "version":1,"effects":cat.entries}
                    ),
                );
            }
            if method != "POST" || path != "/v1/effects/trigger" {
                return (
                    404,
                    serde_json::json!({
                    "error":"Unknown endpoint"}
                    ),
                );
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Trigger {
                version: u32,
                id: u64,
            }
            let Ok(trigger) = serde_json::from_slice::<Trigger>(&body) else {
                return (
                    400,
                    serde_json::json!({
                    "error":"Expected version and saved effect id"}
                    ),
                );
            };
            if trigger.version != 1 || !cat.entries.iter().any(|e| e.id == trigger.id) {
                return (
                    404,
                    serde_json::json!({
                    "error":"Unknown effect or protocol version"}
                    ),
                );
            }
            cat.next_ticket += 1;
            let ticket = cat.next_ticket;
            let command = Command {
                profile: cat.profile.clone(),
                id: trigger.id,
                action: None,
                ticket,
                generation: cat.generation,
            };
            queue(&mut cat, tx, command)
        }
    }
}
fn queue(
    cat: &mut Catalog,
    tx: &mpsc::SyncSender<Command>,
    command: Command,
) -> (u16, serde_json::Value) {
    let ticket = command.ticket;
    let id = command.id;
    match tx.try_send(command) {
        Ok(()) => {
            cat.results
                .insert(ticket, serde_json::json!({"status":"queued"}));
            while cat.results.len() > 128 {
                cat.results.pop_first();
            }
            (
                202,
                serde_json::json!({"queued":true,"id":id,"ticket":ticket}),
            )
        }
        Err(_) => (429, serde_json::json!({"error":"Queue full"})),
    }
}
fn request(socket: &mut TcpStream, key: &str) -> anyhow::Result<(String, String, Vec<u8>)> {
    use anyhow::{Context, ensure};
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    let started = Instant::now();
    let mut bytes = Vec::new();
    let mut buf = [0_u8; 1024];
    let header_end;
    loop {
        ensure!(
            started.elapsed() < Duration::from_secs(1) && bytes.len() < 8192,
            "Request too large or slow"
        );
        let n = socket.read(&mut buf)?;
        ensure!(n > 0, "Incomplete request");
        bytes.extend_from_slice(&buf[..n]);
        if let Some(n) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
            header_end = n + 4;
            break;
        }
    }
    let header = std::str::from_utf8(&bytes[..header_end])?;
    ensure!(header_end <= 8192, "Header exceeds 8 KiB");
    let mut lines = header.lines();
    let mut first = lines
        .next()
        .context("Request line missing")?
        .split_whitespace();
    let method = first.next().context("Method missing")?.to_owned();
    let path = first.next().context("Path missing")?.to_owned();
    let mut length = None;
    let mut authorized = false;
    let mut expect_continue = false;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').context("Bad header")?;
        match name.to_ascii_lowercase().as_str() {
            "authorization" => authorized = value.trim() == format!("Bearer {key}"),
            "origin" => anyhow::bail!("Browser-origin requests are disabled"),
            "transfer-encoding" => anyhow::bail!("Chunked requests are unsupported"),
            "expect" => {
                ensure!(
                    value.trim().eq_ignore_ascii_case("100-continue"),
                    "Unsupported expectation"
                );
                expect_continue = true;
            }
            "content-length" => {
                ensure!(length.is_none(), "Duplicate length");
                length = Some(value.trim().parse::<usize>()?);
            }
            _ => {}
        }
    }
    ensure!(authorized, "Missing or invalid API key");
    let length = length.unwrap_or(0);
    ensure!(length <= 4096, "Body exceeds 4 KiB");
    if expect_continue && length > 0 {
        socket.set_write_timeout(Some(Duration::from_millis(200)))?;
        socket.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
    }
    while bytes.len() < header_end + length {
        ensure!(started.elapsed() < Duration::from_secs(1), "Slow request");
        let n = socket.read(&mut buf)?;
        ensure!(n > 0, "Incomplete body");
        bytes.extend_from_slice(&buf[..n]);
    }
    Ok((
        method,
        path,
        bytes[header_end..header_end + length].to_vec(),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn new_request(body: &str, generation: u64) -> ((u16, serde_json::Value), Option<Command>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::sync_channel(2);
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let cat = Mutex::new(Catalog {
                profile: "avatar-a".into(),
                generation,
                state: serde_json::json!({"parameters":[]}),
                ..Default::default()
            });
            handle(&mut socket, "test-key", &cat, &tx)
        });
        let mut socket = TcpStream::connect(address).unwrap();
        write!(socket,"POST /v1/commands HTTP/1.1\r\nAuthorization: Bearer test-key\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
        (worker.join().unwrap(), rx.try_recv().ok())
    }
    #[test]
    fn typed_commands_reject_stale_models_unknown_actions_and_extra_fields() {
        for (body, status) in [
            (
                r#"{"version":1,"generation":6,"action":{"type":"pose","frozen":true}}"#,
                409,
            ),
            (
                r#"{"version":1,"generation":7,"action":{"type":"execute","path":"bad.exe"}}"#,
                400,
            ),
            (
                r#"{"version":1,"generation":7,"action":{"type":"pose","frozen":true,"path":"bad.exe"}}"#,
                400,
            ),
            (
                r#"{"version":2,"generation":7,"action":{"type":"pose","frozen":true}}"#,
                409,
            ),
        ] {
            let (result, command) = new_request(body, 7);
            assert_eq!(result.0, status);
            assert!(command.is_none());
        }
        let (result, command) = new_request(
            r#"{"version":1,"generation":7,"action":{"type":"pose","frozen":true}}"#,
            7,
        );
        assert_eq!(result.0, 202);
        let command = command.unwrap();
        assert_eq!(command.generation, 7);
        assert_eq!(command.profile, "avatar-a");
        assert_eq!(result.1["ticket"], command.ticket);
        assert!(matches!(
            command.action,
            Some(Action::Pose { frozen: true })
        ));
    }
    #[test]
    fn rejects_missing_auth_browser_origins_unknown_ids_and_excess_body() {
        for (headers, body, expected) in [
            ("", r#"{"version":1,"id":7}"#, 400),
            (
                "Authorization: Bearer test-key\r\nOrigin: https://example.invalid\r\n",
                r#"{"version":1,"id":7}"#,
                400,
            ),
            (
                "Authorization: Bearer test-key\r\n",
                r#"{"version":1,"id":9}"#,
                404,
            ),
            (
                "Authorization: Bearer test-key\r\n",
                r#"{"version":1,"id":7,"path":"arbitrary.exe"}"#,
                400,
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let (tx, rx) = mpsc::sync_channel(2);
            let worker = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                handle(
                    &mut socket,
                    "test-key",
                    &Mutex::new(Catalog {
                        profile: "a".into(),

                        entries: vec![Entry {
                            id: 7,
                            name: "x".into(),
                            kind: Default::default(),
                        }],
                        ..Default::default()
                    }),
                    &tx,
                )
                .0
            });
            let mut c = TcpStream::connect(addr).unwrap();
            write!(
                c,
                "POST /v1/effects/trigger HTTP/1.1\r\n{headers}Content-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            assert_eq!(worker.join().unwrap(), expected);
            assert!(rx.try_recv().is_err());
        }
    }
    #[test]
    fn loopback_auth_catalog_and_trigger_preserve_profile_scope() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::sync_channel(2);
        let worker = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let cat = Mutex::new(Catalog {
                profile: "model-a".into(),
                entries: vec![Entry {
                    id: 7,
                    name: "Spray".into(),
                    kind: aria_core::effects::Kind::Spray,
                }],
                ..Default::default()
            });
            handle(&mut s, "test-key", &cat, &tx)
        });
        let mut c = TcpStream::connect(addr).unwrap();
        let body = r#"{"version":1,"id":7}"#;
        write!(c,"POST /v1/effects/trigger HTTP/1.1\r\nAuthorization: Bearer test-key\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
        assert_eq!(worker.join().unwrap().0, 202);
        let command = rx.recv().unwrap();
        assert_eq!(command.id, 7);
        assert_eq!(command.profile, "model-a");
    }
}
