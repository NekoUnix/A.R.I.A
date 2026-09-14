//! Bounded local diagnostic history and a human/machine-readable support report.
//! No network upload, artwork, account tokens, or raw camera/audio samples.
use eframe::egui;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock, mpsc},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
const EVENTS: usize = 2048;
const DISK_LIMIT: u64 = 2 * 1024 * 1024;
static LOG: OnceLock<Logger> = OnceLock::new();
#[derive(Clone, Serialize)]
struct Entry {
    unix_ms: u128,
    session_ms: u128,
    level: String,
    code: String,
    message: String,
}
struct Logger {
    start: Instant,
    session: String,
    entries: Mutex<VecDeque<Entry>>,
    disk: mpsc::SyncSender<Disk>,
    directory: PathBuf,
    disk_error: std::sync::Arc<Mutex<Option<String>>>,
}
enum Disk {
    Entry(Entry),
    Flush(mpsc::SyncSender<()>),
}
pub fn flush() {
    if let Some(logger) = LOG.get() {
        let (tx, rx) = mpsc::sync_channel(1);
        if logger.disk.try_send(Disk::Flush(tx)).is_ok() {
            let _ = rx.recv_timeout(std::time::Duration::from_millis(250));
        }
    }
}
fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
pub fn init() {
    if LOG.get().is_some() {
        return;
    }
    let directory = std::env::var_os("ARIA_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join("A.R.I.A")
            } else {
                std::env::var_os("XDG_STATE_HOME")
                    .map(PathBuf::from)
                    .or_else(|| {
                        std::env::var_os("HOME").map(|p| {
                            PathBuf::from(p).join(if cfg!(target_os = "macos") {
                                "Library/Logs"
                            } else {
                                ".local/state"
                            })
                        })
                    })
                    .unwrap_or_else(std::env::temp_dir)
                    .join("aria")
            }
        })
        .join("diagnostics");
    let session = format!("{}-{}", stamp(), std::process::id());
    let (tx, rx) = mpsc::sync_channel::<Disk>(256);
    let dir = directory.clone();
    let filename = format!("session-{session}.jsonl");
    let disk_error = std::sync::Arc::new(Mutex::new(None));
    let err = disk_error.clone();
    let _ = std::thread::Builder::new()
        .name("aria-diagnostic-log".into())
        .spawn(move || {
            let write = (|| -> anyhow::Result<()> {
                std::fs::create_dir_all(&dir)?;
                let mut files = session_files(&dir);
                files.sort();
                while files.len() > 2 {
                    let p = files.remove(0);
                    if p.symlink_metadata()?.file_type().is_file() {
                        std::fs::remove_file(p)?;
                    }
                }
                let path = dir.join(filename);
                let mut file = std::fs::File::create(&path)?;
                let mut size = 0;
                for command in rx {
                    let entry = match command {
                        Disk::Entry(e) => e,
                        Disk::Flush(reply) => {
                            file.flush()?;
                            let _ = reply.try_send(());
                            continue;
                        }
                    };
                    let mut line = serde_json::to_vec(&entry)?;
                    line.push(b'\n');
                    if size + line.len() as u64 > DISK_LIMIT {
                        file = std::fs::File::create(&path)?;
                        size = 0;
                    }
                    file.write_all(&line)?;
                    file.flush()?;
                    size += line.len() as u64;
                }
                Ok(())
            })();
            if let Err(e) = write
                && let Ok(mut slot) = err.lock()
            {
                *slot = Some(redact(&format!("Persistent log unavailable: {e:#}")));
            }
        });
    let _ = LOG.set(Logger {
        start: Instant::now(),
        session,
        entries: Mutex::new(VecDeque::new()),
        disk: tx,
        directory,
        disk_error,
    });
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .unwrap_or("Rust panic");
        record("fatal", "PANIC", message);
        if let Some(logger) = LOG.get() {
            let data = json!({"version":env!("CARGO_PKG_VERSION"),"session":logger.session,"unix_ms":stamp(),"code":"PANIC","message":redact(message),"location":info.location().map(|l|format!("{}:{}:{}",l.file(),l.line(),l.column())).map(|s|redact(&s)),"backtrace":redact(&std::backtrace::Backtrace::force_capture().to_string())});
            let _ = std::fs::write(
                logger.directory.join("last-panic.json"),
                serde_json::to_vec_pretty(&data).unwrap_or_default(),
            );
        }
        previous(info);
    }));
    record(
        "info",
        "SESSION_START",
        concat!("ARIA ", env!("CARGO_PKG_VERSION"), " started"),
    );
}
fn session_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            e.file_type().is_ok_and(|t| t.is_file())
                && e.file_name()
                    .to_str()
                    .is_some_and(|s| s.starts_with("session-") && s.ends_with(".jsonl"))
        })
        .map(|e| e.path())
        .collect()
}
pub fn record(level: &str, code: &str, message: &str) {
    let Some(logger) = LOG.get() else {
        return;
    };
    let e = Entry {
        unix_ms: stamp(),
        session_ms: logger.start.elapsed().as_millis(),
        level: level.chars().take(16).collect(),
        code: code.chars().take(80).collect(),
        message: redact(message),
    };
    if let Ok(mut entries) = logger.entries.lock() {
        if entries.len() == EVENTS {
            entries.pop_front();
        }
        entries.push_back(e.clone());
    }
    if logger.disk.try_send(Disk::Entry(e)).is_err()
        && let Ok(mut slot) = logger.disk_error.lock()
    {
        *slot=Some("Disk log queue was full or unavailable; current session report retains the in-memory history".into());
    }
}
/// Remove known personal roots, absolute paths, IP endpoints and credential lines.
/// Called at collection time and again on export, including recovered panic text.
pub fn redact(text: &str) -> String {
    let mut clean = String::new();
    for line in text.lines().take(160) {
        let lower = line.to_ascii_lowercase();
        if [
            "token",
            "authorization",
            "bearer ",
            "password",
            "client_secret",
            "oauth",
            "cookie",
            "api_key",
            "apikey",
        ]
        .iter()
        .any(|key| lower.contains(key))
        {
            clean.push_str("[credential-bearing detail omitted]\n");
            continue;
        }
        // Prefer readability of the diagnostic itself over leaking a path with spaces.
        let path_start = line.char_indices().find_map(|(i, c)| {
            let rest = &line[i..];
            ((c.is_ascii_alphabetic()
                && (rest.as_bytes().get(1..3) == Some(b":\\")
                    || rest.as_bytes().get(1..3) == Some(b":/")))
                || rest.starts_with("\\\\")
                || ["/Users/", "/home/", "/tmp/", "/private/", "/var/", "/mnt/"]
                    .iter()
                    .any(|p| rest.starts_with(p)))
            .then_some(i)
        });
        let line = path_start.map_or(line, |i| &line[..i]);
        for word in line.split_whitespace() {
            let candidate =
                word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != ':');
            if candidate.parse::<std::net::IpAddr>().is_ok()
                || candidate.parse::<std::net::SocketAddr>().is_ok()
            {
                clean.push_str("[network address]");
            } else {
                clean.push_str(word);
            }
            clean.push(' ');
        }
        if path_start.is_some() {
            clean.push_str("[local path omitted]");
        }
        clean.push('\n');
    }
    clean.trim().chars().take(4096).collect()
}
pub fn sanitize(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(redact(&s)),
        Value::Array(a) => Value::Array(a.into_iter().map(sanitize).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter()
                .filter(|(k, _)| {
                    ![
                        "token",
                        "password",
                        "cookie",
                        "secret",
                        "sender_ip",
                        "username",
                        "client_id",
                        "access_key",
                    ]
                    .iter()
                    .any(|s| k.to_ascii_lowercase().contains(s))
                })
                .map(|(k, v)| (k, sanitize(v)))
                .collect(),
        ),
        v => v,
    }
}
pub fn export(path: &Path, snapshot: Value, description: &str) -> anyhow::Result<()> {
    let logger = LOG
        .get()
        .ok_or_else(|| anyhow::anyhow!("Diagnostic logger is not initialized"))?;
    export_with(logger, path, snapshot, description)
}
fn export_with(
    logger: &Logger,
    path: &Path,
    snapshot: Value,
    description: &str,
) -> anyhow::Result<()> {
    let entries = logger
        .entries
        .lock()
        .map_err(|_| anyhow::anyhow!("Diagnostic history unavailable"))?
        .clone();
    let mut previous = Vec::new();
    let mut files = session_files(&logger.directory);
    files.sort();
    for p in files
        .into_iter()
        .rev()
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains(&logger.session))
        })
        .take(2)
    {
        let mut text = String::new();
        let read =
            std::fs::File::open(p).and_then(|f| f.take(DISK_LIMIT).read_to_string(&mut text));
        if read.is_err() {
            previous.push(json!({"code":"PREVIOUS_LOG_UNREADABLE","message":"An older local log could not be recovered"}));
            continue;
        }
        for line in text
            .lines()
            .rev()
            .take(256)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                previous.push(sanitize(v));
            }
        }
    }
    let panic = std::fs::File::open(logger.directory.join("last-panic.json"))
        .ok()
        .and_then(|f| {
            let mut b = Vec::new();
            f.take(64 * 1024).read_to_end(&mut b).ok()?;
            serde_json::from_slice::<Value>(&b).ok()
        })
        .map(sanitize);
    let report = json!({"schema":"aria-support-report/v1","version":env!("CARGO_PKG_VERSION"),"experimental_features":["VTube Studio import","VBridger import"],"session":logger.session,"created_unix_ms":stamp(),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"logical_processors":std::thread::available_parallelism().map_or(1,usize::from),"description":redact(description),"snapshot":sanitize(snapshot),"events":entries,"previous_session_events":previous,"last_recorded_panic":panic,"disk_logging_error":logger.disk_error.lock().ok().and_then(|e|e.clone())});
    let data = serde_json::to_string_pretty(&report)?;
    anyhow::ensure!(
        data.len() <= 20 * 1024 * 1024,
        "Report exceeds 20 MiB; reduce optional avatar detail"
    );
    let summary = format!(
        "# A.R.I.A. support report\n\nVersion: {} (Alpha)\nPlatform: {} / {}\nSession: {}\n\nWhat happened\n{}\n\nAttach this file to https://github.com/NekoUnix/A.R.I.A/issues/new/choose or create a support ticket in https://discord.gg/79GfpWtpct . Include expected behavior, actual behavior, reproduction steps, and whether it happens with other avatars. Review before sharing. This report is not uploaded automatically.\n\nNo artwork, moc3/VRM files, camera/audio recordings, account tokens or complete app settings are included. Model IDs, parameter IDs, topology and selected tuning metadata can still identify a rig.\n\nCodes: SESSION_START/END = app lifecycle; PANIC = crash; VTS_IMPORT/VTS_ACTION = compatibility import/action; MODEL = avatar load; TRACKING = receiver; GPU = rendering; API = local automation; HEALTH = component status. Times use UTC Unix milliseconds and milliseconds since session start. Repeated unchanged messages are collapsed by the collector.\n\n## Structured diagnostic data\n\n```json\n{data}\n```\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        logger.session,
        redact(description)
    );
    std::fs::write(path, summary)?;
    record("info", "REPORT_EXPORTED", "Support report saved by user");
    Ok(())
}
#[derive(Default)]
pub struct Panel {
    pub open: bool,
    description: String,
    pub exported: Option<PathBuf>,
    error: Option<String>,
    observed: BTreeMap<String, String>,
    pub snapshot: Value,
    last_poll: Option<Instant>,
}
impl Panel {
    pub fn due(&mut self) -> bool {
        if self
            .last_poll
            .is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(1))
        {
            return false;
        }
        self.last_poll = Some(Instant::now());
        true
    }
    pub fn observe(&mut self, code: &str, level: &str, message: Option<&str>) {
        let message = message.map(redact).unwrap_or_default();
        if self.observed.get(code) == Some(&message) {
            return;
        }
        self.observed.insert(code.into(), message.clone());
        if !message.is_empty() {
            record(level, code, &message);
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Diagnostics & support report").open(&mut open).default_width(670.).show(ctx,|ui|{
            ui.label("Reproduce the problem, describe what happened below, then export a report. It includes recent errors and avatar metadata to help explain model-specific failures.");
            ui.label("What did you do? What should have happened? What happened instead?");ui.add(egui::TextEdit::multiline(&mut self.description).char_limit(4000).desired_rows(4).desired_width(f32::INFINITY));
            ui.small("Local logs are bounded to three sessions (2 MiB each) plus a last-panic record. Reports include two previous session tails and current metadata. Personal paths and credential-bearing details are removed. No automatic upload.");
            egui::CollapsingHeader::new("Review avatar and system metadata before export").show(ui,|ui|{egui::ScrollArea::vertical().max_height(240.).show(ui,|ui|{ui.monospace(serde_json::to_string_pretty(&sanitize(self.snapshot.clone())).unwrap_or_default());});});
            if ui.button("Export support report…").clicked() && let Some(path)=rfd::FileDialog::new().set_file_name(format!("ARIA-{}-report.txt",env!("CARGO_PKG_VERSION"))).add_filter("Readable support report",&["txt"]).save_file(){match export(&path,self.snapshot.clone(),&self.description){Ok(())=>{self.exported=Some(path);self.error=None;},Err(e)=>self.error=Some(format!("Report not saved: {e:#}"))}}
            if let Some(e)=&self.error{ui.colored_label(crate::theme::orange(),e);}
            if let Some(path)=&self.exported{ui.separator();ui.strong("Report exported. Please open a support ticket.");ui.label(path.file_name().unwrap_or_default().to_string_lossy());ui.label("Attach the .txt report to a GitHub issue, or join NekoUnix’s Discord and create a ticket. Add steps to reproduce, expected/actual behavior, and whether other avatars have the same problem.");ui.horizontal(|ui|{ui.hyperlink_to("Create GitHub issue","https://github.com/NekoUnix/A.R.I.A/issues/new/choose");ui.hyperlink_to("Discord · create a ticket","https://discord.gg/79GfpWtpct");});}
        });
        self.open = open;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_roundtrip_recovers_prior_events_and_omits_private_data() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("session-0001.jsonl"),
            concat!(
                "{\"code\":\"MODEL\",\"message\":\"C:/Users/Alice/private.moc3\"}\n",
                "truncated-json\n",
                "{\"code\":\"GPU\",\"message\":\"Device lost\",\"token\":\"private-key\"}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("last-panic.json"),
            r#"{"message":"Oops in /home/alice/rig","code":"PANIC"}"#,
        )
        .unwrap();
        let (disk, _rx) = mpsc::sync_channel(1);
        let logger = Logger {
            start: Instant::now(),
            session: "current".into(),
            entries: Mutex::new(VecDeque::new()),
            disk,
            directory: dir.path().to_owned(),
            disk_error: Default::default(),
        };
        let path = dir.path().join("report.txt");
        export_with(&logger, &path, json!({"avatar":{"kind":"Live2D","parameters":[{"id":"ParamEyeLOpen","min":0.,"max":1.}]},"secret":"do-not-share"}), "Eye is stuck after the toggle").unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        for secret in ["Alice", "alice", "private-key", "do-not-share"] {
            assert!(!text.contains(secret));
        }
        assert!(text.contains("https://github.com/NekoUnix/A.R.I.A/issues/new/choose"));
        assert!(text.contains("https://discord.gg/79GfpWtpct"));
        let json = text
            .split("```json\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap();
        let v: Value = serde_json::from_str(json).unwrap();
        assert_eq!(v["schema"], "aria-support-report/v1");
        assert_eq!(v["previous_session_events"].as_array().unwrap().len(), 2);
        assert_eq!(
            v["snapshot"]["avatar"]["parameters"][0]["id"],
            "ParamEyeLOpen"
        );
        assert_eq!(v["last_recorded_panic"]["code"], "PANIC");
    }
    #[test]
    fn redacts_secrets_paths_addresses_and_nested_values() {
        let text = "Error 42 in C:\\Users\\Alice Smith\\Private Model\\x.moc3\nAuthorization: Bearer private-token\nPeer 192.168.0.7:11125 disconnected\n/home/alice/model.json missing";
        let r = redact(text);
        for secret in ["Alice", "private-token", "192.168.0.7", "/home/alice"] {
            assert!(!r.contains(secret));
        }
        assert!(r.contains("Error 42"));
        let v = sanitize(
            json!({"token":"secret","rig":{"parameters":[{"id":"ParamAngleX","min":-30}],"path":"C:/private/model"}}),
        );
        assert!(v.get("token").is_none());
        assert_eq!(v["rig"]["parameters"][0]["id"], "ParamAngleX");
    }
}
