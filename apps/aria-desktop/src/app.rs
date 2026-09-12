use crate::avatar::{self, Sprite};
use crate::input_monitor::{InputMonitor, Tab};
use crate::output::{Background, OutputSettings, OutputWindows};
use aria_core::{MappingSettings, ParameterPipeline, Parameters, TrackingFrame, demo_frame};
use aria_core::{
    movement::{self, PoseMode, RigConfig, SavedRig},
    rig::{self, Inputs, RigParameter},
};
use aria_model::ModelReport;
use aria_tracking::{Protocol, Receiver, ReceiverConfig, Snapshot};
use eframe::egui::{self, Color32, Frame, RichText, Stroke};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::theme::{self, BG, MINT, MUTED, PANEL};

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
enum Source {
    #[default]
    Demo,
    Vts,
    Json,
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
    saved_rigs: BTreeMap<String, SavedRig>,
    model_preferences: BTreeMap<String, ModelPreferences>,
    outputs: Option<OutputSettings>,
    high_priority: bool,
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
            saved_rigs: BTreeMap::new(),
            model_preferences: BTreeMap::new(),
            outputs: None,
            high_priority: false,
        }
    }
}

/// Per-avatar controls outside its native rig. The SDK path is a machine preference.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct ModelPreferences {
    source: Source,
    sender_ip: String,
    request_port: u16,
    listen_port: u16,
    mapping: MappingSettings,
    background: Background,
    fps: u32,
    zoom: f32,
    always_on_top: bool,
    calibration: aria_core::Vec3,
    outputs: Option<OutputSettings>,
}
impl Default for ModelPreferences {
    fn default() -> Self {
        Self::capture(&Settings::default())
    }
}
impl ModelPreferences {
    fn capture(settings: &Settings) -> Self {
        Self {
            source: settings.source,
            sender_ip: settings.sender_ip.clone(),
            request_port: settings.request_port,
            listen_port: settings.listen_port,
            mapping: settings.mapping.clone(),
            background: settings.background,
            fps: settings.fps,
            zoom: settings.zoom,
            always_on_top: settings.always_on_top,
            calibration: aria_core::Vec3::default(),
            outputs: settings.outputs.clone(),
        }
    }
    fn restore(&self, settings: &mut Settings) {
        settings.source = self.source;
        settings.sender_ip = self.sender_ip.clone();
        settings.request_port = self.request_port;
        settings.listen_port = self.listen_port;
        settings.mapping = self.mapping.clone();
        settings.background = self.background;
        settings.fps = self.fps.clamp(15, 120);
        settings.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.5, 1.5)
        } else {
            1.0
        };
        settings.always_on_top = self.always_on_top;
        settings.outputs = self.outputs.clone().map(OutputSettings::sanitized);
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
    frame_clock: crate::performance::FrameClock,
    scene_revision: u64,
    render_fps: f32,
    gpu: String,
    metrics: crate::metrics::Metrics,
    status_message: Option<String>,
    outputs: OutputWindows,
    broadcasts: crate::broadcast::Broadcasts,
    priority_status: Option<String>,
    input_monitor: InputMonitor,
    items: crate::items::Items,
    hotkeys: crate::hotkeys::Hotkeys,
    live_inputs: Inputs,
    animation_time: f32,
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
    fn scene(&self) -> crate::output::Scene {
        crate::output::Scene {
            items: self.items.draws.clone(),
            model: self.live2d.as_ref().map(|a| a.image()),
            model_bounds: self
                .live2d
                .as_ref()
                .map_or(egui::Rect::NOTHING, |a| a.image_bounds()),
            sprite: self.active_sprite().cloned(),
            params: self.params,
        }
    }
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let mut settings: Settings = if crate::smoke_mode() {
            Settings::default()
        } else {
            cc.storage
                .and_then(|s| eframe::get_value(s, "aria-settings-v1"))
                .unwrap_or_default()
        };
        if let Some(preferences) = settings.model_preferences.get("preview-v1").cloned() {
            preferences.restore(&mut settings);
        }
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
        let preview_parameters = movement::preview_parameters(Parameters::default());
        let input_monitor = InputMonitor::new(
            "preview-v1".into(),
            RigConfig::from_parameters(&preview_parameters),
            settings.saved_rigs.get("preview-v1").cloned(),
            &preview_parameters,
        );
        let outputs = OutputWindows::new(settings.outputs.clone().unwrap_or_else(|| {
            OutputSettings::from_legacy(settings.background, settings.zoom, settings.always_on_top)
        }));
        let app = Self {
            settings,
            receiver: None,
            snapshot: Snapshot::default(),
            pipeline: ParameterPipeline::default(),
            raw: None,
            params: Parameters::default(),
            started: Instant::now(),
            frame_clock: crate::performance::FrameClock::new(Instant::now()),
            scene_revision: 0,
            render_fps: 60.0,
            gpu,
            metrics: crate::metrics::Metrics::default(),
            status_message: None,
            outputs,
            broadcasts: Default::default(),
            priority_status: None,
            input_monitor,
            items: Default::default(),
            hotkeys: crate::hotkeys::Hotkeys::new(cc.egui_ctx.clone()),
            live_inputs: Inputs::new(),
            animation_time: 0.0,
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
        if app.settings.high_priority {
            match crate::performance::set_high_priority(true) {
                Ok(()) => app.priority_status = Some("Windows priority: High".into()),
                Err(error) => {
                    app.settings.high_priority = false;
                    app.priority_status = Some(format!("Could not set High priority: {error:#}"));
                }
            }
        }
        if let Some(preferences) = app.settings.model_preferences.get("preview-v1") {
            app.pipeline.restore_calibration(preferences.calibration);
        }
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
                        assert!(
                            app.receiver.is_some(),
                            "Smoke tracking connection failed: {:?}",
                            app.status_message
                        );
                    }
                    Ok("output") => {
                        app.outputs.set_open(0, true);
                        app.outputs
                            .edit_canvas(0, |c| c.background = Background::Green);
                    }
                    Ok("output-both")
                    | Ok("output-transparent")
                    | Ok("capture-controls")
                    | Ok("capture-minimized")
                    | Ok("output-resize")
                    | Ok("output-freeform") => {
                        app.outputs.set_open(0, true);
                        app.outputs.set_open(1, true);
                        app.outputs.set_open(2, true);
                        app.outputs.edit_canvas(2, |c| {
                            c.freeform_size = [1536, 1024];
                            c.freeform_window = [480, 320];
                            c.background = Background::Transparent;
                        });
                        app.outputs.edit_canvas(0, |c| {
                            c.background = Background::Green;
                            c.position = [0.2, 0.0];
                        });
                        app.outputs.edit_canvas(1, |c| {
                            c.background = Background::Studio;
                            c.position = [-0.12, 0.1];
                            c.zoom = 0.75;
                        });
                        if matches!(
                            std::env::var("ARIA_SMOKE_SCENARIO").as_deref(),
                            Ok("output-transparent") | Ok("capture-minimized")
                        ) {
                            for i in 0..3 {
                                app.outputs
                                    .edit_canvas(i, |c| c.background = Background::Transparent);
                            }
                        } else {
                            app.detect_key_color(0);
                        }
                    }
                    Ok("physics") | Ok("physics-group") => {
                        app.input_monitor.tab = Tab::Physics;
                    }
                    Ok("expressions") => {
                        app.input_monitor.tab = Tab::Expressions;
                        if let Some(entry) = app.input_monitor.expressions.entries.first() {
                            app.input_monitor
                                .saved
                                .config
                                .expressions
                                .insert(entry.file.id.clone());
                        }
                    }
                    Ok("pose") | Ok("presets") | Ok("inputs") => {
                        let parameters = app.current_parameters();
                        app.input_monitor
                            .prepare_smoke(&parameters, &app.settings.mapping);
                        app.input_monitor.tab =
                            match std::env::var("ARIA_SMOKE_SCENARIO").as_deref() {
                                Ok("pose") => Tab::Pose,
                                Ok("presets") => Tab::Presets,
                                _ => Tab::Inputs,
                            };
                        if let Some(path) = std::env::var_os("ARIA_SMOKE_AVATAR_PNG") {
                            app.input_monitor.export_png = Some(path.into());
                        }
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
            // Smoke runs may coexist with the user's running copy. Ask Windows
            // for a free loopback port and advertise it in the subscription.
            listen_port: if crate::smoke_mode() {
                0
            } else {
                self.settings.listen_port
            },
            protocol: if self.settings.source == Source::Vts {
                Protocol::VTubeStudio
            } else {
                Protocol::AriaJson
            },
        };
        match Receiver::start(config) {
            Ok(receiver) => {
                if crate::smoke_mode() {
                    self.settings.listen_port = receiver.local_port;
                }
                self.receiver = Some(receiver);
                self.snapshot = Snapshot::default();
                self.pipeline.current = Parameters::default();
                self.status_message = None;
            }
            Err(e) => self.status_message = Some(format!("{e:#}")),
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        section(ui, "STUDIO CONTROLS");
        ui.horizontal_wrapped(|ui| {
            let name = self
                .live2d
                .as_ref()
                .map(|a| a.name.as_str())
                .or_else(|| self.idle.as_ref().map(|s| s.name.as_str()))
                .unwrap_or("Mica");
            ui.label(RichText::new(name).strong().color(MINT));
            if crate::help::control(ui, "profiles", |ui| ui.small_button("Save profile")).clicked()
            {
                self.input_monitor.save_requested = true;
                self.input_monitor.message =
                    Some("This model's complete profile was saved locally.".into());
            }
        });
        theme::caption(ui, "Settings belong to this avatar.");
        theme::category(ui, "tracking-card", "Tracking & connection", true, |ui| {
            crate::help::label(ui, "Tracking source", "tracking");
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
            if self.settings.source == Source::Json {
                crate::help::label(ui, "Packet format & external tools", "json");
            }
            if self.settings.source == Source::Demo {
                ui.label(
                    RichText::new("Synthetic movement for checking your avatar and output.")
                        .color(MUTED),
                );
            } else {
                ui.scope(|ui| {
                    ui.label(if self.settings.source == Source::Vts {
                        "iPhone IPv4 address"
                    } else {
                        "Allowed sender IPv4 address"
                    });
                    crate::help::control(ui, "network", |ui| {
                        ui.add_enabled(
                            self.receiver.is_none(),
                            egui::TextEdit::singleline(&mut self.settings.sender_ip)
                                .desired_width(230.0),
                        )
                    });
                    egui::Grid::new("ports").num_columns(2).show(ui, |ui| {
                        if self.settings.source == Source::Vts {
                            ui.label("Phone request port");
                            crate::help::control(ui, "network", |ui| {
                                ui.add_enabled(
                                    self.receiver.is_none(),
                                    egui::DragValue::new(&mut self.settings.request_port)
                                        .range(1..=65535),
                                )
                            });
                            ui.end_row();
                        }
                        ui.label("PC receive port");
                        crate::help::control(ui, "network", |ui| {
                            ui.add_enabled(
                                self.receiver.is_none(),
                                egui::DragValue::new(&mut self.settings.listen_port)
                                    .range(1..=65535),
                            )
                        });
                        ui.end_row();
                    });
                });
                if self.receiver.is_some() {
                    if crate::help::control(ui, "tracking", |ui| ui.button("Disconnect")).clicked()
                    {
                        self.receiver = None;
                        self.raw = None;
                    }
                } else if crate::help::control(ui, "tracking", |ui| {
                    ui.add_sized(
                        [230.0, 36.0],
                        egui::Button::new(RichText::new("Connect tracking").color(BG)).fill(MINT),
                    )
                })
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
        });
        theme::category(ui, "movement-card", "Movement & calibration", false, |ui| {
            if crate::help::control(ui, "calibration", |ui| {
                ui.add_enabled(
                    self.raw.as_ref().is_some_and(|f| f.face_found),
                    egui::Button::new("Calibrate neutral pose"),
                )
            })
            .clicked()
                && let Some(f) = &self.raw
            {
                self.pipeline.calibrate(f);
            }
            crate::help::control(ui, "smoothing", |ui| {
                ui.add(
                    egui::Slider::new(&mut self.settings.mapping.smoothing_ms, 0.0..=300.0)
                        .text("Smooth ms"),
                )
            });
            crate::help::control(ui, "gain", |ui| {
                ui.add(
                    egui::Slider::new(&mut self.settings.mapping.head_gain, 0.1..=3.0)
                        .text("Head gain"),
                )
            });
            crate::help::control(ui, "gain", |ui| {
                ui.add(
                    egui::Slider::new(&mut self.settings.mapping.mouth_gain, 0.1..=3.0)
                        .text("Mouth gain"),
                )
            });
            crate::help::control(ui, "axes", |ui| {
                ui.checkbox(&mut self.settings.mapping.mirror, "Mirror movement")
            });
            ui.collapsing("Axis correction", |ui| {
                crate::help::control(ui, "axes", |ui| {
                    ui.checkbox(
                        &mut self.settings.mapping.invert_yaw,
                        "Invert yaw (left / right)",
                    )
                });
                crate::help::control(ui, "axes", |ui| {
                    ui.checkbox(
                        &mut self.settings.mapping.invert_pitch,
                        "Invert pitch (up / down)",
                    )
                });
                crate::help::control(ui, "axes", |ui| {
                    ui.checkbox(&mut self.settings.mapping.invert_roll, "Invert roll (tilt)")
                });
                if crate::help::control(ui, "calibration", |ui| {
                    ui.button("Reset mapping and calibration")
                })
                .clicked()
                {
                    self.settings.mapping = MappingSettings::default();
                    self.pipeline.reset();
                }
            });
        });
        theme::category(ui, "avatar-card", "Avatar & appearance", true, |ui| {
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
                if crate::help::control(ui, "avatar", |ui| ui.button("Open PNG…")).clicked() {
                    self.load_image(ctx, false);
                }
                if crate::help::control(ui, "avatar", |ui| {
                    ui.add_enabled(
                        self.idle.is_some() || self.live2d.is_some(),
                        egui::Button::new("Reset"),
                    )
                })
                .clicked()
                {
                    self.idle = None;
                    self.talking = None;
                    self.use_preview_rig();
                }
            });
            if self.idle.is_some() && self.live2d.is_none() {
                if crate::help::control(ui, "avatar", |ui| ui.button("Set talking image…"))
                    .clicked()
                {
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
            crate::help::control(ui, "avatar", |ui| {
                ui.add(egui::Slider::new(&mut self.settings.zoom, 0.5..=1.5).text("Zoom"))
            });
            if crate::help::control(ui, "avatar", |ui| ui.button("Open Live2D avatar…")).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Cubism export", &["moc3", "json"])
                    .pick_file()
            {
                self.open_model(&path);
            }
            if let Some(avatar) = &mut self.live2d {
                ui.label(RichText::new(&avatar.name).color(MINT));
                ui.collapsing("Model details", |ui| {
                    crate::help::button(ui, "avatar");
                    ui.label(
                        RichText::new(format!(
                            "{} meshes · {} tracked parameters\n{:.0} MiB atlases · Core {}",
                            avatar.model.drawables.len(),
                            self.input_monitor.saved.config.bindings.len(),
                            avatar.atlas_mib(),
                            avatar.model.version
                        ))
                        .small(),
                    );
                    for warning in &avatar.files.warnings {
                        ui.label(RichText::new(warning).small().color(MUTED));
                    }
                });
                if crate::help::control(ui, "inputs", |ui| ui.button("Model parameters…")).clicked()
                {
                    self.input_monitor.tab = Tab::Inputs;
                }
                ui.label(
                    RichText::new(format!(
                        "{} assignments from VTS profile",
                        avatar.imported_count
                    ))
                    .small(),
                );
                if crate::help::control(ui, "physics", |ui| ui.button("Configure avatar physics…"))
                    .clicked()
                {
                    self.input_monitor.tab = Tab::Physics;
                }
            }
            ui.collapsing("Cubism runtime setup", |ui| {
            ui.label("Choose Core/dll/windows/x86_64/Live2DCubismCore.dll from the official Native SDK. The path is saved locally.");
            crate::help::control(ui, "runtime", |ui| ui.text_edit_singleline(&mut self.settings.cubism_core));
            if crate::help::control(ui, "runtime", |ui| ui.button("Select Core DLL…")).clicked() && let Some(path) = rfd::FileDialog::new().add_filter("Cubism Core", &["dll"]).pick_file() {
                self.settings.cubism_core = path.display().to_string();
            }
            ui.hyperlink_to("Download the official SDK ↗", "https://www.live2d.com/en/sdk/download/native/");
            ui.hyperlink_to("Avatar import instructions ↗", "https://github.com/NekoUnix/A.R.I.A/blob/main/docs/live2d.md");
        });
            ui.collapsing("Inspect model files", |ui| {
                if crate::help::control(ui, "inspection", |ui| {
                    ui.button("Inspect Live2D .model3.json…")
                })
                .clicked()
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
            });
            ui.label(
                RichText::new("Drop a .model3.json or .moc3 here to load an avatar.")
                    .small()
                    .color(MUTED),
            );
        });
        theme::category(ui, "output-card", "Capture & performance", false, |ui| {
            if let Some(index) = self.outputs.ui(ui) {
                self.detect_key_color(index);
            }
            let selected = self.outputs.snapshot().selected;
            if let Some(status) = &self.broadcasts.status[selected] {
                ui.label(RichText::new(status).small().color(MINT));
            }
            if crate::help::control(ui, "spout", |ui| ui.small_button("Retry OBS output")).clicked()
            {
                self.broadcasts.retry();
            }
            theme::caption(
                ui,
                "OBS → Spout2 Capture → select the matching ARIA sender. Keep ARIA and OBS on the same GPU. Leave the output open; minimizing its preview keeps the full-resolution sender running.",
            );
            crate::help::label(ui, "Frame rate target", "performance");
            egui::ComboBox::from_id_salt("fps")
                .selected_text(format!("{} FPS target", self.settings.fps))
                .show_ui(ui, |ui| {
                    for fps in [30, 60, 120] {
                        if ui
                            .selectable_value(&mut self.settings.fps, fps, format!("{fps} FPS"))
                            .changed()
                        {
                            self.input_monitor.save_requested = true;
                        }
                    }
                });
            ui.collapsing("Windows performance", |ui| {
                if crate::help::control(ui, "priority", |ui| ui.add_enabled(cfg!(windows), egui::Checkbox::new(&mut self.settings.high_priority, "High process priority"))).changed() {
                    match crate::performance::set_high_priority(self.settings.high_priority) {
                        Ok(()) => {
                            self.priority_status = Some(format!("Windows priority: {}", if self.settings.high_priority { "High" } else { "Normal" }));
                            self.input_monitor.save_requested = true;
                        }
                        Err(e) => {
                            self.settings.high_priority = !self.settings.high_priority;
                            self.priority_status = Some(format!("Priority change failed: {e:#}"));
                        }
                    }
                }
                theme::caption(ui, "Gives ARIA CPU scheduling preference over normal-priority apps when Windows is busy. It may reduce CPU scheduling hitches, but cannot fix GPU overload or guarantee smooth frames. High is the strongest priority offered here; Realtime can starve Windows, input and OBS. Turn High off if other apps become less responsive. This preference applies to ARIA on this PC.");
                if let Some(status) = &self.priority_status { ui.label(RichText::new(status).small().color(MINT)); }
                theme::caption(ui, "The FPS target limits model simulation and rendering even while dragging UI controls. Static poses reuse the last model texture. Closed outputs release their capture textures.");
            });
            ui.hyperlink_to(
                "Windows setup & troubleshooting ↗",
                "https://github.com/NekoUnix/A.R.I.A/blob/main/docs/windows.md",
            );
        });
    }

    fn detect_key_color(&mut self, index: usize) {
        let result = (|| -> anyhow::Result<crate::chroma::Suggestion> {
            let mut palette = if let Some(avatar) = &self.live2d {
                avatar.key_palette()?
            } else if let Some(idle) = &self.idle {
                let mut palette = (*idle.palette).clone();
                if let Some(talking) = &self.talking {
                    palette.merge(&talking.palette);
                }
                palette
            } else {
                avatar::mica_palette()
            };
            self.items.merge_palette(&mut palette);
            palette.suggest()
        })();
        self.outputs.apply_suggestion(index, result);
    }

    fn load_image(&mut self, ctx: &egui::Context, talking: bool) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Avatar image", &["png", "jpg", "jpeg"])
            .pick_file()
        {
            match avatar::load_sprite(ctx, &path) {
                Ok(sprite) => {
                    if !talking || self.live2d.is_some() {
                        self.use_puppet_rig(&sprite.model_key);
                    }
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
        self.remember_current_rig();
        self.input_monitor = InputMonitor::new(
            avatar.model_key.clone(),
            avatar.initial_config.clone(),
            self.settings.saved_rigs.get(&avatar.model_key).cloned(),
            avatar.model.parameters(),
        );
        self.input_monitor.physics_groups = avatar
            .physics
            .as_ref()
            .map(|p| p.groups())
            .unwrap_or_default();
        self.input_monitor.expressions.load(
            &avatar.files.expressions,
            avatar.files.source.parent().unwrap_or(Path::new("")),
            &self.input_monitor.saved,
            avatar.model.parameters(),
        );
        self.restore_model_preferences(&avatar.model_key);
        self.hotkeys.configure(Vec::new());
        self.animation_time = 0.0;
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
    fn current_parameters(&self) -> Vec<RigParameter> {
        self.live2d.as_ref().map_or_else(
            || movement::preview_parameters(self.params),
            |a| a.model.parameters().to_vec(),
        )
    }
    fn remember_current_rig(&mut self) {
        self.settings.outputs = Some(self.outputs.snapshot());
        self.settings.saved_rigs.insert(
            self.input_monitor.model_key.clone(),
            self.input_monitor.saved.clone(),
        );
        let mut preferences = ModelPreferences::capture(&self.settings);
        preferences.calibration = self.pipeline.calibration();
        self.settings
            .model_preferences
            .insert(self.input_monitor.model_key.clone(), preferences);
    }
    fn restore_model_preferences(&mut self, key: &str) {
        self.items.reset();
        self.scene_revision = self.scene_revision.wrapping_add(1);
        let preferences = self
            .settings
            .model_preferences
            .get(key)
            .cloned()
            .unwrap_or_default();
        preferences.restore(&mut self.settings);
        self.outputs
            .reset(self.settings.outputs.clone().unwrap_or_else(|| {
                OutputSettings::from_legacy(
                    self.settings.background,
                    self.settings.zoom,
                    self.settings.always_on_top,
                )
            }));
        // A model switch never silently connects to a different saved sender.
        self.receiver = None;
        self.snapshot = Snapshot::default();
        self.raw = None;
        self.pipeline = ParameterPipeline::default();
        self.pipeline.restore_calibration(preferences.calibration);
        self.params = Parameters::default();
    }
    fn use_preview_rig(&mut self) {
        self.use_puppet_rig("preview-v1");
    }
    fn use_puppet_rig(&mut self, key: &str) {
        self.remember_current_rig();
        self.live2d = None;
        let parameters = movement::preview_parameters(Parameters::default());
        self.input_monitor = InputMonitor::new(
            key.into(),
            RigConfig::from_parameters(&parameters),
            self.settings.saved_rigs.get(key).cloned(),
            &parameters,
        );
        self.hotkeys.configure(Vec::new());
        self.restore_model_preferences(key);
        self.animation_time = 0.0;
    }
    fn bare_import_window(&mut self, ctx: &egui::Context) {
        let Some(path) = self.bare_moc.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Import moc3 with explicit textures").open(&mut open).default_width(600.0).show(ctx,|ui| {
            ui.label(path.display().to_string());
 crate::help::button(ui, "textures");
            ui.label("No usable matching manifest was found. Prefer opening the exported .model3.json. For a bare moc3, add each texture atlas and put them in index order (0, 1, 2…).");
            if crate::help::control(ui, "textures", |ui| ui.button("Add texture atlases…")).clicked() && let Some(files) = rfd::FileDialog::new().add_filter("PNG atlases", &["png"]).pick_files() { self.bare_textures.extend(files); }
            let mut swap = None; let mut remove = None;
            egui::ScrollArea::vertical().max_height(280.0).show(ui,|ui| {
                for (i,texture) in self.bare_textures.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{i}: {}",texture.display()));
                        if crate::help::control(ui, "textures", |ui| ui.add_enabled(i>0,egui::Button::new("Up"))).clicked() { swap = Some((i,i-1)); }
                        if crate::help::control(ui, "textures", |ui| ui.add_enabled(i+1<self.bare_textures.len(),egui::Button::new("Down"))).clicked() { swap = Some((i,i+1)); }
                        if crate::help::control(ui, "textures", |ui| ui.button("Remove")).clicked() { remove = Some(i); }
                    });
                }
            });
            if let Some((a,b)) = swap { self.bare_textures.swap(a,b); }
            if let Some(i) = remove { self.bare_textures.remove(i); }
            if crate::help::control(ui, "textures", |ui| ui.add_enabled(!self.bare_textures.is_empty(),egui::Button::new("Load avatar"))).clicked() {
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
        let parameters = self.current_parameters();
        let labels = self
            .live2d
            .as_ref()
            .map(|a| a.labels.clone())
            .unwrap_or_default();
        let monitor_key = self.input_monitor.model_key.clone();
        ui.push_id(&monitor_key, |ui| {
            self.input_monitor.ui(
                ui,
                &parameters,
                &labels,
                &self.live_inputs,
                &mut self.settings.mapping,
                true,
            );
            if self.input_monitor.tab == Tab::Items {
                if self.items.panel(
                    ui,
                    &mut self.input_monitor.saved,
                    &self.live_inputs,
                    &parameters,
                ) {
                    self.items.edited();
                }
                if let Some(error) = &self.input_monitor.hotkey_status {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            }
        });
        if self.input_monitor.tab == Tab::Raw {
            crate::help::label(ui, "Received tracking data", "diagnostics");
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
        }

        theme::category(
            ui,
            "diagnostic-details",
            "Connection diagnostics & export",
            false,
            |ui| {
                if crate::help::control(ui, "diagnostics", |ui| ui.button("Export mapped values…"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name("aria-parameters.json")
                        .add_filter("JSON", &["json"])
                        .save_file()
                {
                    match serde_json::to_vec_pretty(
                        &parameters
                            .iter()
                            .map(|p| (p.id.as_str(), p.value))
                            .collect::<BTreeMap<_, _>>(),
                    )
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
            },
        );
    }

    fn model_window(&mut self, ctx: &egui::Context) {
        if let Some(report) = &self.model {
            egui::Window::new("Live2D asset inspection").open(&mut self.model_open).default_width(670.0).show(ctx, |ui| {
                crate::help::button(ui, "inspection");
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
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && ctx.current_pass_index() == 0
            && let Some(path) = std::env::var_os("ARIA_SMOKE_ITEM")
        {
            let key = egui::Id::new("smoke-drop-png");
            if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                ctx.input_mut(|i| {
                    i.raw.dropped_files.push(egui::DroppedFile {
                        path: Some(path.into()),
                        ..Default::default()
                    })
                });
                ctx.data_mut(|d| d.insert_temp(key, true));
            }
        }
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        if ctx.current_pass_index() == 0
            && let Some(path) = dropped.iter().find(|p| !crate::items::is_png(p))
        {
            self.open_model(path);
        }
        let mut dropped_pngs: Vec<_> = if ctx.current_pass_index() == 0 {
            dropped
                .into_iter()
                .filter(|p| crate::items::is_png(p))
                .collect()
        } else {
            Vec::new()
        };
        let now = Instant::now();
        // A layout discard can call update twice for the same frame.
        let dt = self
            .frame_clock
            .tick(now, self.settings.fps, ctx.current_pass_index() == 0);
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
        for event in self.hotkeys.events() {
            match event {
                crate::hotkeys::Event::Pressed { generation, action }
                    if generation == self.hotkeys.generation
                        && self.input_monitor.saved.global_hotkeys =>
                {
                    let parameters = self.current_parameters();
                    self.input_monitor.hotkey_action(
                        action,
                        &parameters,
                        &mut self.settings.mapping,
                    );
                }
                crate::hotkeys::Event::Registered {
                    generation,
                    errors,
                    thread_id,
                } if generation == self.hotkeys.generation => {
                    let _ = thread_id;
                    self.input_monitor.hotkey_status = if errors.is_empty() {
                        None
                    } else {
                        Some(errors.join("\n"))
                    };
                }
                _ => {}
            }
        }
        if std::mem::take(&mut self.input_monitor.reset_item_rules) {
            self.items.reset_rules();
        }
        if dt > 0.0 {
            self.params = self
                .pipeline
                .update(self.raw.as_ref(), &self.settings.mapping, dt);
            if self.input_monitor.saved.config.pose.mode != PoseMode::Frozen {
                self.animation_time += dt.min(0.25);
            }
            self.live_inputs = rig::tracking_inputs(
                self.raw.as_ref(),
                self.params,
                self.settings.mapping.mirror,
                self.animation_time,
            );
            if let Some(avatar) = &mut self.live2d {
                if std::mem::take(&mut self.input_monitor.reset_motion) {
                    avatar.reset_motion();
                }
                match avatar.update(
                    &self.live_inputs,
                    &mut self.input_monitor.saved.config,
                    &mut self.input_monitor.expressions,
                    dt,
                ) {
                    Ok(true) => self.scene_revision = self.scene_revision.wrapping_add(1),
                    Ok(false) => {}
                    Err(error) => {
                        self.status_message = Some(format!("Live2D update stopped: {error:#}"));
                        self.use_preview_rig();
                    }
                }
            } else {
                self.scene_revision = self.scene_revision.wrapping_add(1);
                let mut parameters = movement::preview_parameters(self.params);
                self.input_monitor.saved.config.evaluate(
                    &self.live_inputs,
                    &mut parameters,
                    dt,
                    None,
                );
                for (value, p) in self.params.0.iter_mut().zip(parameters) {
                    *value = p.value;
                }
            }
            let parameters = self.current_parameters();
            if self.items.evaluate(
                &mut self.input_monitor.saved.config,
                &self.live_inputs,
                &parameters,
            ) {
                self.items.edited();
            }
        }
        self.items.sync_assets(
            ctx,
            self.render_state.as_ref(),
            &self.input_monitor.saved.config,
        );

        self.metrics.update(self.render_state.as_ref());
        egui::TopBottomPanel::top("header")
            .frame(Frame::new().fill(BG).inner_margin(18.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("A.R.I.A.").size(26.0).strong().color(MINT));
                    ui.label(RichText::new("AVATAR STUDIO").size(12.0).color(MUTED));
                    crate::help::button(ui, "welcome");
                    if ui.small_button("Help & documentation").clicked() {
                        crate::help::open(ctx, "welcome", String::new());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("v0.10 · WINDOWS PREVIEW")
                                .small()
                                .color(MUTED),
                        );
                    });
                });
            });
        egui::TopBottomPanel::bottom("status")
            .frame(Frame::new().fill(BG).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&self.gpu).small().color(MUTED));
                    ui.separator();
                    ui.label(RichText::new(format!("{:.0} model FPS", self.render_fps)).small());
                    crate::help::button(ui, "performance");
                    self.metrics.footer(ui);
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
            .exact_width(302.0)
            .resizable(false)
            .frame(Frame::new().fill(PANEL).inner_margin(12.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("controls-scroll")
                    .show(ui, |ui| self.controls(ui, ctx));
            });
        egui::SidePanel::right("diagnostics")
            .default_width(380.0)
            .width_range(330.0..=700.0)
            .resizable(true)
            .frame(Frame::new().fill(PANEL).inner_margin(12.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("diagnostics-scroll")
                    .show(ui, |ui| self.diagnostics(ui));
            });
        let output_settings = self.outputs.snapshot();
        let stage_output = output_settings.canvas(output_settings.selected);
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(20.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Your stage");
                    crate::help::button(ui, "stage");
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
                ui.horizontal_wrapped(|ui| {
                    crate::help::button(ui, "png-items");
                    if ui
                        .small_button("Drop PNGs here · items & toggles")
                        .clicked()
                    {
                        self.input_monitor.tab = Tab::Items;
                    }
                });
                let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                let old_revision = self.items.revision;
                if !dropped_pngs.is_empty() {
                    let pointer = ctx.input(|i| i.pointer.latest_pos());
                    if pointer.is_none_or(|p| rect.contains(p)) {
                        let scene = self.scene();
                        let position = crate::items::Items::drop_position(
                            &scene,
                            rect,
                            self.settings.zoom,
                            pointer.or(self.items.last_hover),
                        );
                        if self.items.add(
                            std::mem::take(&mut dropped_pngs),
                            &mut self.input_monitor.saved.config,
                            position,
                        ) {
                            self.items.edited();
                        }
                        self.input_monitor.tab = Tab::Items;
                    } else {
                        self.items.message = Some(
                            "Drop PNG images inside Your stage, or use Add PNGs in the items tab."
                                .into(),
                        );
                        self.input_monitor.tab = Tab::Items;
                    }
                }
                self.items.sync_assets(
                    ctx,
                    self.render_state.as_ref(),
                    &self.input_monitor.saved.config,
                );
                self.items
                    .refresh(&self.input_monitor.saved.config, self.live2d.as_ref());
                let scene = self.scene();
                let previous_selection = self.items.selected;
                #[cfg(feature = "screenshots")]
                if crate::smoke_mode() && std::env::var_os("ARIA_SMOKE_ITEM").is_some() {
                    let key = egui::Id::new("smoke-pinned-png");
                    if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                        self.items.prepare_smoke(
                            &mut self.input_monitor.saved.config,
                            self.live2d.as_ref(),
                        );
                        ctx.data_mut(|d| d.insert_temp(key, true));
                    }
                }
                if self.items.stage(
                    ui,
                    rect,
                    &scene,
                    &mut self.input_monitor.saved.config,
                    self.live2d.as_ref(),
                    self.settings.zoom,
                ) {
                    self.items.edited();
                }
                if self.items.selected.is_some() && self.items.selected != previous_selection {
                    self.input_monitor.tab = Tab::Items;
                }
                self.items
                    .refresh(&self.input_monitor.saved.config, self.live2d.as_ref());
                if self.items.revision != old_revision {
                    self.scene_revision = self.scene_revision.wrapping_add(1);
                }
                let scene = self.scene();
                let painter = ui.painter_at(rect);
                painter.rect_filled(
                    rect,
                    12.0,
                    if stage_output.background == Background::Transparent {
                        Color32::from_rgb(30, 34, 48)
                    } else {
                        stage_output.background.color(stage_output.key)
                    },
                );
                if stage_output.background == Background::Studio {
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
                crate::items::paint(&painter, &scene, rect, self.settings.zoom, true);
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
                crate::items::paint(&painter, &scene, rect, self.settings.zoom, false);
                self.items
                    .selection(&painter, &scene, rect, self.settings.zoom);
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
        if let Some(path) = self.input_monitor.export_png.take() {
            let scene = self.scene();
            let result = if scene.items.is_empty()
                && let Some(avatar) = &self.live2d
            {
                avatar.save_png(&path)
            } else {
                self.render_state
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("GPU renderer unavailable"))
                    .and_then(|state| crate::broadcast::save_png(ctx, state, &scene, &path))
            };
            self.input_monitor.message = Some(match result {
                Ok(()) => format!(
                    "Saved transparent PNG with avatar and items: {}",
                    path.display()
                ),
                Err(e) => format!("PNG export failed: {e:#}"),
            });
        }
        self.model_window(ctx);
        self.bare_import_window(ctx);
        if !self.hotkeys.available() {
            self.input_monitor.hotkey_status =
                Some("Hotkey worker could not start. Preset buttons remain available.".into());
        }
        self.hotkeys.configure(self.input_monitor.hotkey_keys());
        self.input_monitor.save_requested |= self.outputs.take_dirty();
        self.input_monitor.save_requested |= self.items.take_save();
        if std::mem::take(&mut self.input_monitor.save_requested) && !crate::smoke_mode() {
            self.remember_current_rig();
            if let Some(storage) = frame.storage_mut() {
                eframe::set_value(storage, "aria-settings-v1", &self.settings);
                storage.flush();
            }
        }
        #[cfg(feature = "screenshots")]
        {
            if crate::smoke_mode()
                && std::env::var_os("ARIA_SMOKE_ITEM").is_some()
                && self.started.elapsed()
                    > crate::screenshot::delay().saturating_sub(Duration::from_millis(750))
            {
                let key = egui::Id::new("smoke-verify-png");
                if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                    assert_eq!(self.items.draws.len(), 2, "Both PNG textures must load");
                    assert!(
                        self.items
                            .draws
                            .iter()
                            .all(|d| d.anchor != crate::items::Anchor::Missing && d.visible)
                    );
                    if std::env::var_os("ARIA_TEST_MODEL").is_some() {
                        assert!(
                            self.live2d.is_some(),
                            "PNG drop must preserve the loaded avatar"
                        );
                    }
                    eprintln!(
                        "PNG drop and pin smoke verified: {:?}",
                        self.items.draws[0].anchor
                    );
                    if let Some(path) = std::env::var_os("ARIA_SMOKE_AVATAR_PNG") {
                        self.input_monitor.export_png = Some(path.into());
                    }
                    ctx.data_mut(|d| d.insert_temp(key, true));
                }
            }
            if crate::smoke_mode()
                && self.settings.source == Source::Vts
                && self.started.elapsed() > crate::screenshot::delay()
            {
                assert!(
                    self.snapshot.fresh_frame().is_some_and(|f| f.face_found),
                    "Smoke run needs fresh VTS tracking, not a disconnected screenshot"
                );
            }
            screenshot_capture(ctx, self.started, false);
        }
        if self.outputs.any_open() {
            let scene = self.scene();
            if let Some(state) = &self.render_state {
                self.broadcasts.update(
                    ctx,
                    state,
                    self.outputs.broadcasts(),
                    &scene,
                    self.scene_revision,
                );
            }
            self.outputs
                .show(ctx, scene, self.settings.fps, self.started);
            #[cfg(feature = "screenshots")]
            if crate::smoke_mode() && self.started.elapsed() > Duration::from_secs(2) {
                let scenario = std::env::var("ARIA_SMOKE_SCENARIO").unwrap_or_default();
                let key = egui::Id::new("smoke-native-window-change");
                if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                    if scenario == "output-resize" {
                        self.outputs.edit_canvas(2, |c| {
                            c.freeform_window = [600, 400];
                            c.freeform_size = [2048, 1024];
                        });
                    } else if scenario == "capture-minimized" {
                        for i in 0..3 {
                            ctx.send_viewport_cmd_to(
                                crate::output::viewport_id(i),
                                egui::ViewportCommand::Minimized(true),
                            );
                        }
                    }
                    ctx.data_mut(|d| d.insert_temp(key, true));
                }
                if scenario == "capture-minimized"
                    && self.started.elapsed() > Duration::from_secs(3)
                {
                    let key = key.with("verified");
                    if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                        assert!(
                            ctx.input(|i| (0..3).all(|n| i
                                .raw
                                .viewports
                                .get(&crate::output::viewport_id(n))
                                .is_some_and(|v| v.minimized == Some(true)))),
                            "All output previews should be minimized"
                        );
                        eprintln!(
                            "All three output previews verified minimized; GPU senders remain active"
                        );
                        ctx.data_mut(|d| d.insert_temp(key, true));
                    }
                }
            }
        } else {
            // Drop senders immediately when their last preview is closed.
            self.broadcasts.retry();
        }
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode() {
            let scenario = std::env::var("ARIA_SMOKE_SCENARIO").unwrap_or_default();
            if let Some(topic) = scenario.strip_prefix("help-") {
                let key = egui::Id::new("help-smoke-open");
                if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                    if let Some(a) = crate::help::articles().iter().find(|a| a.id == topic) {
                        let context = if a.id == "parameter" {
                            self.current_parameters()
                                .first()
                                .map_or_else(String::new, |p| {
                                    crate::help::parameter_context(p, &p.id)
                                })
                        } else {
                            String::new()
                        };
                        crate::help::open(ctx, a.id, context);
                    }
                    ctx.data_mut(|d| d.insert_temp(key, true));
                }
            }
        }
        crate::help::show(ctx, self.started);
        let remaining = self
            .frame_clock
            .remaining(Instant::now(), self.settings.fps);
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
        self.remember_current_rig();
        eframe::set_value(storage, "aria-settings-v1", &self.settings);
    }
}

