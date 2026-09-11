use crate::avatar::{self, Sprite};
use aria_core::{
    MappingSettings, PARAMETER_SPECS, ParameterPipeline, Parameters, TrackingFrame, demo_frame,
};
use aria_model::ModelReport;
use aria_tracking::{Protocol, Receiver, ReceiverConfig, Snapshot};
use eframe::egui::{self, Color32, Frame, RichText, Stroke};
use serde::{Deserialize, Serialize};
use std::{
    net::Ipv4Addr,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const BG: Color32 = Color32::from_rgb(15, 18, 29);
const PANEL: Color32 = Color32::from_rgb(23, 27, 41);
const MINT: Color32 = Color32::from_rgb(114, 235, 209);
const MUTED: Color32 = Color32::from_rgb(149, 160, 180);
const OUTPUT: &str = "aria-output";

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
enum Source {
    #[default]
    Demo,
    Vts,
    Json,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
enum Background {
    #[default]
    Studio,
    Green,
    Transparent,
}

impl Background {
    fn color(self) -> Color32 {
        match self {
            Self::Studio => BG,
            Self::Green => Color32::from_rgb(0, 255, 0),
            Self::Transparent => Color32::TRANSPARENT,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    source: Source,
    sender_ip: String,
    request_port: u16,
    listen_port: u16,
    mapping: MappingSettings,
    background: Background,
    fps: u32,
    zoom: f32,
    always_on_top: bool,
    cubism_core: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source: Source::Demo,
            sender_ip: "127.0.0.1".into(),
            request_port: 21412,
            listen_port: 11125,
            mapping: MappingSettings::default(),
            background: Background::Studio,
            fps: 60,
            zoom: 1.0,
            always_on_top: false,
            cubism_core: String::new(),
        }
    }
}

pub struct AriaApp {
    settings: Settings,
    receiver: Option<Receiver>,
    snapshot: Snapshot,
    pipeline: ParameterPipeline,
    raw: Option<TrackingFrame>,
    params: Parameters,
    started: Instant,
    last_update: Instant,
    render_fps: f32,
    gpu: String,
    status_message: Option<String>,
    output_open: bool,
    output_close_requested: Arc<AtomicBool>,
    raw_view: bool,
    idle: Option<Sprite>,
    talking: Option<Sprite>,
    model: Option<ModelReport>,
    model_open: bool,
    live2d: Option<crate::live2d::Avatar>,
    render_state: Option<eframe::egui_wgpu::RenderState>,
    bare_moc: Option<PathBuf>,
    bare_textures: Vec<PathBuf>,
}

impl AriaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = PANEL;
        style.visuals.window_fill = PANEL;
        style.visuals.selection.bg_fill = Color32::from_rgb(42, 92, 88);
        style.visuals.selection.stroke = Stroke::new(1.0_f32, MINT);
        style.spacing.item_spacing = egui::vec2(10.0, 9.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        cc.egui_ctx.set_style(style);
        let mut settings: Settings = if crate::smoke_mode() {
            Settings::default()
        } else {
            cc.storage
                .and_then(|s| eframe::get_value(s, "aria-settings-v1"))
                .unwrap_or_default()
        };
        settings.fps = settings.fps.clamp(15, 120);
        if let Some(path) = std::env::var_os("ARIA_CUBISM_CORE") {
            settings.cubism_core = path.to_string_lossy().into_owned();
        }
        settings.zoom = if settings.zoom.is_finite() {
            settings.zoom.clamp(0.5, 1.5)
        } else {
            1.0
        };
        let gpu = cc
            .wgpu_render_state
            .as_ref()
            .map(|r| {
                let i = r.adapter.get_info();
                format!("{} · {:?}", i.name, i.backend)
            })
            .unwrap_or_else(|| "GPU unavailable".into());
        let app = Self {
            settings,
            receiver: None,
            snapshot: Snapshot::default(),
            pipeline: ParameterPipeline::default(),
            raw: None,
            params: Parameters::default(),
            started: Instant::now(),
            last_update: Instant::now(),
            render_fps: 60.0,
            gpu,
            status_message: None,
            output_open: false,
            output_close_requested: Arc::new(AtomicBool::new(false)),
            raw_view: false,
            idle: None,
            talking: None,
            model: None,
            model_open: false,
            live2d: None,
            render_state: cc.wgpu_render_state.clone(),
            bare_moc: None,
            bare_textures: Vec::new(),
        };
        let mut app = app;
        if let Some(path) = std::env::args_os().nth(1) {
            app.open_model(Path::new(&path));
        }
        #[cfg(feature = "screenshots")]
        let app = {
            let mut app = app;
            if crate::smoke_mode() {
                if let Some(path) = std::env::var_os("ARIA_TEST_MODEL") {
                    app.import_model(
                        aria_model::load_files(Path::new(&path)).expect("Smoke model assets"),
                    )
                    .expect("Smoke model load");
                }
                match std::env::var("ARIA_SMOKE_SCENARIO").as_deref() {
                    Ok("vts") => {
                        app.settings.source = Source::Vts;
                        app.connect();
                    }
                    Ok("output") => {
                        app.output_open = true;
                        app.settings.background = Background::Green;
                    }
                    _ => {}
                }
            }
            app
        };
        app
    }

    fn connection_status(&self) -> &str {
        if self.settings.source == Source::Demo {
            "Demo input"
        } else if self.receiver.is_none() {
            "Disconnected"
        } else {
            self.snapshot.status()
        }
    }

    fn connect(&mut self) {
        let Ok(sender_ip) = self.settings.sender_ip.trim().parse::<Ipv4Addr>() else {
            self.status_message = Some("Enter an IPv4 address, for example 192.168.1.42.".into());
            return;
        };
        if self.settings.listen_port == 0 || self.settings.request_port == 0 {
            self.status_message = Some("Ports must be between 1 and 65535.".into());
            return;
        }
        let config = ReceiverConfig {
            sender_ip,
            request_port: self.settings.request_port,
            listen_port: self.settings.listen_port,
            protocol: if self.settings.source == Source::Vts {
                Protocol::VTubeStudio
            } else {
                Protocol::AriaJson
            },
        };
        match Receiver::start(config) {
            Ok(receiver) => {
                self.receiver = Some(receiver);
                self.snapshot = Snapshot::default();
                self.pipeline.reset();
                self.status_message = None;
            }
            Err(e) => self.status_message = Some(format!("{e:#}")),
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        section(ui, "TRACKING SOURCE");
        let old = self.settings.source;
        egui::ComboBox::from_id_salt("source")
            .selected_text(match old {
                Source::Demo => "Demo · no device needed",
                Source::Vts => "iPhone · VTube Studio",
                Source::Json => "External tool · ARIA JSON",
            })
            .width(230.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.settings.source,
                    Source::Demo,
                    "Demo · no device needed",
                );
                ui.selectable_value(
                    &mut self.settings.source,
                    Source::Vts,
                    "iPhone · VTube Studio",
                );
                ui.selectable_value(
                    &mut self.settings.source,
                    Source::Json,
                    "External tool · ARIA JSON",
                );
            });
        if old != self.settings.source {
            self.receiver = None;
            self.snapshot = Snapshot::default();
            self.pipeline.reset();
            self.raw = None;
            self.status_message = None;
        }
        if self.settings.source == Source::Demo {
            ui.label(
                RichText::new("Synthetic movement for checking your avatar and output.")
                    .color(MUTED),
            );
        } else {
            ui.add_enabled_ui(self.receiver.is_none(), |ui| {
                ui.label(if self.settings.source == Source::Vts {
                    "iPhone IPv4 address"
                } else {
                    "Allowed sender IPv4 address"
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.sender_ip).desired_width(230.0),
                );
                egui::Grid::new("ports").num_columns(2).show(ui, |ui| {
                    if self.settings.source == Source::Vts {
                        ui.label("Phone request port");
                        ui.add(
                            egui::DragValue::new(&mut self.settings.request_port).range(1..=65535),
                        );
                        ui.end_row();
                    }
                    ui.label("PC receive port");
                    ui.add(egui::DragValue::new(&mut self.settings.listen_port).range(1..=65535));
                    ui.end_row();
                });
            });
            if self.receiver.is_some() {
                if ui.button("Disconnect").clicked() {
                    self.receiver = None;
                    self.raw = None;
                }
            } else if ui
                .add_sized(
                    [230.0, 36.0],
                    egui::Button::new(RichText::new("Connect tracking").color(BG)).fill(MINT),
                )
                .clicked()
            {
                self.connect();
            }
            ui.label(RichText::new(if self.settings.source == Source::Vts { "On iPhone: enable 3rd Party PC Clients in VTube Studio. Use the phone's IPv4 address." }
                else { "Accepts ARIA JSON v1 packets from this IP. Use 127.0.0.1 for local tools." }).small().color(MUTED));
        }
        ui.add_space(7.0);
        let color = if self.raw.as_ref().is_some_and(|f| f.face_found) {
            MINT
        } else {
            Color32::from_rgb(238, 191, 119)
        };
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.colored_label(color, self.connection_status());
        });
        if let Some(error) = &self.status_message {
            ui.colored_label(Color32::from_rgb(255, 164, 167), error);
        }
        if let Some(error) = &self.snapshot.last_error {
            ui.label(
                RichText::new(error)
                    .small()
                    .color(Color32::from_rgb(255, 164, 167)),
            );
        }

        ui.separator();
        section(ui, "MOVEMENT");
        if ui
            .add_enabled(
                self.raw.as_ref().is_some_and(|f| f.face_found),
                egui::Button::new("Calibrate neutral pose"),
            )
            .clicked()
            && let Some(f) = &self.raw
        {
            self.pipeline.calibrate(f);
        }
        ui.add(
            egui::Slider::new(&mut self.settings.mapping.smoothing_ms, 0.0..=300.0)
                .text("Smooth ms"),
        );
        ui.add(
            egui::Slider::new(&mut self.settings.mapping.head_gain, 0.1..=3.0).text("Head gain"),
        );
        ui.add(
            egui::Slider::new(&mut self.settings.mapping.mouth_gain, 0.1..=3.0).text("Mouth gain"),
        );
        ui.checkbox(&mut self.settings.mapping.mirror, "Mirror movement");
        ui.collapsing("Axis correction", |ui| {
            ui.checkbox(
                &mut self.settings.mapping.invert_yaw,
                "Invert yaw (left / right)",
            );
            ui.checkbox(
                &mut self.settings.mapping.invert_pitch,
                "Invert pitch (up / down)",
            );
            ui.checkbox(&mut self.settings.mapping.invert_roll, "Invert roll (tilt)");
            if ui.button("Reset mapping and calibration").clicked() {
                self.settings.mapping = MappingSettings::default();
                self.pipeline.reset();
            }
        });

        ui.separator();
        section(ui, "AVATAR");
        ui.label(if self.live2d.is_some() {
            "Live2D Cubism avatar"
        } else if self.idle.is_some() {
            "PNG puppet"
        } else {
            "Mica · built-in test puppet"
        });
        if let Some(sprite) = &self.idle {
            ui.label(RichText::new(&sprite.name).small().color(MUTED));
        }
        ui.horizontal(|ui| {
            if ui.button("Open PNG…").clicked() {
                self.load_image(ctx, false);
            }
            if ui
                .add_enabled(
                    self.idle.is_some() || self.live2d.is_some(),
                    egui::Button::new("Reset"),
                )
                .clicked()
            {
                self.idle = None;
                self.talking = None;
                self.live2d = None;
            }
        });
        if self.idle.is_some() && self.live2d.is_none() {
            if ui.button("Set talking image…").clicked() {
                self.load_image(ctx, true);
            }
            if let Some(sprite) = &self.talking {
                ui.label(
                    RichText::new(format!("Talking: {}", sprite.name))
                        .small()
                        .color(MUTED),
                );
            }
            ui.label(RichText::new("Images move with your head. Optional talking image switches when your mouth opens.").small().color(MUTED));
        }
        ui.add(egui::Slider::new(&mut self.settings.zoom, 0.5..=1.5).text("Zoom"));
        if ui.button("Open Live2D avatar…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Cubism export", &["moc3", "json"])
                .pick_file()
        {
            self.open_model(&path);
        }
        if let Some(avatar) = &mut self.live2d {
            ui.label(RichText::new(&avatar.name).color(MINT));
            ui.label(
                RichText::new(format!(
                    "{} meshes · {} tracked parameters\n{:.0} MiB atlases · Core {}",
                    avatar.model.drawables.len(),
                    avatar.mapped_count(),
                    avatar.atlas_mib(),
                    avatar.model.version
                ))
                .small(),
            );
            if ui.button("Model parameters…").clicked() {
                avatar.controls_open = true;
            }
            for warning in &avatar.files.warnings {
                ui.label(RichText::new(warning).small().color(MUTED));
            }
        }
        ui.collapsing("Cubism runtime setup", |ui| {
            ui.label("Choose Core/dll/windows/x86_64/Live2DCubismCore.dll from the official Native SDK. The path is saved locally.");
            ui.text_edit_singleline(&mut self.settings.cubism_core);
            if ui.button("Select Core DLL…").clicked() && let Some(path) = rfd::FileDialog::new().add_filter("Cubism Core", &["dll"]).pick_file() {
                self.settings.cubism_core = path.display().to_string();
            }
            ui.hyperlink_to("Download the official SDK ↗", "https://www.live2d.com/en/sdk/download/native/");
            ui.hyperlink_to("Avatar import instructions ↗", "https://github.com/NekoUnix/A.R.I.A/blob/main/docs/live2d.md");
        });
        if ui.button("Inspect Live2D .model3.json…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Cubism model3 JSON", &["json"])
                .pick_file()
        {
            match aria_model::inspect(&path) {
                Ok(report) => {
                    self.model = Some(report);
                    self.model_open = true;
                    self.status_message = None;
                }
                Err(e) => self.status_message = Some(format!("{e:#}")),
            }
        }
        ui.label(
            RichText::new("Drop a .model3.json or .moc3 here to load an avatar.")
                .small()
                .color(MUTED),
        );

        ui.separator();
        section(ui, "WINDOWS OUTPUT");
        ui.checkbox(&mut self.output_open, "Open OBS capture window");
        ui.checkbox(&mut self.settings.always_on_top, "Keep output on top");
        egui::ComboBox::from_id_salt("background")
            .selected_text(match self.settings.background {
                Background::Studio => "Studio background",
                Background::Green => "Green screen",
                Background::Transparent => "Transparent (experimental)",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.settings.background,
                    Background::Studio,
                    "Studio background",
                );
                ui.selectable_value(
                    &mut self.settings.background,
                    Background::Green,
                    "Green screen",
                );
                ui.selectable_value(
                    &mut self.settings.background,
                    Background::Transparent,
                    "Transparent (experimental)",
                );
            });
        ui.label(RichText::new("OBS → Window Capture → A.R.I.A. Output. For reliable transparency, use Green screen + Chroma Key.").small().color(MUTED));
        egui::ComboBox::from_id_salt("fps")
            .selected_text(format!("{} FPS target", self.settings.fps))
            .show_ui(ui, |ui| {
                for fps in [30, 60, 120] {
                    ui.selectable_value(&mut self.settings.fps, fps, format!("{fps} FPS"));
                }
            });
        ui.hyperlink_to(
            "Windows setup & troubleshooting ↗",
            "https://github.com/NekoUnix/A.R.I.A/blob/main/docs/windows.md",
        );
    }

    fn load_image(&mut self, ctx: &egui::Context, talking: bool) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Avatar image", &["png", "jpg", "jpeg"])
            .pick_file()
        {
            match avatar::load_sprite(ctx, &path) {
                Ok(sprite) => {
                    self.live2d = None;
                    if talking {
                        self.talking = Some(sprite);
                    } else {
                        self.idle = Some(sprite);
                        self.talking = None;
                    }
                    self.status_message = None;
                }
                Err(e) => self.status_message = Some(format!("{e:#}")),
            }
        }
    }

    fn active_sprite(&self) -> Option<&Sprite> {
        if self.params.0[5] > 0.18 {
            self.talking.as_ref().or(self.idle.as_ref())
        } else {
            self.idle.as_ref()
        }
    }

    fn import_model(&mut self, files: aria_model::ModelFiles) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.settings.cubism_core.trim().is_empty(),
            "First choose the official x64 Cubism Core DLL under Cubism runtime setup."
        );
        let state = self
            .render_state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("GPU renderer is unavailable"))?;
        let avatar =
            crate::live2d::Avatar::load(state, Path::new(self.settings.cubism_core.trim()), files)?;
        self.live2d = Some(avatar);
        self.idle = None;
        self.talking = None;
        self.status_message = None;
        Ok(())
    }
    fn open_model(&mut self, path: &Path) {
        match aria_model::load_files(path) {
            Ok(files) => {
                if let Err(error) = self.import_model(files) {
                    self.status_message = Some(format!("{error:#}"));
                }
            }
            Err(error) => {
                self.status_message = Some(format!("{error:#}"));
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("moc3"))
                {
                    self.bare_moc = Some(path.to_owned());
                    self.bare_textures.clear();
                }
            }
        }
    }
    fn bare_import_window(&mut self, ctx: &egui::Context) {
        let Some(path) = self.bare_moc.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Import moc3 with explicit textures").open(&mut open).default_width(600.0).show(ctx,|ui| {
            ui.label(path.display().to_string());
            ui.label("No usable matching manifest was found. Prefer opening the exported .model3.json. For a bare moc3, add each texture atlas and put them in index order (0, 1, 2…).");
            if ui.button("Add texture atlases…").clicked() && let Some(files) = rfd::FileDialog::new().add_filter("PNG atlases", &["png"]).pick_files() { self.bare_textures.extend(files); }
            let mut swap = None; let mut remove = None;
            egui::ScrollArea::vertical().max_height(280.0).show(ui,|ui| {
                for (i,texture) in self.bare_textures.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{i}: {}",texture.display()));
                        if ui.add_enabled(i>0,egui::Button::new("Up")).clicked() { swap = Some((i,i-1)); }
                        if ui.add_enabled(i+1<self.bare_textures.len(),egui::Button::new("Down")).clicked() { swap = Some((i,i+1)); }
                        if ui.button("Remove").clicked() { remove = Some(i); }
                    });
                }
            });
            if let Some((a,b)) = swap { self.bare_textures.swap(a,b); }
            if let Some(i) = remove { self.bare_textures.remove(i); }
            if ui.add_enabled(!self.bare_textures.is_empty(),egui::Button::new("Load avatar")).clicked() {
                match aria_model::bare_moc(&path,&self.bare_textures).and_then(|f| self.import_model(f)) {
                    Ok(()) => { self.bare_moc = None; self.bare_textures.clear(); },
                    Err(e) => self.status_message = Some(format!("{e:#}")),
                }
            }
            if let Some(error) = &self.status_message { ui.colored_label(egui::Color32::LIGHT_RED,error); }
        });
        if !open {
            self.bare_moc = None;
            self.bare_textures.clear();
        }
    }

    fn diagnostics(&mut self, ui: &mut egui::Ui) {
        section(ui, "INPUT MONITOR");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.raw_view, false, "Mapped");
            ui.selectable_value(&mut self.raw_view, true, "Raw");
        });
        if self.raw_view {
            if let Some(f) = &self.raw {
                ui.label(format!(
                    "Head {:.1} / {:.1} / {:.1}°",
                    f.rotation.x, f.rotation.y, f.rotation.z
                ));
                ui.label(format!(
                    "Position {:.3} / {:.3} / {:.3}",
                    f.position.x, f.position.y, f.position.z
                ));
                ui.label(format!(
                    "{} blendshapes · hotkey {}",
                    f.blend_shapes.len(),
                    f.hotkey
                ));
                ui.label(
                    RichText::new(
                        "Sender time is preserved; packet age uses the PC's monotonic clock.",
                    )
                    .small()
                    .color(MUTED),
                );
                for (name, value) in &f.blend_shapes {
                    meter(ui, name, *value, 0.0, 1.0);
                }
            } else {
                ui.label("Connect a tracker to see raw values.");
            }
        } else {
            for ((name, min, max), value) in PARAMETER_SPECS.iter().zip(self.params.0) {
                meter(ui, name, value, *min, *max);
            }
        }
        ui.separator();
        if ui.button("Export mapped values…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_file_name("aria-parameters.json")
                .add_filter("JSON", &["json"])
                .save_file()
        {
            match serde_json::to_vec_pretty(&self.params.named())
                .map_err(anyhow::Error::from)
                .and_then(|bytes| std::fs::write(path, bytes).map_err(anyhow::Error::from))
            {
                Ok(()) => self.status_message = Some("Parameter snapshot exported.".into()),
                Err(e) => self.status_message = Some(format!("Export failed: {e}")),
            }
        }
        section(ui, "CONNECTION");
        if self.settings.source == Source::Demo {
            ui.label("Demo generates values locally.");
        } else {
            ui.label(format!(
                "{} valid · {} rejected",
                self.snapshot.packets, self.snapshot.rejected
            ));
            ui.label(format!(
                "{} ignored · {} requests",
                self.snapshot.ignored, self.snapshot.requests
            ));
            ui.label(format!(
                "{:.0} packets / second",
                if self.snapshot.fresh_frame().is_some() {
                    self.snapshot.packets_per_second
                } else {
                    0.0
                }
            ));
            if let Some(at) = self.snapshot.received_at {
                ui.label(format!(
                    "Last valid packet: {:.0} ms ago",
                    at.elapsed().as_secs_f64() * 1000.0
                ));
            }
        }
    }

    fn model_window(&mut self, ctx: &egui::Context) {
        if let Some(report) = &self.model {
            egui::Window::new("Live2D asset inspection").open(&mut self.model_open).default_width(670.0).show(ctx, |ui| {
                ui.label(report.manifest.display().to_string());
                ui.colored_label(MINT, format!("{} textures · {} expressions · {} motions · {:.2} MiB on disk", report.texture_count,
                    report.expression_count, report.motion_count, report.total_bytes() as f64 / 1048576.0));
                ui.label(format!("{} file problems. Presence checks do not validate the contents of a .moc3 file.", report.problem_count()));
                ui.colored_label(Color32::from_rgb(238, 191, 119), "This checks exported assets. It does not load or render a Cubism model.");
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    egui::Grid::new("assets").striped(true).show(ui, |ui| {
                        ui.strong("Type"); ui.strong("File"); ui.strong("Result"); ui.end_row();
                        for asset in &report.assets {
                            ui.label(asset.kind); ui.label(&asset.relative_path);
                            ui.label(asset.problem.clone().unwrap_or_else(|| format!("{} bytes", asset.bytes.unwrap_or_default()))); ui.end_row();
                        }
                    });
                });
            });
        }
    }
}

