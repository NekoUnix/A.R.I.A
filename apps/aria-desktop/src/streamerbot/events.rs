//! Read-only local Streamer.bot event subscription. Remote data cannot select action targets.
use crate::event_rules::{Event, Kind, Platform};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    net::{Ipv4Addr, SocketAddr, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tungstenite::{Message, protocol::WebSocketConfig};

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub port: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self { port: 8080 }
    }
}
#[derive(Default)]
struct Feed {
    status: String,
    pending: VecDeque<(Instant, Event)>,
    recent: VecDeque<Event>,
    received: u64,
    dropped: u64,
    rejected: u64,
}
struct Connection {
    cancel: Arc<AtomicBool>,
    feed: Arc<Mutex<Feed>>,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
#[derive(Default)]
pub struct Bridge {
    connection: Option<Connection>,
    password: String,
    error: Option<String>,
}
impl Bridge {
    fn connect(&mut self, port: u16, ctx: egui::Context) -> Result<()> {
        ensure!(port > 0, "Choose a valid Streamer.bot port");
        self.connection = None;
        let cancel = Arc::new(AtomicBool::new(false));
        let feed = Arc::new(Mutex::new(Feed {
            status: "Connecting…".into(),
            ..Default::default()
        }));
        let c = cancel.clone();
        let f = feed.clone();
        let password = self.password.clone();
        std::thread::Builder::new()
            .name("aria-streamerbot-events".into())
            .spawn(move || {
                if let Err(e) = run(port, &password, &c, &f, &ctx) {
                    f.lock().unwrap().status = format!("Disconnected: {e}");
                } else {
                    f.lock().unwrap().status = "Disconnected".into();
                }
                ctx.request_repaint();
            })?;
        self.connection = Some(Connection { cancel, feed });
        self.error = None;
        Ok(())
    }
    pub fn drain(&mut self, enabled: bool, commands: bool) -> Vec<Event> {
        let Some(c) = &self.connection else {
            return vec![];
        };
        let mut feed = c.feed.lock().unwrap();
        if !enabled {
            feed.pending.clear();
            return vec![];
        }
        let mut result = Vec::new();
        for _ in 0..8 {
            let Some((at, event)) = feed.pending.pop_front() else {
                break;
            };
            if at.elapsed() > Duration::from_secs(5) {
                feed.dropped += 1;
                continue;
            }
            if event.kind != Kind::Command || commands {
                result.push(event);
            }
        }
        result
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &mut Settings) -> (bool, Option<Event>) {
        let old = settings.port;
        let mut selected = None;
        crate::theme::card(ui, |ui| {
            ui.heading("Streamer.bot event connection");
            ui.label("Sign in to Twitch / YouTube in Streamer.bot through their websites. Start its WebSocket Server under Servers/Clients, then connect here. No per-action script is needed for supported events.");
            ui.horizontal_wrapped(|ui| {
                ui.label("Local port");
                ui.add(egui::DragValue::new(&mut settings.port).range(1..=65535));
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .password(true)
                        .hint_text("Server password, if enabled")
                        .char_limit(512),
                );
            });
            ui.small("Password stays in memory for this session. Connection is limited to this PC (127.0.0.1).");
            ui.horizontal(|ui| {
                if ui.button("Connect / reconnect").clicked()
                    && let Err(e) = self.connect(settings.port, ui.ctx().clone())
                {
                    self.error = Some(e.to_string());
                }
                if ui.button("Disconnect").clicked() {
                    self.connection = None;
                    self.password.clear();
                }
            });
            if let Some(c) = &self.connection {
                let f = c.feed.lock().unwrap();
                ui.label(&f.status);
                ui.small(format!(
                    "{} received · {} pending · {} dropped / expired · {} invalid",
                    f.received,
                    f.pending.len(),
                    f.dropped,
                    f.rejected
                ));
                if !f.recent.is_empty() {
                    ui.separator();
                    ui.label("Create a reaction from an event");
                    ui.small(
                        "New rules start disabled. Choose a target and preview it before enabling.",
                    );
                    for event in &f.recent {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!(
                                "{:?} · {:?} · {} · {}{}",
                                event.platform,
                                event.kind,
                                event.name,
                                event.amount,
                                if event.test { " · test" } else { "" }
                            ));
                            if ui.small_button("Create reaction").clicked() {
                                selected = Some(event.clone());
                            }
                        });
                    }
                } else {
                    ui.small("Waiting for an event. A received reward appears here even while reactions are disabled.");
                }
            } else {
                ui.label("Disconnected");
            }
            if let Some(e) = &self.error {
                ui.colored_label(egui::Color32::YELLOW, e);
            }
            ui.collapsing("Supported events and matching",|ui| {ui.small("Twitch: rewards, follows, subscriptions, gift subs, bits and raids. YouTube: subscribers, memberships, gifts, Super Chats and stickers. Reward names match exactly; other event names match their provider type (for example SuperChat). Amount is a count, bits, viewers, or Super Chat tier—not currency.");});
            ui.small("TikTok and X require an external adapter that emits an ARIA custom event. They are not native account connections. Create Twitch channel rewards in Twitch or Streamer.bot; ARIA maps their reactions.");
            ui.hyperlink_to(
                "Connection and event guide",
                "https://docs.streamer.bot/api/websocket/guide",
            );
        });
        (old != settings.port, selected)
    }
    pub fn status(&self) -> String {
        self.connection
            .as_ref()
            .map_or("Streamer.bot disconnected".into(), |c| {
                c.feed.lock().unwrap().status.clone()
            })
    }
}
fn authentication(password: &str, salt: &str, challenge: &str) -> String {
    let secret = STANDARD.encode(Sha256::digest(format!("{password}{salt}")));
    STANDARD.encode(Sha256::digest(format!("{secret}{challenge}")))
}
fn subscription() -> Value {
    json!({"request":"Subscribe","id":"aria-events","events":{
        "Twitch":["RewardRedemption","Follow","Sub","ReSub","GiftSub","GiftBomb","Cheer","Raid"],
        "YouTube":["NewSubscriber","NewSponsor","MemberMileStone","MembershipGift","SuperChat","SuperSticker"],
        "General":["Custom"]
    }})
}
fn run(
    port: u16,
    password: &str,
    cancel: &AtomicBool,
    feed: &Mutex<Feed>,
    ctx: &egui::Context,
) -> Result<()> {
    let stream = TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_secs(3),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(256 * 1024))
        .max_frame_size(Some(256 * 1024));
    let (mut socket, _) = tungstenite::client::client_with_config(
        format!("ws://127.0.0.1:{port}/"),
        stream,
        Some(config),
    )?;
    socket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(100)))?;
    let start = Instant::now();
    let mut ready = false;
    let mut hello = false;
    let mut authenticating = false;
    let mut ping = Instant::now();
    while !cancel.load(Ordering::Relaxed) {
        ensure!(
            ready || start.elapsed() < Duration::from_secs(8),
            "Streamer.bot handshake timed out; check its server and password"
        );
        if ping.elapsed() > Duration::from_secs(10) {
            socket.send(Message::Ping(vec![].into()))?;
            ping = Instant::now();
        }
        let message = match socket.read() {
            Ok(m) => m,
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let Message::Text(text) = message else {
            if matches!(message, Message::Close(_)) {
                break;
            }
            socket.flush()?;
            continue;
        };
        let value: Value = serde_json::from_str(&text)?;
        if !hello && value["request"] == "Hello" {
            hello = true;
            if let Some(auth) = value.get("authentication").filter(|v| !v.is_null()) {
                ensure!(!password.is_empty(), "This server requires a password");
                let salt = auth["salt"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid authentication challenge"))?;
                let challenge = auth["challenge"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid authentication challenge"))?;
                socket.send(Message::Text(json!({"request":"Authenticate","id":"aria-auth","authentication":authentication(password,salt,challenge)}).to_string().into()))?;
                authenticating = true;
            } else {
                socket.send(Message::Text(subscription().to_string().into()))?;
            }
        } else if authenticating && value["id"] == "aria-auth" {
            ensure!(
                value["status"] == "ok",
                "Server authentication failed; check the password"
            );
            authenticating = false;
            socket.send(Message::Text(subscription().to_string().into()))?;
        } else if hello && !authenticating && value["id"] == "aria-events" {
            ensure!(
                value["status"] == "ok",
                "Server rejected event subscription"
            );
            ready = true;
            feed.lock().unwrap().status = "Connected · listening for events".into();
            ctx.request_repaint();
        } else if ready {
            match normalize(&value) {
                Ok(Some(event)) => {
                    let mut f = feed.lock().unwrap();
                    f.received += 1;
                    f.recent.retain(|e| {
                        e.platform != event.platform || e.kind != event.kind || e.name != event.name
                    });
                    f.recent.push_front(event.clone());
                    f.recent.truncate(12);
                    if f.pending.len() < 64 {
                        f.pending.push_back((Instant::now(), event));
                    } else {
                        f.dropped += 1;
                    }
                    ctx.request_repaint();
                }
                Err(_) => feed.lock().unwrap().rejected += 1,
                Ok(None) => {}
            }
        }
    }
    Ok(())
}
/// Explicit adapter envelope: General.Custom data.aria contains a bounded Event,
/// never an action, filename, command line or script. Tests remain opt-in per rule.
fn normalize(v: &Value) -> Result<Option<Event>> {
    let source = v["event"]["source"].as_str().unwrap_or("");
    let kind = v["event"]["type"].as_str().unwrap_or("");
    let d = &v["data"];
    if source == "General" && kind == "Custom" {
        // BroadcastJson delivers an object; Broadcast's CustomEvent uses a data string.
        let decoded;
        let payload =
            if let Some(text) = d.get("data").and_then(Value::as_str).or_else(|| d.as_str()) {
                decoded = serde_json::from_str::<Value>(text)?;
                &decoded
            } else {
                d
            };
        let Some(aria) = payload.get("aria") else {
            return Ok(None);
        };
        let event: Event = serde_json::from_value(aria.clone())?;
        ensure!(
            event.kind != Kind::Gesture,
            "External events cannot supply tracking gestures"
        );
        event.validate()?;
        return Ok(Some(event));
    }
    let platform = match source {
        "Twitch" => Platform::Twitch,
        "YouTube" => Platform::YouTube,
        _ => return Ok(None),
    };
    // Shared-chat guest activity must not trigger the host's paid reactions.
    if d["isFromSharedChatGuest"] == true {
        return Ok(None);
    }
    if platform == Platform::Twitch && kind == "GiftSub" && d["fromCommunitySubGift"] == true {
        return Ok(None);
    }
    let (event_kind, amount_field) = match (platform, kind) {
        (Platform::Twitch, "RewardRedemption") => (Kind::Reward, None),
        (Platform::Twitch, "Follow") | (Platform::YouTube, "NewSubscriber") => (Kind::Follow, None),
        (Platform::Twitch, "Sub" | "ReSub")
        | (Platform::YouTube, "NewSponsor" | "MemberMileStone") => (Kind::Subscription, None),
        (Platform::Twitch, "GiftSub") => (Kind::Gift, None),
        (Platform::Twitch, "GiftBomb") => (Kind::Gift, Some("total")),
        (Platform::YouTube, "MembershipGift") => (Kind::Gift, Some("count")),
        (Platform::Twitch, "Cheer") => (Kind::Bits, Some("bits")),
        (Platform::Twitch, "Raid") => (Kind::Raid, Some("viewers")),
        (Platform::YouTube, "SuperChat" | "SuperSticker") => (Kind::Tip, Some("tier")),
        _ => return Ok(None),
    };
    let name = if event_kind == Kind::Reward {
        ensure!(d["status"] != "CANCELED", "Canceled redemption");
        d["reward"]["title"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Reward has no title"))?
    } else {
        kind
    };
    let amount = if let Some(field) = amount_field {
        d[field]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 1_000_000)
            .ok_or_else(|| anyhow::anyhow!("Invalid event amount"))? as u32
    } else {
        1
    };
    let identity = ["redemptionId", "eventId", "messageId"]
        .iter()
        .find_map(|key| d[key].as_str().filter(|s| !s.is_empty()));
    // Hash stable provider ID when available, otherwise the complete timestamped envelope.
    let key = if let Some(id) = identity {
        format!("{source}:{kind}:{id}")
    } else {
        ensure!(
            v["timeStamp"].as_str().is_some_and(|s| !s.is_empty()),
            "Event has no identity or timestamp"
        );
        v.to_string()
    };
    let event = Event {
        platform,
        test: d["isTest"] == true || d["meta"]["isTest"] == true,
        id: format!("sb-{:x}", Sha256::digest(key)),
        kind: event_kind,
        name: name.into(),
        amount,
    };
    event.validate()?;
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an explicitly started local Streamer.bot research server"]
    fn native_streamerbot_subscription_acceptance() {
        let port = std::env::var("ARIA_TEST_STREAMERBOT_PORT")
            .expect("explicit test port")
            .parse()
            .unwrap();
        let feed = Arc::new(Mutex::new(Feed::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let f = feed.clone();
        let c = cancel.clone();
        let worker = std::thread::spawn(move || run(port, "", &c, &f, &egui::Context::default()));
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10)
            && !feed.lock().unwrap().status.starts_with("Connected")
            && !worker.is_finished()
        {
            std::thread::sleep(Duration::from_millis(25));
        }
        cancel.store(true, Ordering::Relaxed);
        worker.join().unwrap().unwrap();
        assert!(feed.lock().unwrap().status.starts_with("Connected"));
    }
    #[test]
    fn authentication_matches_independent_hash_and_gift_bundles_do_not_double_fire() {
        assert_eq!(
            authentication("password", "salt", "challenge"),
            "zTM5ki6L2vVvBQiTG9ckH1Lh64AbnCf6XZ226UmnkIA="
        );
        let mut event = json!({"event":{"source":"Twitch","type":"GiftBomb"},"data":{"messageId":"1","total":10}});
        assert_eq!(normalize(&event).unwrap().unwrap().amount, 10);
        event["event"]["type"] = json!("GiftSub");
        event["data"]["fromCommunitySubGift"] = json!(true);
        assert!(normalize(&event).unwrap().is_none());
        let payload = json!({"aria":{"id":"external-1","platform":"X","kind":"Custom","name":"Mention","amount":1}});
        let wrapped = json!({"event":{"source":"General","type":"Custom"},"data":{"data":payload.to_string()}});
        assert_eq!(normalize(&wrapped).unwrap().unwrap().platform, Platform::X);
    }
    #[test]
    fn websocket_handshake_auth_subscription_overload_and_disconnect() {
        for authenticated in [false, true] {
            let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut ws = tungstenite::accept(stream).unwrap();
                let mut hello = json!({"request":"Hello"});
                if authenticated {
                    hello["authentication"] = json!({"salt":"salt","challenge":"challenge"});
                }
                ws.send(Message::Text(hello.to_string().into())).unwrap();
                if authenticated {
                    let request: Value =
                        serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
                    assert_eq!(request["request"], "Authenticate");
                    assert_eq!(
                        request["authentication"],
                        authentication("password", "salt", "challenge")
                    );
                    assert!(!request.to_string().contains("password"));
                    ws.send(Message::Text(
                        json!({"id":"aria-auth","status":"ok"}).to_string().into(),
                    ))
                    .unwrap();
                }
                let request: Value =
                    serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
                assert_eq!(request, subscription());
                ws.send(Message::Text(
                    json!({"id":"aria-events","status":"ok"}).to_string().into(),
                ))
                .unwrap();
                ws.send(Message::Text(json!({"event":{"source":"Twitch","type":"Cheer"},"data":{"messageId":"invalid","bits":-5}}).to_string().into())).unwrap();
                for n in 0..80 {
                    ws.send(Message::Text(json!({"event":{"source":"Twitch","type":"RewardRedemption"},"data":{"redemptionId":format!("reward-{n}"),"reward":{"title":"Bonk"}}}).to_string().into())).unwrap();
                }
                ws.close(None).unwrap();
            });
            let feed = Mutex::new(Feed::default());
            run(
                port,
                "password",
                &AtomicBool::new(false),
                &feed,
                &egui::Context::default(),
            )
            .unwrap();
            server.join().unwrap();
            let feed = feed.lock().unwrap();
            assert_eq!(feed.pending.len(), 64);
            assert_eq!(feed.received, 80);
            assert_eq!(feed.dropped, 16);
            assert_eq!(feed.rejected, 1);
        }
    }
    #[test]
    fn websocket_rejects_failed_authentication_without_subscribing() {
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            ws.send(Message::Text(
                json!({"request":"Hello","authentication":{"salt":"a","challenge":"b"}})
                    .to_string()
                    .into(),
            ))
            .unwrap();
            assert!(
                ws.read()
                    .unwrap()
                    .to_text()
                    .unwrap()
                    .contains("Authenticate")
            );
            ws.send(Message::Text(
                json!({"id":"aria-auth","status":"error"})
                    .to_string()
                    .into(),
            ))
            .unwrap();
            assert!(ws.read().is_err());
        });
        let result = run(
            port,
            "wrong",
            &AtomicBool::new(false),
            &Mutex::new(Feed::default()),
            &egui::Context::default(),
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("authentication failed")
        );
        server.join().unwrap();
    }
    #[test]
    fn rewards_keep_stable_identity_and_never_treat_user_input_as_an_action() {
        let mut v = json!({"event":{"source":"Twitch","type":"RewardRedemption"},"data":{"redemptionId":"one","reward":{"title":"Bonk"},"userInput":"delete everything","isTest":true}});
        let a = normalize(&v).unwrap().unwrap();
        assert_eq!(a.name, "Bonk");
        assert!(a.test);
        assert_eq!(a.kind, Kind::Reward);
        v["timeStamp"] = json!("later");
        assert_eq!(a.id, normalize(&v).unwrap().unwrap().id);
        v["data"]["isFromSharedChatGuest"] = json!(true);
        assert!(normalize(&v).unwrap().is_none());
    }
    #[test]
    fn typed_amounts_and_custom_adapter_are_bounded() {
        let mut v = json!({"event":{"source":"YouTube","type":"SuperChat"},"data":{"eventId":"one","tier":3,"decimalAmount":500.0}});
        let e = normalize(&v).unwrap().unwrap();
        assert_eq!(e.amount, 3);
        assert_eq!(e.kind, Kind::Tip);
        v["data"]["tier"] = json!(-1);
        assert!(normalize(&v).is_err());
        let mut custom = json!({"event":{"source":"General","type":"Custom"},"data":{"aria":{"platform":"TikTok","kind":"Gift","id":"gift-1","name":"Rose","amount":5}}});
        assert_eq!(
            normalize(&custom).unwrap().unwrap().platform,
            Platform::TikTok
        );
        custom["data"]["aria"]["path"] = json!("evil.exe");
        assert!(normalize(&custom).is_err());
        assert!(
            normalize(&json!({"event":{"source":"Twitch","type":"Follow"},"data":{}})).is_err()
        );
    }
    #[test]
    fn disconnected_or_disabled_events_do_not_replay_and_backlog_expires() {
        let event = normalize(
            &json!({"event":{"source":"Twitch","type":"Follow"},"timeStamp":"now","data":{}}),
        )
        .unwrap()
        .unwrap();
        let feed = Arc::new(Mutex::new(Feed::default()));
        feed.lock()
            .unwrap()
            .pending
            .push_back((Instant::now() - Duration::from_secs(6), event.clone()));
        let mut bridge = Bridge {
            connection: Some(Connection {
                cancel: Arc::new(AtomicBool::new(false)),
                feed: feed.clone(),
            }),
            ..Default::default()
        };
        assert!(bridge.drain(true, true).is_empty());
        assert_eq!(feed.lock().unwrap().dropped, 1);
        feed.lock()
            .unwrap()
            .pending
            .push_back((Instant::now(), event));
        assert!(bridge.drain(false, true).is_empty());
        assert!(bridge.drain(true, true).is_empty());
    }
}