#[cfg(feature = "screenshots")]
fn screenshot_capture(ctx: &egui::Context, started: Instant, output: bool) {
    crate::screenshot::capture(ctx, started, output);
}

fn section(ui: &mut egui::Ui, label: &str) {
    ui.add_space(5.0);
    crate::help::label(
        ui,
        RichText::new(label).size(11.0).strong().color(MUTED),
        match label {
            "STUDIO CONTROLS" => "profiles",
            "INPUT MONITOR" => "inputs",
            _ => "diagnostics",
        },
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Memory(BTreeMap<String, String>);
    impl eframe::Storage for Memory {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn flush(&mut self) {}
    }
    #[test]
    fn independent_model_profiles_round_trip_through_actual_app_storage_format() {
        let mut settings = Settings {
            source: Source::Vts,
            sender_ip: "192.0.2.1".into(),
            zoom: 1.3,
            background: Background::Green,
            fps: 30,
            high_priority: true,
            ..Default::default()
        };
        settings.mapping.head_gain = 1.8;
        let mut a = ModelPreferences::capture(&settings);
        a.calibration.y = 17.0;
        settings.model_preferences.insert("avatar-a".into(), a);
        settings.source = Source::Json;
        settings.sender_ip = "127.0.0.1".into();
        settings.zoom = 0.7;
        settings.background = Background::Transparent;
        settings.fps = 120;
        settings.mapping.head_gain = 0.6;
        settings
            .model_preferences
            .insert("avatar-b".into(), ModelPreferences::capture(&settings));
        let parameters = movement::preview_parameters(Parameters::default());
        for (key, angle, strength) in [("avatar-a", 12.0, 0.4), ("avatar-b", -8.0, 1.7)] {
            let mut saved = SavedRig {
                config: RigConfig::from_parameters(&parameters),
                ..Default::default()
            };
            saved.config.capture_pose(&parameters);
            saved.config.pose.frozen.insert("ParamAngleX".into(), angle);
            saved.config.items.push(aria_core::items::Item {
                id: 1_800_000_000_000_000_001,
                name: key.into(),
                path: format!("{key}.png").into(),
                position: [angle / 30.0, 0.0],
                ..Default::default()
            });
            saved.config.physics.groups.insert(
                "Custom spring".into(),
                aria_core::physics::GroupSettings {
                    strength,
                    ..Default::default()
                },
            );
            settings.saved_rigs.insert(key.into(), saved);
        }
        let expressions = &mut settings.saved_rigs.get_mut("avatar-a").unwrap();
        expressions.expression_hotkeys.insert(
            "smile.exp3.json".into(),
            aria_core::shortcuts::Shortcut {
                ctrl: true,
                shift: true,
                key: 0x48,
                ..Default::default()
            },
        );
        expressions
            .config
            .expressions
            .insert("smile.exp3.json".into());
        expressions
            .expression_files
            .push("C:/avatar/smile.exp3.json".into());
        let mut a_outputs = OutputSettings::default();
        a_outputs.canvases[0].position = [0.25, -0.1];
        a_outputs.canvases[0].key = [255, 60, 0];
        a_outputs.canvases[1].background = Background::Transparent;
        a_outputs.canvases[1].position = [-0.3, 0.2];
        a_outputs.freeform.freeform_size = [1536, 1024];
        a_outputs.freeform.freeform_window = [480, 320];
        a_outputs.freeform.position = [0.1, -0.1];
        a_outputs.freeform.zoom = 1.2;
        a_outputs.selected = 2;
        settings
            .model_preferences
            .get_mut("avatar-a")
            .unwrap()
            .outputs = Some(a_outputs.clone());
        settings
            .model_preferences
            .get_mut("avatar-b")
            .unwrap()
            .outputs = Some(OutputSettings::default());
        let mut storage = Memory::default();
        eframe::set_value(&mut storage, "aria-settings-v1", &settings);
        let mut restored: Settings =
            eframe::get_value(&storage, "aria-settings-v1").expect("RON settings restore");
        assert_eq!(
            restored.saved_rigs["avatar-a"].config.items[0].name,
            "avatar-a"
        );
        assert_eq!(
            restored.saved_rigs["avatar-b"].config.items[0].name,
            "avatar-b"
        );
        assert_ne!(
            restored.saved_rigs["avatar-a"].config.items[0].position,
            restored.saved_rigs["avatar-b"].config.items[0].position
        );
        for _ in 0..3 {
            restored.model_preferences["avatar-a"]
                .clone()
                .restore(&mut restored);
            assert_eq!(restored.zoom, 1.3);
            assert_eq!(restored.mapping.head_gain, 1.8);
            assert!(restored.source == Source::Vts && restored.background == Background::Green);
            assert_eq!(restored.sender_ip, "192.0.2.1");
            assert_eq!(restored.fps, 30);
            assert_eq!(restored.outputs.as_ref().unwrap(), &a_outputs);
            assert_eq!(restored.model_preferences["avatar-a"].calibration.y, 17.0);
            restored.model_preferences["avatar-b"]
                .clone()
                .restore(&mut restored);
            assert_eq!(restored.zoom, 0.7);
            assert_eq!(restored.mapping.head_gain, 0.6);
            assert!(
                restored.source == Source::Json && restored.background == Background::Transparent
            );
            assert_eq!(restored.sender_ip, "127.0.0.1");
            assert_eq!(restored.fps, 120);
            assert!(restored.high_priority);
            assert_eq!(
                restored.outputs.as_ref().unwrap(),
                &OutputSettings::default()
            );
        }
        assert_eq!(
            restored.saved_rigs["avatar-a"].config.pose.frozen["ParamAngleX"],
            12.0
        );
        assert_eq!(
            restored.saved_rigs["avatar-b"].config.pose.frozen["ParamAngleX"],
            -8.0
        );
        assert_eq!(
            restored.saved_rigs["avatar-a"].config.physics.groups["Custom spring"].strength,
            0.4
        );
        assert_eq!(
            restored.saved_rigs["avatar-b"].config.physics.groups["Custom spring"].strength,
            1.7
        );
        for rig in restored.saved_rigs.values() {
            rig.validate(&parameters).unwrap();
        }
        assert_eq!(
            restored.saved_rigs["avatar-a"].expression_hotkeys["smile.exp3.json"].label(),
            "Ctrl+Shift+H"
        );
        assert!(
            restored.saved_rigs["avatar-a"]
                .config
                .expressions
                .contains("smile.exp3.json")
        );
        assert_eq!(restored.saved_rigs["avatar-a"].expression_files.len(), 1);
        assert!(
            restored.saved_rigs["avatar-b"]
                .expression_hotkeys
                .is_empty()
        );
        assert!(
            restored.saved_rigs["avatar-b"]
                .config
                .expressions
                .is_empty()
        );
    }
    #[test]
    fn v07_two_canvas_ron_profiles_migrate_without_losing_framing() {
        #[derive(Serialize)]
        struct Legacy {
            canvases: [crate::output::CanvasSettings; 2],
            selected: usize,
        }
        let mut legacy = Legacy {
            canvases: Default::default(),
            selected: 1,
        };
        legacy.canvases[0].long_edge = 1280;
        legacy.canvases[1].position = [-0.3, 0.25];
        let mut memory = Memory::default();
        eframe::set_value(&mut memory, "output", &legacy);
        let restored: OutputSettings = eframe::get_value(&memory, "output").unwrap();
        assert_eq!(restored.selected, 1);
        assert_eq!(restored.canvases[0].pixels(0), [1280, 720]);
        assert_eq!(restored.canvases[1].position, [-0.3, 0.25]);
        assert_eq!(restored.freeform.pixels(2), [1280, 720]);
    }
}
