//! Twitch and YouTube chat companion for the portrait preview.
mod auth;
mod network;
mod protocol;
use crate::{help, output::OutputWindows, theme};
use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

const NAMES: [&str; 2] = ["Twitch", "YouTube"];
pub const TITLE: &str = "A.R.I.A. Chat — Twitch + YouTube";
pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("aria-stream-chat")
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Account {
    pub client_id: String,
    pub target: String,
    pub sealed_session: String,
    pub sealed_secret: String,
    pub remember: bool,
}
impl Default for Account {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            target: String::new(),
            sealed_session: String::new(),
            sealed_secret: String::new(),
            remember: true,
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    pub services: [Account; 2],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub visible: bool,
    pub background: [u8; 3],
    pub background_opacity: f32,
    pub opacity: f32,
    pub text_color: [u8; 3],
    pub font_size: f32,
    pub height: f32,
    pub name_colors: bool,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            visible: true,
            background: [18, 23, 32],
            background_opacity: 0.85,
            opacity: 1.0,
            text_color: [239, 244, 250],
            font_size: 15.0,
            height: 230.0,
            name_colors: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub styles: [Style; 2],
    pub docked: bool,
    pub always_on_top: bool,
    pub width: f32,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            styles: std::array::from_fn(|_| Style::default()),
            docked: true,
            always_on_top: false,
            width: 360.0,
        }
    }
}
fn bounded(value: f32, min: f32, max: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}
impl Appearance {
    pub fn sanitize(&mut self) {
        self.width = bounded(self.width, 280.0, 900.0, 360.0);
        for style in &mut self.styles {
            style.opacity = bounded(style.opacity, 0.0, 1.0, 1.0);
            style.background_opacity = bounded(style.background_opacity, 0.0, 1.0, 0.85);
            style.font_size = bounded(style.font_size, 10.0, 30.0, 15.0);
            style.height = bounded(style.height, 140.0, 600.0, 230.0);
        }
    }
}
struct Feed {
    messages: VecDeque<protocol::Message>,
    status: String,
    account_name: String,
    connected: bool,
    auth_url: Option<String>,
    code: String,
    sealed_session: String,
    official_url: Option<String>,
}
impl Default for Feed {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
            status: "Not connected · configure sign-in in Streaming chat".into(),
            account_name: String::new(),
            connected: false,
            auth_url: None,
            code: String::new(),
            sealed_session: String::new(),
            official_url: None,
        }
    }
}
pub struct Chats {
    feeds: [Arc<Mutex<Feed>>; 2],
    workers: [Option<network::Worker>; 2],
    open: Arc<AtomicBool>,
    hex: [String; 2],
    last_auth_url: [String; 2],
    secret_draft: String,
    dirty: bool,
}
impl Default for Chats {
    fn default() -> Self {
        Self {
            feeds: std::array::from_fn(|_| Arc::new(Mutex::new(Feed::default()))),
            workers: [None, None],
            open: Arc::new(AtomicBool::new(false)),
            hex: std::array::from_fn(|_| "#121720".into()),
            last_auth_url: [String::new(), String::new()],
            secret_draft: String::new(),
            dirty: false,
        }
    }
}
impl Chats {
    fn stop(&mut self, index: usize, account: &Account, logout: bool) {
        let session = self.feeds[index].lock().unwrap().sealed_session.clone();
        self.workers[index] = None; // Cancel before replacing the feed; stale events cannot reach the UI.
        self.feeds[index] = Arc::new(Mutex::new(Feed {
            status: if logout {
                "Signed out.".into()
            } else {
                "Disconnected. Click Connect to resume.".into()
            },
            sealed_session: if logout {
                String::new()
            } else if session.is_empty() {
                account.sealed_session.clone()
            } else {
                session
            },
            ..Default::default()
        }));
        self.last_auth_url[index].clear();
    }
    fn start(&mut self, index: usize, account: &Account, login: bool, ctx: &egui::Context) {
        self.stop(index, account, false);
        if !cfg!(windows) {
            self.feeds[index].lock().unwrap().status =
                "Chat login currently requires Windows.".into();
            return;
        }
        if account.client_id.trim().is_empty()
            || account.client_id.len() > 256
            || account.client_id.chars().any(char::is_whitespace)
        {
            self.feeds[index].lock().unwrap().status =
                "Configure a valid OAuth client ID in Account setup first.".into();
            return;
        }
        if !account.target.trim().is_empty() {
            let result = if index == 0 {
                protocol::channel(&account.target)
            } else {
                protocol::video_id(&account.target)
            };
            if let Err(error) = result {
                self.feeds[index].lock().unwrap().status = error.to_string();
                return;
            }
        }
        let mut account = account.clone();
        // A session-only login remains usable until Sign out or app exit.
        if !login && account.sealed_session.is_empty() {
            account.sealed_session = self.feeds[index].lock().unwrap().sealed_session.clone();
        }
        self.feeds[index].lock().unwrap().status = "Starting connection…".into();
        match network::Worker::start(
            index,
            account,
            login,
            self.feeds[index].clone(),
            ctx.clone(),
        ) {
            Ok(worker) => self.workers[index] = Some(worker),
            Err(error) => self.feeds[index].lock().unwrap().status = error.to_string(),
        }
    }
    pub fn update(&mut self, accounts: &mut Accounts, ctx: &egui::Context) -> bool {
        let mut dirty = std::mem::take(&mut self.dirty);
        for (i, account) in accounts.services.iter_mut().enumerate() {
            let feed = self.feeds[i].lock().unwrap();
            if account.remember
                && !feed.sealed_session.is_empty()
                && account.sealed_session != feed.sealed_session
            {
                account.sealed_session = feed.sealed_session.clone();
                dirty = true;
            }
            if let Some(url) = &feed.auth_url
                && url != &self.last_auth_url[i]
            {
                // Only URLs constructed/validated by our own OAuth worker reach this path.
                ctx.open_url(egui::OpenUrl::new_tab(url));
                self.last_auth_url[i] = url.clone();
            }
        }
        dirty
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, accounts: &mut Accounts, outputs: &OutputWindows) {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        let mut appearance = outputs.snapshot().chat;
        let before = appearance.clone();
        help::label(ui, "Chat companion", "stream-chat");
        theme::caption(
            ui,
            "Show Twitch, YouTube, or both below the portrait preview. Chats have their own window; the avatar's Spout canvas keeps its full resolution.",
        );
        ui.horizontal_wrapped(|ui| {
            if help::control(ui, "stream-chat", |ui| {
                ui.button("Open chats below portrait")
            })
            .clicked()
            {
                self.open.store(true, Ordering::Relaxed);
                outputs.set_open(1, true);
                appearance.docked = true;
                if !appearance.styles.iter().any(|s| s.visible) {
                    appearance.styles[0].visible = true;
                }
            }
            if ui.button("Hide chats").clicked() {
                self.open.store(false, Ordering::Relaxed);
            }
        });
        help::control(ui, "stream-chat", |ui| {
            ui.checkbox(&mut appearance.docked, "Follow portrait preview")
        });
        help::control(ui, "stream-chat", |ui| {
            ui.checkbox(&mut appearance.always_on_top, "Keep chat on top")
        });
        help::control(ui, "chat-style", |ui| {
            ui.add(egui::Slider::new(&mut appearance.width, 280.0..=900.0).text("Chat width"))
        });
        for (i, name) in NAMES.iter().enumerate() {
            egui::CollapsingHeader::new(*name).id_salt(("chat-service",i)).default_open(true).show(ui,|ui|{
                let style=&mut appearance.styles[i];
                help::control(ui,"stream-chat",|ui|ui.checkbox(&mut style.visible,format!("Show {name} chat")));
                let (status,account_name,code,url,official,sealed)={let f=self.feeds[i].lock().unwrap();(f.status.clone(),f.account_name.clone(),f.code.clone(),f.auth_url.clone(),f.official_url.clone(),f.sealed_session.clone())};
                if !account_name.is_empty(){ui.label(RichText::new(format!("Signed in: {account_name}")).color(theme::mint()));}
                ui.label(RichText::new(&status).small());
                if !code.is_empty(){ui.horizontal(|ui|{ui.monospace(format!("Sign-in code: {code}"));if ui.small_button("Copy code").clicked(){ui.ctx().copy_text(code.clone());}});}
                if let Some(url)=url{ui.hyperlink_to("Continue sign-in in browser ↗",url);}
                let account=&mut accounts.services[i];
                help::label(ui,if i==0{"Channel name / Twitch URL · blank = your channel"}else{"Live video URL / ID · blank = your active broadcast"},"chat-account");
                self.dirty|=ui.add(egui::TextEdit::singleline(&mut account.target).char_limit(512).desired_width(f32::INFINITY)).changed();
                let busy=self.workers[i].as_ref().is_some_and(|w|!w.finished());
                ui.horizontal_wrapped(|ui|{
                    if help::control(ui,"chat-account",|ui|ui.add_enabled(!busy,egui::Button::new(format!("Sign in to {name}")))).clicked(){
                        self.start(i,account,true,ui.ctx());self.open.store(true,Ordering::Relaxed);outputs.set_open(1,true);style.visible=true;
                    }
                    if ui.add_enabled(!busy&&(!sealed.is_empty()||!account.sealed_session.is_empty()),egui::Button::new("Connect")).clicked(){
                        let mut connection=account.clone();if connection.sealed_session.is_empty(){connection.sealed_session=sealed;}
                        self.start(i,&connection,false,ui.ctx());self.open.store(true,Ordering::Relaxed);outputs.set_open(1,true);style.visible=true;
                    }
                    if ui.add_enabled(busy,egui::Button::new("Disconnect / cancel")).clicked(){self.stop(i,account,false);}
                    if ui.button("Sign out").clicked(){account.sealed_session.clear();self.stop(i,account,true);self.dirty=true;}
                });
                if let Some(url)=official{ui.hyperlink_to("Open official chat to type / moderate ↗",url);}
                if help::control(ui,"chat-account",|ui|ui.checkbox(&mut account.remember,"Remember login")).changed(){
                    if !account.remember{account.sealed_session.clear();}self.dirty=true;
                }
                ui.collapsing("Account setup",|ui|{
                    help::button(ui,"chat-account");
                    theme::caption(ui,if i==0{"Create a Public Twitch application, then paste its client ID. ARIA requests chat read access; your password stays with Twitch."}else{"Enable YouTube Data API v3 and create a Desktop app OAuth client. Import its downloaded JSON below. Add your Google account as a test user while the consent screen is in Testing."});
                    let link=if i==0{"https://dev.twitch.tv/console/apps"}else{"https://console.cloud.google.com/apis/credentials"};
                    ui.hyperlink_to("Open developer console ↗",link);
                    help::label(ui,"OAuth client ID","chat-account");
                    if ui.add_enabled(!busy,egui::TextEdit::singleline(&mut account.client_id).char_limit(256).desired_width(f32::INFINITY)).changed(){
                        account.sealed_session.clear();account.sealed_secret.clear();self.stop(i,account,true);self.dirty=true;
                    }
                    if i==1 {
                        if ui.add_enabled(!busy,egui::Button::new("Import Google Desktop credentials JSON…")).clicked()
                            && let Some(path)=rfd::FileDialog::new().add_filter("Google OAuth client JSON",&["json"]).pick_file(){
                            match import_google(&path){
                                Ok((id,secret))=>{account.client_id=id;account.sealed_secret=secret;account.sealed_session.clear();self.stop(i,account,true);self.dirty=true;}
                                Err(error)=>self.feeds[i].lock().unwrap().status=error.to_string(),
                            }
                        }
                        if !account.sealed_secret.is_empty(){theme::caption(ui,"Desktop client secret protected by Windows.");}
                        ui.collapsing("Enter Desktop client secret manually",|ui|{
                            ui.add(egui::TextEdit::singleline(&mut self.secret_draft).password(true).char_limit(512));
                            if ui.add_enabled(!busy&&!self.secret_draft.is_empty(),egui::Button::new("Protect and save secret")).clicked(){
                                match auth::seal(self.secret_draft.as_bytes()){
                                    Ok(value)=>{account.sealed_secret=value;self.secret_draft.clear();self.dirty=true;}
                                    Err(error)=>self.feeds[i].lock().unwrap().status=error.to_string(),
                                }
                            }
                        });
                    }
                });
                egui::CollapsingHeader::new("Transparency & colors").open(crate::smoke_mode().then_some(true)).show(ui,|ui|{
                    help::button(ui,"chat-style");
                    help::label(ui,"Whole chat opacity","chat-style");
                    ui.add(egui::Slider::new(&mut style.opacity,0.0..=1.0));
                    help::label(ui,"Background opacity","chat-style");
                    ui.add(egui::Slider::new(&mut style.background_opacity,0.0..=1.0));
                    theme::caption(ui,"0 = invisible, 1 = opaque. Background opacity leaves text readable; whole-chat opacity fades both.");
                    ui.horizontal(|ui|{ui.label("Background");if ui.color_edit_button_srgb(&mut style.background).changed(){self.hex[i]=crate::chroma::hex(style.background);}});
                    ui.horizontal(|ui|{
                        let id=ui.id().with(("chat-hex",i));
                        if !ui.memory(|m|m.has_focus(id)){self.hex[i]=crate::chroma::hex(style.background);}
                        let edit=ui.add(egui::TextEdit::singleline(&mut self.hex[i]).id(id).char_limit(7).desired_width(85.0));
                        if edit.changed() && let Ok(color)=crate::chroma::parse_hex(&self.hex[i]){style.background=color;}
                        ui.label("#RRGGBB");
                    });
                    ui.horizontal(|ui|{ui.label("Text");ui.color_edit_button_srgb(&mut style.text_color);});
                    ui.checkbox(&mut style.name_colors,"Use Twitch username colors");
                    help::control(ui,"chat-style",|ui|ui.add(egui::Slider::new(&mut style.font_size,10.0..=30.0).text("Text size")));
                    help::control(ui,"chat-style",|ui|ui.add(egui::Slider::new(&mut style.height,140.0..=600.0).text("Panel height")));
                    if ui.button("Reset appearance").clicked(){*style=Style::default();}
                });
            });
        }
        theme::caption(
            ui,
            "Read-only chat display. Use the official chat link to type, moderate, or see native emotes. Up to 200 messages per service stay in memory. Sign-in is shared across avatars; chat appearance is saved per avatar.",
        );
        if appearance != before {
            appearance.sanitize();
            outputs.edit_chat(appearance);
        }
    }
    pub fn show(
        &self,
        ctx: &egui::Context,
        appearance: &Appearance,
        portrait_open: bool,
        started: Instant,
    ) {
        let key = egui::Id::new("aria-chat-last-appearance");
        if ctx.data(|d| d.get_temp::<Appearance>(key)).as_ref() != Some(appearance) {
            ctx.data_mut(|d| d.insert_temp(key, appearance.clone()));
            ctx.request_repaint_of(viewport_id());
        }
        if !self.open.load(Ordering::Relaxed) || !appearance.styles.iter().any(|s| s.visible) {
            return;
        }
        let portrait = ctx.input(|i| i.raw.viewports.get(&crate::output::viewport_id(1)).cloned());
        if appearance.docked
            && (!portrait_open || portrait.as_ref().is_some_and(|v| v.minimized == Some(true)))
        {
            return;
        }
        let appearance = appearance.clone();
        let height = appearance
            .styles
            .iter()
            .filter(|s| s.visible)
            .map(|s| s.height)
            .sum::<f32>();
        let mut builder = egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_transparent(true)
            .with_inner_size([appearance.width, height])
            .with_min_inner_size([280.0, 140.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_window_level(if appearance.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            });
        if appearance.docked
            && let Some(rect) = portrait.and_then(|v| v.outer_rect)
        {
            let pos = egui::pos2(rect.left(), rect.bottom() + 4.0);
            builder = builder.with_position(pos);
            let moved = ctx.input(|i| {
                i.raw
                    .viewports
                    .get(&viewport_id())
                    .and_then(|v| v.outer_rect)
                    .is_none_or(|r| r.min.distance(pos) > 1.0)
            });
            if moved {
                ctx.send_viewport_cmd_to(viewport_id(), egui::ViewportCommand::OuterPosition(pos));
            }
        }
        let feeds = self.feeds.clone();
        let open = self.open.clone();
        ctx.show_viewport_deferred(viewport_id(),builder,move |ctx,_|{
            if ctx.input(|i|i.viewport().close_requested()){open.store(false,Ordering::Relaxed);ctx.request_repaint_of(egui::ViewportId::ROOT);return;}
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{
                ui.spacing_mut().item_spacing.y=0.0;
                for i in 0..2 {
                    let style=&appearance.styles[i];if !style.visible{continue;}
                    let (rect,_)=ui.allocate_exact_size(egui::vec2(ui.available_width(),style.height),egui::Sense::hover());
                    let mut child=ui.new_child(egui::UiBuilder::new().id_salt(("chat-panel",i)).max_rect(rect));
                    child.set_clip_rect(rect);
                    child.painter().rect_filled(rect,0.0,Color32::from_rgb(style.background[0],style.background[1],style.background[2]).gamma_multiply(style.background_opacity*style.opacity));
                    child.multiply_opacity(style.opacity);
                    let text_color=Color32::from_rgb(style.text_color[0],style.text_color[1],style.text_color[2]);
                    child.visuals_mut().override_text_color=Some(text_color);
                    let feed=feeds[i].lock().unwrap();
                    egui::Frame::NONE.inner_margin(10).show(&mut child,|ui|{
                        ui.horizontal(|ui|{
                            let accent=if i==0{Color32::from_rgb(183,143,255)}else{Color32::from_rgb(255,124,124)};
                            ui.label(RichText::new(NAMES[i]).strong().color(accent));
                            ui.label(RichText::new(if feed.connected{"LIVE"}else{"OFFLINE"}).small().color(text_color));
                            if let Some(url)=&feed.official_url{ui.hyperlink_to("Open ↗",url);}
                        });
                        ui.add(egui::Label::new(RichText::new(&feed.status).small().color(text_color)).wrap());
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().id_salt(("chat-history",i)).stick_to_bottom(true).auto_shrink([false,false]).max_height(ui.available_height()).show(ui,|ui|{
                            if feed.messages.is_empty(){ui.label(RichText::new("Messages appear here after connecting. Configure this service in Streaming chat.").size(style.font_size).color(text_color));}
                            for message in &feed.messages {
                                let author_color=if style.name_colors{message.color.map(|[r,g,b]|Color32::from_rgb(r,g,b)).unwrap_or(text_color)}else{text_color};
                                let mut job=egui::text::LayoutJob::default();
                                job.append(&format!("{}: ",message.author),0.0,egui::TextFormat{font_id:egui::FontId::proportional(style.font_size),color:author_color,..Default::default()});
                                job.append(&message.text,0.0,egui::TextFormat{font_id:egui::FontId::proportional(style.font_size),color:text_color,..Default::default()});
                                ui.add(egui::Label::new(job).wrap());ui.add_space(5.0);
                            }
                        });
                    });
                }
            });
            #[cfg(feature="screenshots")]
            if crate::smoke_mode() {
                crate::screenshot::capture(ctx,started,false);
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            let _=started;
        });
    }
    #[cfg(feature = "screenshots")]
    pub fn smoke(&mut self, outputs: &OutputWindows) {
        self.open.store(true, Ordering::Relaxed);
        outputs.set_open(1, true);
        let mut appearance = Appearance::default();
        appearance.styles[0].background = [36, 22, 65];
        appearance.styles[0].background_opacity = 0.72;
        appearance.styles[1].background = [48, 18, 26];
        appearance.styles[1].background_opacity = 0.45;
        match std::env::var("ARIA_SMOKE_SCENARIO").as_deref() {
            Ok("chat-twitch") => appearance.styles[1].visible = false,
            Ok("chat-youtube") => appearance.styles[0].visible = false,
            Ok("chat-transparent") => {
                for style in &mut appearance.styles {
                    style.background_opacity = 0.0;
                }
            }
            Ok("chat-opacity") => appearance.styles[0].opacity = 0.5,
            _ => {}
        }
        outputs.edit_chat(appearance);
        for i in 0..2 {
            let mut f = self.feeds[i].lock().unwrap();
            f.status = "Preview examples · no account connected".into();
            for (j, (name, text)) in [
                ("PixelFox", "The new avatar looks great!"),
                (
                    "Moonlight",
                    "Both chats together, right below the portrait preview.",
                ),
                ("Sora", "Hello from the other chat!"),
                ("NightOwl", "Background colors and opacity look good."),
            ]
            .into_iter()
            .enumerate()
            {
                protocol::apply(
                    &mut f.messages,
                    protocol::Change::Add(protocol::Message {
                        id: format!("demo-{j}"),
                        user_id: name.into(),
                        author: name.into(),
                        text: text.into(),
                        color: if i == 0 { Some([180, 154, 255]) } else { None },
                    }),
                );
            }
        }
    }
    #[cfg(feature = "screenshots")]
    pub fn verify_smoke_docking(&self, ctx: &egui::Context) {
        if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() != Ok("chat-dock") {
            return;
        }
        let key = egui::Id::new("chat-dock-smoke-phase");
        let (phase, since) =
            ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(key, || (0u8, Instant::now())));
        if since.elapsed() < std::time::Duration::from_millis(700) || phase >= 4 {
            return;
        }
        let parent = crate::output::viewport_id(1);
        match phase {
            0 => ctx.send_viewport_cmd_to(
                parent,
                egui::ViewportCommand::OuterPosition(egui::pos2(100.0, 70.0)),
            ),
            1 => {
                ctx.input(|i| {
                    let p = i
                        .raw
                        .viewports
                        .get(&parent)
                        .and_then(|v| v.outer_rect)
                        .expect("portrait viewport");
                    let c = i
                        .raw
                        .viewports
                        .get(&viewport_id())
                        .and_then(|v| v.outer_rect)
                        .expect("chat viewport");
                    assert!(
                        (c.left() - p.left()).abs() < 3.0
                            && (c.top() - p.bottom() - 4.0).abs() < 3.0,
                        "Chat must follow directly below the moved portrait: {p:?} {c:?}"
                    );
                });
                ctx.send_viewport_cmd_to(parent, egui::ViewportCommand::Minimized(true));
            }
            2 => {
                ctx.input(|i| {
                    assert!(
                        !i.raw.viewports.contains_key(&viewport_id()),
                        "Docked chat should close while portrait is minimized"
                    )
                });
                ctx.send_viewport_cmd_to(parent, egui::ViewportCommand::Minimized(false));
            }
            3 => {
                ctx.input(|i| {
                    assert!(
                        i.raw.viewports.contains_key(&viewport_id()),
                        "Chat should reopen after portrait restores"
                    )
                });
                eprintln!(
                    "Chat native docking verified: follows movement, hides on minimize, returns on restore"
                );
            }
            _ => {}
        }
        ctx.data_mut(|d| d.insert_temp(key, (phase + 1, Instant::now())));
    }
}
fn import_google(path: &std::path::Path) -> anyhow::Result<(String, String)> {
    use anyhow::{Context, bail};
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        bail!("OAuth credentials JSON must be smaller than 64 KiB.");
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).context("Invalid credentials JSON.")?;
    let installed = value
        .get("installed")
        .context("Choose Google OAuth credentials of type Desktop app, not Web application.")?;
    let id = installed["client_id"]
        .as_str()
        .filter(|v| v.ends_with(".apps.googleusercontent.com") && v.len() <= 256)
        .context("Desktop client ID is missing or invalid.")?;
    let secret = installed["client_secret"]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 512)
        .context("Desktop client secret is missing.")?;
    Ok((id.into(), auth::seal(secret.as_bytes())?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_is_bounded_and_old_profiles_get_defaults() {
        let mut a: Appearance = serde_json::from_str("{}").unwrap();
        a.width = f32::NAN;
        a.styles[0].opacity = f32::INFINITY;
        a.styles[1].height = -100.0;
        a.sanitize();
        assert_eq!(a.width, 360.0);
        assert_eq!(a.styles[0].opacity, 1.0);
        assert_eq!(a.styles[1].height, 140.0);
        assert_eq!(
            serde_json::from_str::<Appearance>(&serde_json::to_string(&a).unwrap()).unwrap(),
            a
        );
    }
    #[test]
    fn dropping_worker_or_replacing_feed_prevents_stale_ui_writes() {
        let mut chat = Chats::default();
        let old = chat.feeds[0].clone();
        chat.stop(0, &Account::default(), true);
        old.lock().unwrap().status = "stale login completion".into();
        assert_eq!(chat.feeds[0].lock().unwrap().status, "Signed out.");
        assert!(chat.feeds[1].lock().unwrap().account_name.is_empty());
    }
}