impl eframe::App for AriaApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        if let Some(path) = dropped.first() {
            self.open_model(path);
        }
        if self.output_close_requested.swap(false, Ordering::AcqRel) {
            self.output_open = false;
        }
        let now = Instant::now();
        // A layout discard can call update twice for the same frame.
        let dt = if ctx.current_pass_index() == 0 {
            let dt = now.duration_since(self.last_update).as_secs_f32();
            self.last_update = now;
            dt
        } else {
            0.0
        };
        if dt > 0.0 {
            self.render_fps = self.render_fps * 0.92 + (1.0 / dt).min(1000.0) * 0.08;
        }
        if self.settings.source == Source::Demo {
            self.raw = Some(demo_frame(self.started.elapsed().as_secs_f32()));
        } else if let Some(receiver) = &self.receiver {
            self.snapshot = receiver.snapshot();
            self.raw = self.snapshot.fresh_frame().cloned();
        } else {
            self.raw = None;
        }
        self.params = self
            .pipeline
            .update(self.raw.as_ref(), &self.settings.mapping, dt);
        if let Some(avatar) = &mut self.live2d
            && dt > 0.0
            && let Err(error) = avatar.update(self.params)
        {
            self.status_message = Some(format!("Live2D update stopped: {error:#}"));
            self.live2d = None;
        }

        egui::TopBottomPanel::top("header")
            .frame(Frame::new().fill(BG).inner_margin(18.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("A.R.I.A.").size(26.0).strong().color(MINT));
                    ui.label(RichText::new("AVATAR STUDIO").size(12.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("v0.2 · WINDOWS PREVIEW").small().color(MUTED));
                    });
                });
            });
        egui::TopBottomPanel::bottom("status")
            .frame(Frame::new().fill(BG).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&self.gpu).small().color(MUTED));
                    ui.separator();
                    ui.label(RichText::new(format!("{:.0} UI FPS", self.render_fps)).small());
                    if let Some(cpu) = frame.info().cpu_usage {
                        ui.label(RichText::new(format!("{:.2} ms UI work", cpu * 1000.0)).small());
                    }
                    ui.separator();
                    ui.label(
                        RichText::new("Local processing · no telemetry")
                            .small()
                            .color(MUTED),
                    );
                });
            });
        egui::SidePanel::left("controls")
            .exact_width(288.0)
            .resizable(false)
            .frame(Frame::new().fill(PANEL).inner_margin(18.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("controls-scroll")
                    .show(ui, |ui| self.controls(ui, ctx));
            });
        egui::SidePanel::right("diagnostics")
            .exact_width(258.0)
            .resizable(false)
            .frame(Frame::new().fill(PANEL).inner_margin(18.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("diagnostics-scroll")
                    .show(ui, |ui| self.diagnostics(ui));
            });
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(20.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Your stage");
                    ui.label(
                        RichText::new(if self.live2d.is_some() {
                            "LIVE2D / CUBISM"
                        } else if self.idle.is_some() {
                            "PNG PUPPET"
                        } else {
                            "MICA / TEST PUPPET"
                        })
                        .small()
                        .color(MUTED),
                    );
                });
                ui.label(RichText::new(self.connection_status()).color(MINT));
                let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(
                    rect,
                    12.0,
                    if self.settings.background == Background::Transparent {
                        Color32::from_rgb(30, 34, 48)
                    } else {
                        self.settings.background.color()
                    },
                );
                if self.settings.background == Background::Studio {
                    for i in 1..12 {
                        let x = rect.left() + rect.width() * i as f32 / 12.0;
                        painter.line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            Stroke::new(1.0_f32, Color32::from_rgb(25, 30, 44)),
                        );
                    }
                    painter.circle_stroke(
                        rect.center(),
                        rect.width().min(rect.height()) * 0.38,
                        Stroke::new(1.0_f32, Color32::from_rgb(45, 60, 69)),
                    );
                }
                if let Some(avatar) = &self.live2d {
                    avatar.image().draw(&painter, rect, self.settings.zoom);
                } else {
                    avatar::draw(
                        &painter,
                        rect,
                        self.params,
                        self.active_sprite(),
                        self.settings.zoom,
                    );
                }
                painter.text(
                    rect.left_bottom() + egui::vec2(16.0, -18.0),
                    egui::Align2::LEFT_BOTTOM,
                    if self.settings.source == Source::Demo {
                        "DEMO INPUT  /  Connect your iPhone to go live"
                    } else {
                        "TRACKING PREVIEW  /  Open the capture window for OBS"
                    },
                    egui::FontId::proportional(11.0),
                    MUTED,
                );
            });
        self.model_window(ctx);
        self.bare_import_window(ctx);
        if let Some(avatar) = &mut self.live2d {
            avatar.controls(ctx);
        }
        #[cfg(feature = "screenshots")]
        screenshot_capture(ctx, self.started, false);
        if self.output_open {
            let close_requested = Arc::clone(&self.output_close_requested);
            let background = self.settings.background.color();
            let params = self.params;
            let sprite = self.active_sprite().cloned();
            let model_image = self.live2d.as_ref().map(|a| a.image());
            let zoom = self.settings.zoom;
            let interval = Duration::from_secs_f64(1.0 / self.settings.fps as f64);
            #[cfg(feature = "screenshots")]
            let started = self.started;
            let level = if self.settings.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ctx.show_viewport_deferred(
                egui::ViewportId::from_hash_of(OUTPUT),
                egui::ViewportBuilder::default()
                    .with_title("A.R.I.A. Output")
                    .with_inner_size([720.0, 720.0])
                    .with_transparent(true)
                    .with_window_level(level),
                move |ctx, _class| {
                    let began = Instant::now();
                    if ctx.input(|i| i.viewport().close_requested()) {
                        close_requested.store(true, Ordering::Release);
                        ctx.request_repaint_of(egui::ViewportId::ROOT);
                    }
                    egui::CentralPanel::default()
                        .frame(Frame::NONE.fill(background))
                        .show(ctx, |ui| {
                            if let Some(image) = model_image {
                                image.draw(ui.painter(), ui.max_rect(), zoom);
                            } else {
                                avatar::draw(
                                    ui.painter(),
                                    ui.max_rect(),
                                    params,
                                    sprite.as_ref(),
                                    zoom,
                                );
                            }
                        });
                    #[cfg(feature = "screenshots")]
                    screenshot_capture(ctx, started, true);
                    let prediction =
                        Duration::from_secs_f32(ctx.input(|i| i.predicted_dt).max(0.0));
                    ctx.request_repaint_after(
                        interval.saturating_sub(began.elapsed()) + prediction,
                    );
                },
            );
        }
        let interval = Duration::from_secs_f64(1.0 / self.settings.fps as f64);
        let remaining = (self.last_update + interval).saturating_duration_since(Instant::now());
        // egui subtracts predicted_dt internally. Compensate so a 60 Hz request
        // does not become an immediate repaint on a high-refresh monitor.
        let prediction = Duration::from_secs_f32(ctx.input(|i| i.predicted_dt).max(0.0));
        ctx.request_repaint_after(remaining + prediction);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if crate::smoke_mode() {
            return;
        }
        eframe::set_value(storage, "aria-settings-v1", &self.settings);
    }
}

#[cfg(feature = "screenshots")]
fn screenshot_capture(ctx: &egui::Context, started: Instant, output: bool) {
    crate::screenshot::capture(ctx, started, output);
}

fn section(ui: &mut egui::Ui, label: &str) {
    ui.add_space(5.0);
    ui.label(RichText::new(label).size(11.0).strong().color(MUTED));
}

fn meter(ui: &mut egui::Ui, label: &str, value: f32, min: f32, max: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{value:.2}")).monospace().size(11.0));
        });
    });
    ui.add(
        egui::ProgressBar::new((value - min) / (max - min))
            .desired_height(5.0)
            .fill(MINT),
    );
    ui.add_space(3.0);
}
