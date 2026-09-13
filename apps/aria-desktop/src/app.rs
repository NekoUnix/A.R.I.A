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

use crate::theme::{self, bg, mint, muted, panel};

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
enum Source {
    #[default]
    Demo,
    Vts,
    Json,
    Local,
    Webcam,
    Rtx,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    chat_accounts: crate::chat::Accounts,
    image_avatar: Option<PathBuf>,
    vrm_avatar: Option<PathBuf>,
    theme: crate::theme::Settings,
    camera_runtime: crate::webcam::Runtime,
    camera: crate::webcam::Settings,
    effect_api: crate::effect_api::Settings,
    #[serde(default)]
    vts_pitch_revision: u8,
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
            chat_accounts: Default::default(),
            image_avatar: None,
            vrm_avatar: None,
            theme: Default::default(),
            camera_runtime: Default::default(),
            camera: Default::default(),
            effect_api: Default::default(),
            vts_pitch_revision: 1,
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

impl Settings {
    fn migrate_vts_pitch(&mut self) {
        if self.vts_pitch_revision == 0 {
            for p in self
                .model_preferences
                .values_mut()
                .filter(|p| p.source == Source::Vts)
            {
                p.calibration.x = -p.calibration.x;
            }
            self.vts_pitch_revision = 1;
        }
    }
}

/// Per-avatar controls outside its native rig. The SDK path is a machine preference.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct ModelPreferences {
    camera: crate::webcam::Settings,
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
            camera: settings.camera.clone(),
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
        settings.camera = self.camera.clone();
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

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
enum ControlsPage {
    #[default]
    Avatar,
    Tracking,
    Output,
    Chat,
    Settings,
}

struct PendingImage {
    path: PathBuf,
    talking: bool,
    artwork: Vec<crate::avatar_import::Artwork>,
    budget: u32,
    job: crate::media::LoadJob,
}
pub struct AriaApp {
    camera: crate::webcam::Camera,
    api_snapshot_at: Instant,
    tracking_guide: crate::tracking_guide::Guide,
    tracking_filter: aria_core::calibration::Filter,
    speech_filter: aria_core::speech::Filter,
    importer: crate::avatar_import::Wizard,
    pending_image: Option<PendingImage>,
    pending_vrm: Option<crate::vrm::LoadJob>,
    controls_page: ControlsPage,
    chats: crate::chat::Chats,
    images: crate::image_actions::Images,
    microphone: crate::microphone::Microphone,
    controller: crate::controller::Controller,
    effects: crate::effects::Effects,
    effect_api: crate::effect_api::Api,
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
    vrm: Option<crate::vrm::Avatar>,
    render_state: Option<eframe::egui_wgpu::RenderState>,
    bare_moc: Option<PathBuf>,
    bare_textures: Vec<PathBuf>,
}

impl AriaApp {
    fn scene(&self) -> crate::output::Scene {
        crate::output::Scene {
            images: self.images.draws.clone(),
            dents: self.effects.dents.clone(),
            effects: self.effects.draws.clone(),
            recoil: self.effects.simulation.impulse,
            _model_lease: self
                .live2d
                .as_ref()
                .map(|a| a.image_lease())
                .or_else(|| self.vrm.as_ref().map(|a| a.image_lease())),
            items: self.items.draws.clone(),
            model: self
                .live2d
                .as_ref()
                .map(|a| a.image())
                .or_else(|| self.vrm.as_ref().map(|a| a.image())),
            model_bounds: self
                .live2d
                .as_ref()
                .map(|a| a.image_bounds())
                .or_else(|| self.vrm.as_ref().map(|a| a.image_bounds()))
                .unwrap_or(egui::Rect::NOTHING),
            sprite: self.images.primary().cloned().or_else(|| {
                self.active_sprite()
                    .map(|s| s.at(self.animation_time, 1.0, true))
            }),
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
        theme::apply(&cc.egui_ctx, settings.theme.active.colors);
        settings.migrate_vts_pitch();
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
            camera: Default::default(),
            api_snapshot_at: Instant::now(),
            tracking_guide: Default::default(),
            tracking_filter: Default::default(),
            speech_filter: Default::default(),
            importer: Default::default(),
            pending_image: None,
            pending_vrm: None,
            controls_page: ControlsPage::default(),
            chats: Default::default(),
            images: Default::default(),
            microphone: Default::default(),
            controller: Default::default(),
            effects: Default::default(),
            effect_api: Default::default(),
            settings,
            receiver: None,
            snapshot: Snapshot::default(),
            pipeline: ParameterPipeline::default(),
            raw: None,
            params: Parameters::default(),
            started: Instant::now(),
            frame_clock: crate::performance::FrameClock::new(Instant::now()),
            scene_revision: 0,
            render_fps: 0.0,
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
            vrm: None,
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
            let path = Path::new(&path);
            if path.extension().is_some_and(|e| {
                ["png", "gif", "jpg", "jpeg"]
                    .iter()
                    .any(|x| e.eq_ignore_ascii_case(x))
            }) {
                app.open_image(&cc.egui_ctx, path, false);
            } else {
                app.open_model(path);
            }
        }
        if !crate::smoke_mode()
            && std::env::args_os().nth(1).is_none()
            && let Some(path) = app.settings.image_avatar.clone()
        {
            app.open_image(&cc.egui_ctx, &path, false);
        }
        if !crate::smoke_mode()
            && std::env::args_os().nth(1).is_none()
            && let Some(path) = app.settings.vrm_avatar.clone()
        {
            app.begin_vrm(&path);
        }
        #[cfg(feature = "screenshots")]
        let app = {
            let mut app = app;
            if crate::smoke_mode() {
                if let Ok(token) = std::env::var("ARIA_SMOKE_API_KEY") {
                    app.settings.effect_api = crate::effect_api::Settings {
                        enabled: true,
                        token,
                        port: std::env::var("ARIA_SMOKE_API_PORT")
                            .unwrap()
                            .parse()
                            .unwrap(),
                    };
                }
                if let Some(path) = std::env::var_os("ARIA_TEST_MODEL") {
                    app.import_model(
                        aria_model::load_files(Path::new(&path)).expect("Smoke model assets"),
                    )
                    .expect("Smoke model load");
                }
                if let Some(path) = std::env::var_os("ARIA_TEST_VRM") {
                    app.begin_vrm(Path::new(&path));
                }
                let scenario = std::env::var("ARIA_SMOKE_SCENARIO").unwrap_or_default();
                app.controls_page = if scenario.starts_with("chat") {
                    ControlsPage::Chat
                } else if scenario.starts_with("output") || scenario == "capture-controls" {
                    ControlsPage::Output
                } else if matches!(
                    scenario.as_str(),
                    "inputs" | "microphone" | "vts" | "responsiveness" | "performance-details"
                ) {
                    ControlsPage::Tracking
                } else {
                    ControlsPage::Avatar
                };
                if scenario.starts_with("theme-") || scenario == "api-controls" {
                    app.controls_page = ControlsPage::Settings;
                    if scenario == "theme-light" {
                        app.settings.theme.active = theme::presets().remove(1);
                    }
                    if scenario == "theme-sakura" {
                        app.settings.theme.active = theme::presets().remove(2);
                    }
                }
                if scenario == "camera-controls" || scenario == "rtx-controls" {
                    app.controls_page = ControlsPage::Tracking;
                    app.settings.source = if scenario == "rtx-controls" {
                        Source::Rtx
                    } else {
                        Source::Webcam
                    };
                }
                if scenario == "odette-gifs" {
                    let root = PathBuf::from(
                        std::env::var_os("ARIA_TEST_GIF_DIR").expect("ARIA_TEST_GIF_DIR"),
                    );
                    let mut paths: Vec<_> = std::fs::read_dir(root)
                        .unwrap()
                        .filter_map(Result::ok)
                        .map(|e| e.path())
                        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif")))
                        .collect();
                    paths.sort();
                    app.importer.start(Some(crate::avatar_import::Kind::Images));
                    app.importer.add(paths);
                    let artwork = app.importer.artwork.clone();
                    let path = artwork
                        .iter()
                        .find(|a| a.trigger == aria_core::image_actions::Trigger::Idle)
                        .unwrap()
                        .path
                        .clone();
                    app.begin_image(&cc.egui_ctx, &path, false, artwork, 256);
                    app.importer.open = false;
                }
                if scenario == "import-choice" {
                    app.importer.start(None);
                }
                if scenario == "import-images" {
                    app.importer.start(Some(crate::avatar_import::Kind::Images));
                }
                if scenario == "import-vrm" {
                    app.importer.start(Some(crate::avatar_import::Kind::Vrm));
                }
                if scenario == "import-live2d" {
                    app.importer.start(Some(crate::avatar_import::Kind::Live2d));
                }
                match std::env::var("ARIA_SMOKE_SCENARIO").as_deref() {
                    Ok("image-actions") | Ok("output-images") | Ok("microphone") => {
                        let root =
                            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/images");
                        app.open_image(&cc.egui_ctx, &root.join("artwork/idle.png"), false);
                        app.input_monitor.saved.config.images =
                            crate::image_actions::import_config(
                                &root.join("starter.aria-images.json"),
                            )
                            .unwrap();
                        app.input_monitor.saved.config.images.manual = Some(4);
                        app.input_monitor.saved.config.images.states[3]
                            .motion
                            .repeat = true;
                        app.images.select_smoke(4);
                        app.input_monitor
                            .saved
                            .config
                            .items
                            .push(aria_core::items::Item {
                                id: 901,
                                name: "Animated GIF accessory".into(),
                                path: root.join("artwork/excited.gif"),
                                position: [-0.32, -0.12],
                                height: 0.22,
                                ..Default::default()
                            });
                        app.input_monitor.saved.effects.muted = true;
                        app.input_monitor
                            .saved
                            .effects
                            .designs
                            .push(aria_core::effects::Design {
                                id: 66,
                                name: "Animated GIF throws".into(),
                                assets: vec![root.join("artwork/excited.gif")],
                                count: 3,
                                interval: 0.25,
                                flight: 0.4,
                                lifetime: 15.0,
                                stickiness: Some(1.0),
                                size: 0.15,
                                deformation: aria_core::deformation::Settings::gentle(),
                                ..Default::default()
                            });
                        app.effects.pending.push(66);
                        let scenario = std::env::var("ARIA_SMOKE_SCENARIO").unwrap();
                        if scenario == "microphone" {
                            app.input_monitor.saved.microphone.enabled = true;
                            app.settings.source = Source::Local;
                            app.input_monitor.tab = Tab::Microphone;
                        } else {
                            app.input_monitor.tab = Tab::Images;
                        }
                        if scenario == "output-images" {
                            for i in 0..3 {
                                app.outputs.set_open(i, true);
                                app.outputs
                                    .edit_canvas(i, |c| c.background = Background::Transparent);
                            }
                            app.outputs.edit_canvas(2, |c| {
                                c.freeform_size = [1536, 1024];
                                c.freeform_window = [480, 320];
                            });
                        }
                    }
                    Ok("effect-deformation") => {
                        app.input_monitor.tab = Tab::Effects;
                        app.input_monitor.saved.effects.muted = true;
                        app.effects.editor =
                            Some(Box::new(crate::effect_editor::Editor::deformation_smoke()));
                    }
                    Ok("effect-editor") => {
                        app.input_monitor.tab = Tab::Effects;
                        app.input_monitor.saved.effects.muted = true;
                        app.effects.editor = Some(Box::new(crate::effect_editor::Editor::smoke()));
                    }
                    Ok("effects") | Ok("output-effects") | Ok("effects-audio") => {
                        app.input_monitor.tab = Tab::Effects;
                        app.effects.selected = Some(4);
                        let library = &mut app.input_monitor.saved.effects;
                        library.muted =
                            std::env::var("ARIA_SMOKE_SCENARIO").as_deref() != Ok("effects-audio");
                        library.volume = 0.08;
                        for d in &mut library.designs {
                            d.lifetime = 10.0;
                            d.impact = 0.0;
                            if d.kind == aria_core::effects::Kind::Spray {
                                d.count = 55;
                                d.flight = 0.3;
                                d.spread = 0.2;
                            } else {
                                d.count = 3;
                                d.flight = 2.5;
                                d.interval = 0.3;
                            }
                        }
                        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../../templates/effects/assets");
                        let mut assets: Vec<_> =
                            ["star.png", "cube.glb", "cube.gltf", "cube.obj", "cube.fbx"]
                                .iter()
                                .map(|p| root.join(p))
                                .collect();
                        if let Some(path) = std::env::var_os("ARIA_SMOKE_EFFECT_ASSET") {
                            assets.push(path.into());
                        }
                        library.designs.push(aria_core::effects::Design {
                            id: 6,
                            name: "Native asset import check".into(),
                            deformation: aria_core::deformation::Settings {
                                avatar: aria_core::deformation::Response {
                                    enabled: true,
                                    hold: 4.0,
                                    ..Default::default()
                                },
                                object: aria_core::deformation::Response {
                                    enabled: true,
                                    radius: 0.8,
                                    hold: 4.0,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                            assets,
                            routes: vec![
                                aria_core::effects::Route {
                                    origin: [-0.8, -0.3],
                                    target: [-0.08, -0.16],
                                },
                                aria_core::effects::Route {
                                    origin: [0.8, -0.3],
                                    target: [0.08, -0.16],
                                },
                                aria_core::effects::Route {
                                    origin: [0.0, -0.8],
                                    target: [0.0, -0.16],
                                },
                            ],
                            stickiness: Some(1.0),
                            count: 1,
                            selection: aria_core::effects::Selection::All,
                            flight: 0.3,
                            interval: 0.05,
                            size: 0.2,
                            lifetime: 10.0,
                            ..Default::default()
                        });
                        app.effects.pending.extend([1, 3, 4, 5, 6]);
                        if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("output-effects") {
                            for i in 0..3 {
                                app.outputs.set_open(i, true);
                                app.outputs
                                    .edit_canvas(i, |c| c.background = Background::Transparent);
                            }
                            app.outputs.edit_canvas(2, |c| {
                                c.freeform_size = [1536, 1024];
                                c.freeform_window = [480, 320];
                            });
                        }
                    }
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
                    Ok("layers") => {
                        app.input_monitor.tab = Tab::Layers;
                        if let Some(avatar) = &app.live2d {
                            app.input_monitor
                                .layers_panel
                                .prepare_smoke(avatar, &mut app.input_monitor.saved);
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
                    Ok("controller") => {
                        app.input_monitor.tab = Tab::Controller;
                        if let Some(entry) = app
                            .input_monitor
                            .expressions
                            .entries
                            .iter()
                            .find(|e| e.is_controller_pose())
                        {
                            app.input_monitor
                                .saved
                                .config
                                .expressions
                                .insert(entry.file.id.clone());
                        }
                        app.controller
                            .prepare_smoke(&mut app.input_monitor.saved.controller);
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
        let mut app = app;
        // Initial import/setup is not a model-update interval.
        app.frame_clock = crate::performance::FrameClock::new(Instant::now());
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO")
                .is_ok_and(|s| matches!(s.as_str(), "responsiveness" | "performance-details"))
        {
            app.started = Instant::now();
        }
        app
    }

    fn apply_api_action(&mut self, action: crate::effect_api::Action) -> anyhow::Result<()> {
        use crate::effect_api::Action;
        use anyhow::ensure;
        let parameters = self.current_parameters();
        match action {
            Action::SetParameters { values } => {
                ensure!(
                    !values.is_empty() && values.len() <= 64,
                    "Provide 1–64 parameter values"
                );
                for (id, value) in &values {
                    let p = parameters
                        .iter()
                        .find(|p| &p.id == id)
                        .ok_or_else(|| anyhow::anyhow!("Unknown parameter: {id}"))?;
                    ensure!(
                        value.is_finite() && *value >= p.min && *value <= p.max,
                        "{id} must be within {} to {}",
                        p.min,
                        p.max
                    );
                }
                self.input_monitor.saved.config.pose.held.extend(values);
                self.input_monitor.saved.config.pose.mode = PoseMode::Override;
            }
            Action::ReleaseParameters { ids } => {
                ensure!(ids.len() <= 64, "At most 64 IDs");
                for id in &ids {
                    ensure!(
                        parameters.iter().any(|p| &p.id == id),
                        "Unknown parameter: {id}"
                    );
                }
                if ids.is_empty() {
                    self.input_monitor.saved.config.pose.held.clear();
                } else {
                    for id in ids {
                        self.input_monitor.saved.config.pose.held.remove(&id);
                    }
                }
                if self.input_monitor.saved.config.pose.held.is_empty() {
                    self.input_monitor.saved.config.pose.mode = PoseMode::Live;
                }
            }
            Action::Pose { frozen } => {
                if frozen {
                    self.input_monitor.saved.config.capture_pose(&parameters);
                } else {
                    self.input_monitor.saved.config.pose.mode = PoseMode::Live;
                }
            }
            Action::Preset { index } => {
                ensure!(
                    self.input_monitor
                        .apply_preset(index, &mut self.settings.mapping),
                    "Preset missing or invalid"
                );
            }
            Action::Expression { id, enabled } => {
                ensure!(
                    self.input_monitor
                        .expressions
                        .entries
                        .iter()
                        .any(|e| e.file.id == id),
                    "Unknown expression"
                );
                if enabled {
                    self.input_monitor.saved.config.expressions.insert(id);
                } else {
                    self.input_monitor.saved.config.expressions.remove(&id);
                }
            }
            Action::Output {
                index,
                open,
                zoom,
                position,
            } => {
                ensure!(index < 3, "Output index must be 0, 1 or 2");
                ensure!(
                    zoom.is_none_or(|v| v.is_finite() && (0.25..=3.).contains(&v)),
                    "Zoom must be 0.25–3"
                );
                ensure!(
                    position.is_none_or(|v| v
                        .iter()
                        .all(|x| x.is_finite() && (-1.0..=1.0).contains(x))),
                    "Position coordinates must be -1 to 1"
                );
                self.outputs.edit_canvas(index, |c| {
                    if let Some(v) = zoom {
                        c.zoom = v;
                    }
                    if let Some(v) = position {
                        c.position = v;
                    }
                });
                self.outputs.set_open(index, open);
            }
            Action::Theme { name } => {
                let t = theme::presets()
                    .into_iter()
                    .chain(self.settings.theme.custom.iter().cloned())
                    .find(|t| t.name == name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown theme"))?;
                self.settings.theme.active = t;
            }
            Action::SaveProfile => {
                self.input_monitor.save_requested = true;
            }
        }
        Ok(())
    }
    fn connection_status(&self) -> &str {
        if self.settings.source == Source::Demo {
            "Demo input"
        } else if self.settings.source == Source::Local {
            "Local microphone / manual input"
        } else if self.receiver.is_none() && !self.camera.running() {
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

    fn controls_header(&mut self, ui: &mut egui::Ui) {
        section(ui, "WORKSPACE");
        ui.horizontal(|ui| {
            let name = self
                .live2d
                .as_ref()
                .map(|a| a.name.as_str())
                .or_else(|| self.vrm.as_ref().map(|a| a.asset.summary.name.as_str()))
                .or_else(|| self.idle.as_ref().map(|s| s.name.as_str()))
                .unwrap_or("Mica");
            ui.add_sized(
                [(ui.available_width() - 112.0).max(60.0), 26.0],
                egui::Label::new(RichText::new(name).strong()).truncate(),
            )
            .on_hover_text(name);
            if crate::help::control(ui, "profiles", |ui| ui.small_button("Save profile")).clicked()
            {
                self.input_monitor.save_requested = true;
                self.input_monitor.message =
                    Some("This model's complete profile was saved locally.".into());
            }
        });
        theme::caption(
            ui,
            if self.controls_page == ControlsPage::Settings {
                "Appearance and API settings apply to this PC."
            } else {
                "Settings belong to this avatar."
            },
        );
        theme::segments(
            ui,
            &mut self.controls_page,
            &[
                (ControlsPage::Avatar, "Avatar"),
                (ControlsPage::Tracking, "Tracking"),
                (ControlsPage::Output, "Output"),
                (ControlsPage::Chat, "Chat"),
                (ControlsPage::Settings, "Settings"),
            ],
        );
        ui.add_space(4.0);
    }
    fn controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.controls_page == ControlsPage::Settings {
            theme::category(
                ui,
                "appearance-settings",
                "Appearance & themes",
                true,
                |ui| self.settings.theme.ui(ui),
            );
            theme::category(ui, "api-settings", "Developer API", true, |ui| {
                self.effect_api.ui(ui, &mut self.settings.effect_api)
            });
        }
        if self.controls_page == ControlsPage::Tracking {
            theme::category(
                ui,
                "tracking-card-v17",
                "Tracking & connection",
                true,
                |ui| {
                    crate::help::label(ui, "Tracking source", "tracking");
                    if crate::help::control(ui, "tracking-guide", |ui| {
                        ui.button("Guided tracking setup…")
                    })
                    .clicked()
                    {
                        self.input_monitor.setup_tracking_requested = true;
                    }
                    let old = self.settings.source;
                    egui::ComboBox::from_id_salt("source")
                        .selected_text(match old {
                            Source::Webcam => "Webcam · MediaPipe",
                            Source::Rtx => "Webcam · NVIDIA RTX",
                            Source::Demo => "Demo · no device needed",
                            Source::Vts => "iPhone · VTube Studio",
                            Source::Json => "External tool · ARIA JSON",
                            Source::Local => "Microphone / manual · no tracker",
                        })
                        .width(230.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.settings.source,
                                Source::Webcam,
                                "Webcam · MediaPipe",
                            );
                            ui.selectable_value(
                                &mut self.settings.source,
                                Source::Rtx,
                                "Webcam · NVIDIA RTX",
                            );
                            ui.selectable_value(
                                &mut self.settings.source,
                                Source::Local,
                                "Microphone / manual · no tracker",
                            );
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
                        self.camera.stop();
                        self.receiver = None;
                        self.snapshot = Snapshot::default();
                        self.pipeline.reset();
                        self.raw = None;
                        self.status_message = None;
                    }
                    if self.settings.source == Source::Json {
                        crate::help::label(ui, "Packet format & external tools", "json");
                    }
                    if matches!(self.settings.source, Source::Webcam | Source::Rtx) {
                        self.camera.ui(
                            ui,
                            &mut self.settings.camera,
                            &mut self.settings.camera_runtime,
                            self.settings.source == Source::Rtx,
                        );
                    } else if matches!(self.settings.source, Source::Demo | Source::Local) {
                        if self.settings.source == Source::Local
                            && ui.button("Configure microphone").clicked()
                        {
                            self.input_monitor.tab = Tab::Microphone;
                        }
                        ui.label(
                            RichText::new(if self.settings.source == Source::Local {
                                "No tracker connected. Use microphone, hotkeys or manual controls."
                            } else {
                                "Synthetic movement for checking your avatar and output."
                            })
                            .color(muted()),
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
                            if crate::help::control(ui, "tracking", |ui| ui.button("Disconnect"))
                                .clicked()
                            {
                                self.receiver = None;
                                self.raw = None;
                            }
                        } else if crate::help::control(ui, "tracking", |ui| {
                            ui.add_sized(
                                [230.0, 36.0],
                                egui::Button::new(RichText::new("Connect tracking").color(bg()))
                                    .fill(mint()),
                            )
                        })
                        .clicked()
                        {
                            self.connect();
                        }
                        ui.label(RichText::new(if self.settings.source == Source::Vts { "On iPhone: enable 3rd Party PC Clients in VTube Studio. Use the phone's IPv4 address." }
                else { "Accepts ARIA JSON v1 packets from this IP. Use 127.0.0.1 for local tools." }).small().color(muted()));
                    }
                    ui.add_space(7.0);
                    let color = if self.raw.as_ref().is_some_and(|f| f.face_found) {
                        mint()
                    } else {
                        Color32::from_rgb(238, 191, 119)
                    };
                    ui.horizontal(|ui| {
                        let (dot, _) =
                            ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
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
                },
            );
        }
        if self.controls_page == ControlsPage::Tracking {
            theme::category(ui, "speech-response", "Mouth response", true, |ui| {
                let response = &mut self.input_monitor.saved.config.mouth_response;
                let before = *response;
                crate::help::control(ui, "mouth-response", |ui| {
                    ui.checkbox(&mut response.enabled, "Responsive speech")
                });
                ui.add_enabled_ui(response.enabled, |ui| {
                    ui.horizontal(|ui| {
                        for (label, ms) in [("Instant", 0.0), ("Quick", 12.0), ("Soft", 35.0)] {
                            if ui
                                .selectable_label(response.smoothing_ms == ms, label)
                                .on_hover_text(format!(
                                    "{ms:.0} ms mouth smoothing; saved for this avatar."
                                ))
                                .clicked()
                            {
                                response.smoothing_ms = ms;
                            }
                        }
                    });
                    crate::help::control(ui, "mouth-response", |ui| {
                        ui.add(
                            egui::Slider::new(&mut response.smoothing_ms, 0.0..=200.0)
                                .text("Smooth ms"),
                        )
                    });
                });
                theme::caption(
                    ui,
                    "Quick speech, smooth head movement. Saved per avatar and in movement presets.",
                );
                if *response != before {
                    self.speech_filter.reset();
                    self.input_monitor.save_requested = true;
                }
            });
            theme::category(
                ui,
                "movement-card-v17",
                "Movement & calibration",
                false,
                |ui| {
                    if !self.input_monitor.saved.config.tracking.ranges.is_empty() {
                        if crate::help::control(ui, "tracking-guide", |ui| {
                            ui.checkbox(
                                &mut self.input_monitor.saved.config.tracking.enabled,
                                "Use personal tracking calibration",
                            )
                        })
                        .changed()
                        {
                            self.input_monitor.save_requested = true;
                        }
                        theme::caption(
                            ui,
                            "Personal ranges are saved with this avatar and movement presets. Rerun guided setup after changing gains, axes or the camera.",
                        );
                    }
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
                        self.input_monitor.saved.config.tracking.enabled = false;
                        self.input_monitor.save_requested = true;
                        self.input_monitor.message = Some("Neutral pose updated. Personal range calibration was disabled; rerun guided setup to match the new origin.".into());
                    }
                    crate::help::control(ui, "smoothing", |ui| {
                        ui.add(
                            egui::Slider::new(&mut self.settings.mapping.smoothing_ms, 0.0..=300.0)
                                .text("Movement ms"),
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
                            ui.checkbox(
                                &mut self.settings.mapping.invert_roll,
                                "Invert roll (tilt)",
                            )
                        });
                        if crate::help::control(ui, "calibration", |ui| {
                            ui.button("Reset mapping and calibration")
                        })
                        .clicked()
                        {
                            self.settings.mapping = MappingSettings::default();
                            self.pipeline.reset();
                            self.input_monitor.saved.config.tracking = Default::default();
                            self.input_monitor.save_requested = true;
                        }
                    });
                },
            );
        }
        if self.controls_page == ControlsPage::Avatar {
            theme::category(ui, "avatar-card-v18", "Avatar & appearance", true, |ui| {
                let kind = self.avatar_kind();
                match kind {
                    None => {
                        ui.heading("Bring your avatar to life");
                        theme::caption(
                            ui,
                            "Choose a type to start a guided import. The built-in puppet stays on stage until your avatar is ready.",
                        );
                        if ui.button("Import PNG / GIF avatar…").clicked() {
                            self.pending_image = None;
                            self.pending_vrm = None;
                            self.importer
                                .start(Some(crate::avatar_import::Kind::Images));
                        }
                        if ui.button("Import VRM avatar…").clicked() {
                            self.pending_image = None;
                            self.pending_vrm = None;
                            self.importer.start(Some(crate::avatar_import::Kind::Vrm));
                        }
                        if ui.button("Import Live2D avatar…").clicked() {
                            self.pending_image = None;
                            self.pending_vrm = None;
                            self.importer
                                .start(Some(crate::avatar_import::Kind::Live2d));
                        }
                    }
                    Some(crate::avatar_import::Kind::Images) => {
                        ui.strong("PNG / GIF avatar");
                        if let Some(sprite) = &self.idle {
                            ui.label(&sprite.name);
                            ui.small(format!(
                                "{} × {} source · {} × {} base playback",
                                sprite.size.x as u32,
                                sprite.size.y as u32,
                                sprite.texture.size()[0],
                                sprite.texture.size()[1]
                            ));
                        }
                        if ui.button("Artwork, actions & transitions").clicked() {
                            self.input_monitor.tab = Tab::Images;
                        }
                        if ui.button("Microphone & talking").clicked() {
                            self.input_monitor.tab = Tab::Microphone;
                        }
                        if ui.button("Choose talking image…").clicked() {
                            self.load_image(ctx, true);
                        }
                        let previous = self.input_monitor.saved.config.images.playback_mib;
                        help_image_memory(
                            ui,
                            &mut self.input_monitor.saved.config.images.playback_mib,
                        );
                        self.input_monitor.save_requested |=
                            previous != self.input_monitor.saved.config.images.playback_mib;
                    }
                    Some(crate::avatar_import::Kind::Vrm) => {
                        ui.strong("VRM 3D avatar");
                        if let Some(avatar) = &self.vrm {
                            ui.label(&avatar.asset.summary.name);
                            ui.small(format!(
                                "VRM {} · {} bones · {} expressions",
                                avatar.asset.summary.version,
                                avatar.asset.bones.len(),
                                avatar.asset.expressions.len()
                            ));
                        }
                        for (label, tab) in [
                            ("View & framing", Tab::Vrm),
                            ("Tracking & parameters", Tab::Inputs),
                            ("Spring physics", Tab::Physics),
                            ("Expressions & hotkeys", Tab::Expressions),
                        ] {
                            if ui.button(label).clicked() {
                                self.input_monitor.tab = tab;
                            }
                        }
                        if let Some(avatar) = &self.vrm {
                            crate::vrm::panel::details(ui, avatar);
                        }
                    }
                    Some(crate::avatar_import::Kind::Live2d) => {
                        ui.strong("Live2D avatar");
                        if let Some(avatar) = &self.live2d {
                            crate::help::label(
                                ui,
                                "Automatic full-avatar framing",
                                "live2d-framing",
                            );
                            ui.label(&avatar.name);
                            ui.small(format!(
                                "{} parameters · {} physics groups · {} expressions",
                                avatar.model.parameters().len(),
                                self.input_monitor.physics_groups.len(),
                                self.input_monitor.expressions.entries.len()
                            ));
                        }
                        if ui.button("Tracking & parameters").clicked() {
                            self.input_monitor.tab = Tab::Inputs;
                        }
                        if ui.button("Avatar physics").clicked() {
                            self.input_monitor.tab = Tab::Physics;
                        }
                        if ui.button("Expressions & hotkeys").clicked() {
                            self.input_monitor.tab = Tab::Expressions;
                        }
                        if let Some(avatar) = &self.live2d {
                            ui.collapsing("Model details", |ui| {
                                crate::help::button(ui, "avatar");
                                ui.small(format!(
                                    "{} meshes · {:.0} MiB atlases · Core {} · {} imported assignments",
                                    avatar.model.drawables.len(),
                                    avatar.atlas_mib(),
                                    avatar.model.version, avatar.imported_count
                                ));
                                for note in &avatar.files.warnings {
                                    ui.small(note);
                                }
                                if avatar.files.source.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
                                    && ui.button("Inspect exported files…").clicked()
                                {
                                    match aria_model::inspect(&avatar.files.source) {
                                        Ok(report) => {self.model=Some(report);self.model_open=true;}
                                        Err(e) => self.status_message=Some(format!("{e:#}")),
                                    }
                                }
                            });
                        }
                        ui.collapsing("Cubism runtime", |ui| {
                            crate::help::label(ui, "Core library path", "runtime");
                            ui.small(aria_live2d::platform::guidance());
                            ui.small("Runs in a separate Cubism runtime process.");
                            ui.hyperlink_to(
                                aria_live2d::platform::download_label(),
                                aria_live2d::platform::DOWNLOAD_URL,
                            );
                            ui.small(aria_live2d::platform::download_hint());
                            ui.text_edit_singleline(&mut self.settings.cubism_core);
                            if ui.button("Choose Core library…").clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Cubism Core", aria_live2d::platform::extensions())
                                    .pick_file()
                            {
                                self.settings.cubism_core = path.display().to_string();
                            }
                        });
                    }
                }
                if kind.is_some() {
                    crate::help::control(ui, "avatar", |ui| {
                        ui.add(
                            egui::Slider::new(&mut self.settings.zoom, 0.5..=1.5)
                                .text("Stage zoom"),
                        )
                    });
                    ui.separator();
                    if ui.button("Change avatar / type…").clicked() {
                        self.pending_image = None;
                        self.pending_vrm = None;
                        self.importer.start(None);
                    }
                    if ui.button("Guided import tour…").clicked() {
                        self.pending_image = None;
                        self.pending_vrm = None;
                        self.importer.start(kind);
                    }
                    if ui.small_button("Use built-in puppet").clicked() {
                        self.pending_image = None;
                        self.pending_vrm = None;
                        self.importer.open = false;
                        self.idle = None;
                        self.talking = None;
                        self.settings.image_avatar = None;
                        self.use_preview_rig();
                    }
                }
                crate::help::label(ui, "Import guide & size limits", "guided-import");
                if let Some(pending) = &self.pending_vrm {
                    ui.spinner();
                    ui.label(pending.progress());
                    if ui.button("Cancel VRM import").clicked() {
                        self.pending_vrm = None;
                    }
                }
                if let Some(pending) = &self.pending_image {
                    let (done, total) = pending.job.progress();
                    ui.label(format!("Loading artwork · {done}/{total} frames"));
                    if ui.button("Cancel image import").clicked() {
                        self.pending_image = None;
                        self.pending_vrm = None;
                    }
                }
            });
        }
        if self.controls_page == ControlsPage::Chat {
            theme::category(ui, "chat-card-v17", "Streaming chat", true, |ui| {
                self.chats
                    .ui(ui, &mut self.settings.chat_accounts, &self.outputs);
            });
        }
        if self.controls_page == ControlsPage::Output {
            theme::category(ui, "output-card-v17", "Capture & performance", true, |ui| {
                if let Some(index) = self.outputs.ui(ui) {
                    self.detect_key_color(index);
                }
                let selected = self.outputs.snapshot().selected;
                if let Some(status) = &self.broadcasts.status[selected] {
                    ui.label(RichText::new(status).small().color(mint()));
                }
                ui.collapsing("OBS connection & troubleshooting", |ui| {
                    if crate::help::control(ui, "spout", |ui| ui.small_button("Retry OBS output"))
                        .clicked()
                    {
                        self.broadcasts.retry();
                    }
                    theme::caption(ui, crate::native_output::GUIDE);
                });
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
                if let Some(status) = &self.priority_status { ui.label(RichText::new(status).small().color(mint())); }
                theme::caption(ui, "The FPS target limits model simulation and rendering even while dragging UI controls. Static poses reuse the last model texture. Closed outputs release their capture textures.");
            });
                ui.hyperlink_to(
                    "Windows setup & troubleshooting ↗",
                    "https://github.com/NekoUnix/A.R.I.A/blob/main/docs/windows.md",
                );
            });
        }
    }

    fn detect_key_color(&mut self, index: usize) {
        let result = (|| -> anyhow::Result<crate::chroma::Suggestion> {
            let mut palette = if let Some(avatar) = &self.live2d {
                avatar.key_palette()?
            } else if let Some(avatar) = &self.vrm {
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
            self.images.merge_palette(&mut palette);
            palette.suggest()
        })();
        self.outputs.apply_suggestion(index, result);
    }

    fn load_image(&mut self, ctx: &egui::Context, talking: bool) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Avatar image", &["png", "gif", "jpg", "jpeg"])
            .pick_file()
        {
            self.open_image(ctx, &path, talking);
        }
    }
    fn open_image(&mut self, ctx: &egui::Context, path: &Path, talking: bool) {
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() != Ok("odette-gifs")
        {
            match crate::media::load(ctx, self.render_state.as_ref(), path, 0, true) {
                Ok(sprite) => self.apply_image(
                    path,
                    talking,
                    sprite,
                    aria_core::asset_limits::GIF_PLAYBACK_MIB,
                ),
                Err(e) => self.status_message = Some(format!("{e:#}")),
            }
        } else {
            self.begin_image(
                ctx,
                path,
                talking,
                Vec::new(),
                self.input_monitor.saved.config.images.playback_mib,
            );
        }
    }
    fn begin_image(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        talking: bool,
        artwork: Vec<crate::avatar_import::Artwork>,
        budget: u32,
    ) {
        self.pending_image = None;
        self.pending_vrm = None;
        match crate::media::LoadJob::start(ctx, self.render_state.as_ref(), path, 0, budget) {
            Ok(job) => {
                self.pending_image = Some(PendingImage {
                    path: path.to_owned(),
                    talking,
                    artwork,
                    budget,
                    job,
                })
            }
            Err(e) => {
                self.status_message = Some(e.to_string());
                self.importer.error = Some(e.to_string());
            }
        }
    }
    fn apply_image(&mut self, path: &Path, talking: bool, sprite: Sprite, budget: u32) {
        if !talking || self.live2d.is_some() || self.vrm.is_some() {
            self.use_puppet_rig(&sprite.model_key);
        }
        if talking {
            self.images.add(
                &mut self.input_monitor.saved.config.images,
                path.to_owned(),
                aria_core::image_actions::Trigger::Talking,
            );
            self.talking = Some(sprite.clone());
        } else {
            if self.input_monitor.saved.config.images.states.is_empty() {
                self.images.add(
                    &mut self.input_monitor.saved.config.images,
                    path.to_owned(),
                    aria_core::image_actions::Trigger::Idle,
                );
            }
            self.settings.image_avatar = Some(path.to_owned());
            self.idle = Some(sprite.clone());
            self.talking = None;
        }
        self.images.seed(path.to_owned(), sprite, budget);
        self.input_monitor.tab = Tab::Images;
        self.input_monitor.save_requested = true;
        self.status_message = None;
    }
    fn poll_image_import(&mut self) {
        let result = self.pending_image.as_ref().and_then(|p| p.job.poll());
        if let Some(result) = result {
            let pending = self.pending_image.take().unwrap();
            match result {
                Ok(sprite) => {
                    self.apply_image(&pending.path, pending.talking, sprite, pending.budget);
                    // Only the guided importer explicitly changes this setting.
                    // Reopening artwork must preserve its restored avatar profile.
                    if !pending.artwork.is_empty() {
                        self.input_monitor.saved.config.images.playback_mib = pending.budget;
                    }
                    for art in pending.artwork {
                        if !self
                            .input_monitor
                            .saved
                            .config
                            .images
                            .states
                            .iter()
                            .any(|s| s.path == art.path)
                        {
                            self.images.add(
                                &mut self.input_monitor.saved.config.images,
                                art.path,
                                art.trigger,
                            );
                        }
                    }
                    self.importer.finished();
                }
                Err(error) => {
                    self.status_message = Some(error.clone());
                    self.importer.error = Some(error);
                }
            }
        }
    }
    fn avatar_kind(&self) -> Option<crate::avatar_import::Kind> {
        if self.live2d.is_some() {
            Some(crate::avatar_import::Kind::Live2d)
        } else if self.vrm.is_some() {
            Some(crate::avatar_import::Kind::Vrm)
        } else if self.idle.is_some() {
            Some(crate::avatar_import::Kind::Images)
        } else {
            None
        }
    }
    fn import_tour(&mut self, ctx: &egui::Context) {
        let progress = self.pending_image.as_ref().map(|p| p.job.progress());
        match self.importer.show(
            ctx,
            &mut self.settings.cubism_core,
            progress,
            self.pending_vrm.as_ref().map(|p| p.progress()),
        ) {
            Some(crate::avatar_import::Request::Images { artwork, budget }) => {
                if let Some(base) = artwork
                    .iter()
                    .find(|a| a.trigger == aria_core::image_actions::Trigger::Idle)
                {
                    let path = base.path.clone();
                    self.begin_image(ctx, &path, false, artwork, budget);
                }
            }
            Some(crate::avatar_import::Request::Vrm(path)) => self.begin_vrm(&path),
            Some(crate::avatar_import::Request::Live2d(path)) => {
                self.pending_image = None;
                self.pending_vrm = None;
                self.open_model(&path);
                if let Some(error) = &self.status_message {
                    self.importer.error = Some(error.clone());
                    if self.bare_moc.is_some() {
                        self.importer.open = false;
                    }
                } else {
                    self.importer.finished();
                }
            }
            Some(crate::avatar_import::Request::Cancel) => {
                self.pending_image = None;
                self.pending_vrm = None;
                self.importer.open = false;
            }
            Some(crate::avatar_import::Request::Microphone) => {
                self.input_monitor.tab = Tab::Microphone;
                self.importer.open = false;
            }
            Some(crate::avatar_import::Request::Tracking) => {
                self.importer.open = false;
                self.start_tracking_guide();
            }
            Some(crate::avatar_import::Request::Controls) => {
                self.input_monitor.tab = if self.live2d.is_some() {
                    Tab::Physics
                } else if self.vrm.is_some() {
                    Tab::Vrm
                } else {
                    Tab::Images
                };
                self.importer.open = false;
            }
            None => {}
        }
    }

    fn active_sprite(&self) -> Option<&Sprite> {
        if let Some(sprite) = self.images.primary() {
            return Some(sprite);
        }
        if self.params.0[5] > 0.18 {
            self.talking.as_ref().or(self.idle.as_ref())
        } else {
            self.idle.as_ref()
        }
    }

    fn import_model(&mut self, files: aria_model::ModelFiles) -> anyhow::Result<()> {
        self.pending_image = None;
        self.pending_vrm = None;
        anyhow::ensure!(
            !self.settings.cubism_core.trim().is_empty(),
            "First choose the official native Cubism Core library under Cubism runtime setup."
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
        self.vrm = None;
        self.settings.vrm_avatar = None;
        self.settings.image_avatar = None;
        self.idle = None;
        self.talking = None;
        self.status_message = None;
        Ok(())
    }
    fn begin_vrm(&mut self, path: &Path) {
        self.pending_image = None;
        self.pending_vrm = Some(crate::vrm::LoadJob::start(path.to_owned()));
        self.status_message = None;
        self.importer.error = None;
    }
    fn poll_vrm_import(&mut self) {
        let Some(result) = self.pending_vrm.as_ref().and_then(|p| p.poll()) else {
            return;
        };
        self.pending_vrm = None;
        let result = result
            .and_then(|asset| {
                crate::vrm::Avatar::from_asset(
                    self.render_state
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("GPU unavailable"))?,
                    asset,
                )
            })
            .and_then(|avatar| {
                let expressions = avatar.embedded_expressions()?;
                Ok((avatar, expressions))
            });
        match result {
            Ok((avatar, expressions)) => {
                self.remember_current_rig();
                self.input_monitor = InputMonitor::new(
                    avatar.asset.key.clone(),
                    avatar.initial_config.clone(),
                    self.settings.saved_rigs.get(&avatar.asset.key).cloned(),
                    avatar.parameters(),
                );
                self.input_monitor.expressions.load_embedded(
                    expressions,
                    &self.input_monitor.saved,
                    avatar.parameters(),
                );
                self.input_monitor.tab = Tab::Vrm;
                self.restore_model_preferences(&avatar.asset.key);
                self.hotkeys.configure(Vec::new());
                self.animation_time = 0.;
                self.settings.vrm_avatar = Some(avatar.asset.path.clone());
                self.settings.image_avatar = None;
                self.live2d = None;
                self.idle = None;
                self.talking = None;
                self.vrm = Some(avatar);
                self.input_monitor.save_requested = true;
                self.status_message = None;
                self.importer.finished();
                #[cfg(feature = "screenshots")]
                if crate::smoke_mode() {
                    eprintln!("VRM primary avatar imported successfully");
                    let scenario = std::env::var("ARIA_SMOKE_SCENARIO").unwrap_or_default();
                    self.settings.source = Source::Local;
                    if scenario == "vrm-motion" {
                        self.input_monitor.saved.config.vrm.motion.gesture_loop = true;
                        self.vrm
                            .as_mut()
                            .unwrap()
                            .motion
                            .play(crate::vrm::motion::Gesture::Wave);
                    }
                    if scenario == "vrm-springs" {
                        self.input_monitor.tab = Tab::Physics;
                    }
                    if scenario == "vrm-expressions" {
                        self.input_monitor.tab = Tab::Expressions;
                    }
                    if scenario == "vrm-portrait" {
                        self.input_monitor.saved.config.vrm.portrait = 1.;
                    }
                    if scenario == "output-vrm" {
                        for index in 0..3 {
                            self.outputs.set_open(index, true);
                            self.outputs
                                .edit_canvas(index, |c| c.background = Background::Green);
                        }
                    }
                }
            }
            Err(error) => {
                let error = format!("{error:#}");
                self.status_message = Some(error.clone());
                self.importer.error = Some(error);
            }
        }
    }
    fn open_model(&mut self, path: &Path) {
        if path.is_dir() {
            self.importer.folder(path.to_owned());
            return;
        }
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("vrm"))
        {
            self.begin_vrm(path);
            return;
        }
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
            || {
                self.vrm.as_ref().map_or_else(
                    || movement::preview_parameters(self.params),
                    |a| a.parameters().to_vec(),
                )
            },
            |a| a.model.parameters().to_vec(),
        )
    }
    fn tracking_context(&self) -> String {
        // A draft must never follow another model, changed assignment or tracker.
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.input_monitor.model_key,
            serde_json::to_string(&self.settings.source).unwrap_or_default(),
            self.settings.sender_ip,
            self.settings.request_port,
            self.settings.listen_port,
            serde_json::to_string(&(
                &self.settings.camera,
                &self.settings.mapping,
                self.pipeline.calibration(),
                &self.input_monitor.saved.config.bindings,
                &self.input_monitor.saved.config.tracking
            ))
            .unwrap_or_default()
        )
    }
    fn start_tracking_guide(&mut self) {
        self.controls_page = ControlsPage::Tracking;
        self.tracking_guide.start(
            self.tracking_context(),
            &self.input_monitor.saved.config,
            &self.current_parameters(),
            &self.live_inputs,
            self.pipeline.calibration(),
        );
    }
    fn tracking_guide_window(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.input_monitor.setup_tracking_requested) {
            self.start_tracking_guide();
        }
        let status = self.connection_status().to_owned();
        match self.tracking_guide.show(
            ctx,
            &status,
            self.input_monitor.saved.config.pose.mode == PoseMode::Live,
            self.input_monitor.saved.microphone.enabled,
        ) {
            Some(crate::tracking_guide::Action::Connection) => {
                self.controls_page = ControlsPage::Tracking;
                self.tracking_guide.open = false;
                theme::open_category(ctx, "Tracking & connection");
            }
            Some(crate::tracking_guide::Action::Resume) => {
                self.input_monitor.saved.config.pose.mode = PoseMode::Live;
                self.input_monitor.reset_motion = true;
            }
            Some(crate::tracking_guide::Action::Save(profile)) => {
                self.input_monitor.saved.config.tracking = profile;
                self.tracking_filter.reset();
                self.input_monitor.saved.config.reset_filters();
                self.input_monitor.save_requested = true;
                self.input_monitor.message = Some("Personal tracking calibration saved for this avatar. Save a movement preset to keep a named version or assign it a hotkey.".into());
            }
            None => {}
        }
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
        self.effect_api.invalidate_model();
        self.camera.stop();
        self.tracking_filter.reset();
        self.speech_filter.reset();
        self.items.reset();
        self.effects.reset();
        self.images.reset();
        self.microphone.stop();
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
        self.vrm = None;
        self.settings.vrm_avatar = None;
        self.pending_vrm = None;
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
        let parameters = self.current_parameters();
        let labels = self
            .live2d
            .as_ref()
            .map(|a| a.labels.clone())
            .or_else(|| self.vrm.as_ref().map(|a| a.labels.clone()))
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
            if let Some(avatar) = &mut self.vrm {
                if self.input_monitor.tab == Tab::Vrm {
                    crate::vrm::panel::view(ui, avatar, &mut self.input_monitor);
                }
                if self.input_monitor.tab == Tab::Physics {
                    crate::vrm::panel::physics(ui, &avatar.asset.springs, &mut self.input_monitor);
                }
            }
            if self.input_monitor.tab == Tab::Layers
                && let Some(avatar) = &self.live2d
            {
                self.input_monitor.save_requested |=
                    self.input_monitor
                        .layers_panel
                        .show(ui, avatar, &mut self.input_monitor.saved);
            }
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
            if self.input_monitor.tab == Tab::Images {
                self.input_monitor.save_requested |= self.images.panel(
                    ui,
                    &mut self.input_monitor.saved,
                    &self.live_inputs,
                    &parameters,
                    self.idle.is_some() && self.live2d.is_none(),
                );
            }
            if self.input_monitor.tab == Tab::Microphone {
                let enabled = self.input_monitor.saved.microphone.enabled;
                self.input_monitor.save_requested |= self
                    .microphone
                    .panel(ui, &mut self.input_monitor.saved.microphone);
                if !enabled
                    && self.input_monitor.saved.microphone.enabled
                    && self.settings.source == Source::Demo
                {
                    self.settings.source = Source::Local;
                    self.pipeline.reset();
                    self.raw = None;
                }
            }
            if self.input_monitor.tab == Tab::Controller {
                let actions = self
                    .input_monitor
                    .expressions
                    .controller_poses_ui(ui, &mut self.input_monitor.saved);
                self.input_monitor.save_requested |= actions.save;
                if actions.message.is_some() {
                    self.input_monitor.message = actions.message;
                }
                self.input_monitor.save_requested |= self.controller.panel(
                    ui,
                    &mut self.input_monitor.saved.controller,
                    &self.live_inputs,
                );
            }
            if self.input_monitor.tab == Tab::Effects {
                self.input_monitor.save_requested |= self.effects.panel(
                    ui,
                    &mut self.input_monitor.saved,
                    &mut self.settings.effect_api,
                    self.effect_api.error.as_deref(),
                );
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
                    .color(muted()),
                );
                for (name, value) in &f.blend_shapes {
                    meter(ui, name, *value, 0.0, 1.0);
                }
            } else {
                ui.label("Connect a tracker to see raw values.");
            }
        }

        if self.input_monitor.tab == Tab::Raw {
            theme::category(
                ui,
                "diagnostic-details",
                "Connection diagnostics & export",
                false,
                |ui| {
                    if crate::help::control(ui, "diagnostics", |ui| {
                        ui.button("Export mapped values…")
                    })
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
                            Ok(()) => {
                                self.status_message = Some("Parameter snapshot exported.".into())
                            }
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
    }

    fn model_window(&mut self, ctx: &egui::Context) {
        if let Some(report) = &self.model {
            egui::Window::new("Live2D asset inspection").open(&mut self.model_open).default_width(670.0).show(ctx, |ui| {
                crate::help::button(ui, "inspection");
 ui.label(report.manifest.display().to_string());
                ui.colored_label(mint(), format!("{} textures · {} expressions · {} motions · {:.2} MiB on disk", report.texture_count,
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
    fn ui(&mut self, root_ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = &root_ui.ctx().clone();
        theme::sync(ctx, self.settings.theme.active.colors);
        self.poll_image_import();
        self.poll_vrm_import();
        self.input_monitor.save_requested |=
            self.chats.update(&mut self.settings.chat_accounts, ctx);
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").is_ok_and(|s| s.starts_with("chat"))
        {
            let key = egui::Id::new("chat-smoke-init");
            if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                self.chats.smoke(&self.outputs);
                ctx.data_mut(|d| d.insert_temp(key, true));
            }
            self.chats.verify_smoke_docking(ctx);
        }
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && ctx.current_pass_index() == 0
            && self.pending_vrm.is_none()
            && self.pending_image.is_none()
            && let Some(path) = std::env::var_os("ARIA_SMOKE_ITEM")
        {
            let key = egui::Id::new("smoke-drop-png");
            if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                let identity = self.live2d.as_ref().map(|a| a.model_key.clone());
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("smoke-main-identity"), identity));
                ctx.input_mut(|i| {
                    i.raw.dropped_files.push(std::sync::Arc::new(
                        crate::screenshot::SmokeDroppedFile(path.into()),
                    ))
                });
                ctx.data_mut(|d| d.insert_temp(key, true));
            }
        }
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_owned())
                .collect::<Vec<_>>()
        });
        let mut dropped_items: Vec<_> =
            if ctx.current_pass_index() == 0 && self.effects.editor.is_none() {
                dropped
            } else {
                Vec::new()
            };
        if let Some(path) = dropped_items.iter().find(|p| p.is_dir()).cloned() {
            self.pending_vrm = None;
            self.pending_image = None;
            self.importer.folder(path);
            dropped_items.retain(|p| !p.is_dir());
        }
        if let Some(path) = dropped_items
            .iter()
            .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vrm")))
            .cloned()
        {
            self.pending_vrm = None;
            self.pending_image = None;
            self.importer.start(Some(crate::avatar_import::Kind::Vrm));
            self.importer.model = Some(path);
            dropped_items.retain(|p| !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vrm")));
        }
        let now = Instant::now();
        // A layout discard can call update twice for the same frame.
        let dt = self
            .frame_clock
            .tick(now, self.settings.fps, ctx.current_pass_index() == 0);
        if dt > 0.0 {
            let seconds = self.frame_clock.last_interval().as_secs_f64();
            self.metrics.record_frame(seconds, self.settings.fps);
            let measured = (1.0 / seconds as f32).min(1000.0);
            self.render_fps = if self.render_fps == 0.0 {
                measured
            } else {
                self.render_fps * 0.92 + measured * 0.08
            };
        }
        if dt > 0.0 {
            if self.settings.source == Source::Demo {
                self.raw = Some(demo_frame(self.started.elapsed().as_secs_f32()));
            } else if matches!(self.settings.source, Source::Webcam | Source::Rtx) {
                self.snapshot = self.camera.snapshot();
                self.raw = self.snapshot.fresh_frame().cloned();
            } else if let Some(receiver) = &self.receiver {
                self.snapshot = receiver.snapshot();
                self.raw = self.snapshot.fresh_frame().cloned();
            } else {
                self.raw = None;
            }
        }
        if self.settings.effect_api.enabled
            && self.api_snapshot_at.elapsed() >= Duration::from_millis(100)
        {
            self.api_snapshot_at = Instant::now();
            let parameters:Vec<_>=self.current_parameters().iter().map(|p|serde_json::json!({"id":p.id,"min":p.min,"max":p.max,"default":p.default,"value":p.value})).collect();
            self.effect_api.publish(serde_json::json!({
                "tracking_status":self.connection_status(),"source":self.settings.source,
                "parameters":parameters,"inputs":self.live_inputs,"pose":self.input_monitor.saved.config.pose.mode,
                "presets":self.input_monitor.saved.presets.iter().enumerate().map(|(index,p)|serde_json::json!({"index":index,"name":p.name})).collect::<Vec<_>>(),
                "expressions":self.input_monitor.expressions.entries.iter().map(|e|serde_json::json!({"id":e.file.id,"name":e.file.name,"active":self.input_monitor.saved.config.expressions.contains(&e.file.id)})).collect::<Vec<_>>(),
                "outputs":(0..3).map(|i|serde_json::json!({"index":i,"name":crate::output::NAMES[i],"open":self.outputs.is_open(i),"settings":self.outputs.snapshot().canvas(i)})).collect::<Vec<_>>(),
                "themes":theme::presets().into_iter().chain(self.settings.theme.custom.iter().cloned()).map(|t|t.name).collect::<Vec<_>>(),
                "theme":self.settings.theme.active.name,
                "usage":self.metrics.usage
            }));
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
        self.effects
            .pending
            .extend(std::mem::take(&mut self.input_monitor.effect_requests));
        if !crate::smoke_mode() || self.settings.effect_api.enabled {
            for command in self.effect_api.update(
                &self.settings.effect_api,
                &self.input_monitor.model_key,
                &self.input_monitor.saved.effects,
                ctx,
            ) {
                let result = if command.profile != self.input_monitor.model_key
                    || !self.effect_api.is_current(&command)
                {
                    Err("Model changed before the command was applied".into())
                } else if let Some(action) = command.action {
                    self.apply_api_action(action).map_err(|e| e.to_string())
                } else if self
                    .input_monitor
                    .saved
                    .effects
                    .designs
                    .iter()
                    .any(|d| d.id == command.id)
                {
                    self.effects.pending.push(command.id);
                    Ok(())
                } else {
                    Err("Effect no longer exists".into())
                };
                self.effect_api.complete(command.ticket, result);
            }
        }
        if dt > 0.0 {
            let mouth_response = self.input_monitor.saved.config.mouth_response;
            self.params = self.pipeline.update_with_response(
                self.raw.as_ref(),
                &self.settings.mapping,
                dt,
                mouth_response.enabled,
            );
            if self.input_monitor.saved.config.pose.mode != PoseMode::Frozen {
                self.animation_time += dt.min(0.25);
            }
            self.live_inputs = rig::tracking_inputs(
                self.raw.as_ref(),
                self.params,
                self.settings.mapping.mirror,
                self.animation_time,
            );
            if self.tracking_guide.open {
                self.tracking_guide.check_context(&self.tracking_context());
                if let Some(message) = self.tracking_guide.message.take() {
                    self.input_monitor.message = Some(message);
                }
            }
            if self.tracking_guide.open || self.input_monitor.saved.config.tracking.enabled {
                let measured = if self.tracking_guide.open {
                    self.tracking_guide.measure(
                        self.raw.as_ref(),
                        &self.settings.mapping,
                        self.animation_time,
                    )
                } else {
                    self.input_monitor.saved.config.tracking.measure(
                        self.raw.as_ref(),
                        &self.settings.mapping,
                        self.animation_time,
                    )
                };
                let face_found = self
                    .raw
                    .as_ref()
                    .is_some_and(|f| f.face_found && f.is_finite());
                self.tracking_guide.observe(
                    self.snapshot.packets,
                    self.started.elapsed().as_secs_f64(),
                    face_found
                        && (self.receiver.is_some() || self.camera.running())
                        && matches!(
                            self.settings.source,
                            Source::Vts | Source::Json | Source::Webcam | Source::Rtx
                        ),
                    &measured,
                );
                let preview = self.tracking_guide.profile();
                let profile = preview
                    .as_ref()
                    .unwrap_or(&self.input_monitor.saved.config.tracking);
                self.tracking_filter.apply(
                    profile,
                    &measured,
                    &mut self.live_inputs,
                    face_found,
                    aria_core::calibration::Smoothing {
                        milliseconds: self.settings.mapping.smoothing_ms,
                        responsive_mouth: mouth_response.enabled,
                    },
                    dt,
                );
            } else {
                self.tracking_filter.reset();
            }
            self.speech_filter
                .apply(mouth_response, &mut self.live_inputs, dt);
            self.microphone
                .update(&self.input_monitor.saved.microphone, dt);
            self.microphone
                .inject(&self.input_monitor.saved.microphone, &mut self.live_inputs);
            self.controller.update(
                &self.input_monitor.saved.controller,
                &self.input_monitor.model_key,
                dt,
                &mut self.live_inputs,
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
            } else if let Some(avatar) = &mut self.vrm {
                if std::mem::take(&mut self.input_monitor.reset_motion)
                    && self.input_monitor.saved.config.pose.mode != PoseMode::Frozen
                {
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
                        self.status_message = Some(format!("VRM update stopped: {error:#}"))
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
            if self.live2d.is_none() && self.vrm.is_none() {
                self.images.update(
                    ctx,
                    self.render_state.as_ref(),
                    &self.input_monitor.saved.config.images,
                    &self.live_inputs,
                    &parameters,
                    (dt, self.input_monitor.saved.config.pose.mode),
                );
                // Release an old fallback after changing the playback budget.
                if let Some(path) = &self.settings.image_avatar
                    && let Some(sprite) = self.images.artwork(path)
                    && self
                        .idle
                        .as_ref()
                        .is_none_or(|s| s.texture.id() != sprite.texture.id())
                {
                    self.idle = Some(sprite.clone());
                }
            }
            if self.input_monitor.saved.config.pose.mode != PoseMode::Frozen {
                self.items.clock += dt.min(0.25);
            }
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
        self.items.models.sync(
            self.render_state.as_ref(),
            Path::new(self.settings.cubism_core.trim()),
            &self.input_monitor.saved.config,
        );

        self.metrics.update(
            self.render_state.as_ref(),
            self.live2d
                .iter()
                .map(|a| a.model.process_id())
                .chain(self.items.models.process_ids())
                .chain(self.effects.process_ids())
                .chain(self.camera.process_ids()),
        );
        egui::Panel::top("header")
            .frame(Frame::new().fill(bg()).inner_margin(10.0))
            .show(root_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("A.R.I.A.")
                            .size(21.0)
                            .strong()
                            .color(theme::text_color()),
                    );
                    ui.label(RichText::new("AVATAR STUDIO").size(12.0).color(muted()));
                    crate::help::button(ui, "welcome");
                    if ui.small_button("Help & documentation").clicked() {
                        crate::help::open(ctx, "welcome", String::new());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "v{} · {}",
                                env!("CARGO_PKG_VERSION"),
                                if cfg!(windows) {
                                    "WINDOWS"
                                } else if cfg!(target_os = "macos") {
                                    "MACOS"
                                } else {
                                    "LINUX"
                                }
                            ))
                            .small()
                            .color(muted()),
                        );
                    });
                });
            });
        egui::Panel::bottom("status")
            .frame(Frame::new().fill(bg()).inner_margin(10.0))
            .show(root_ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.metrics.footer(
                        ui,
                        &self.snapshot,
                        self.render_fps,
                        frame.info().cpu_usage,
                        &self.gpu,
                    );
                });
            });
        egui::Panel::left("controls")
            .exact_size(302.0)
            .resizable(false)
            .frame(Frame::new().fill(panel()).inner_margin(12.0))
            .show(root_ui, |ui| {
                self.controls_header(ui);
                let area = egui::ScrollArea::vertical()
                    .id_salt(("controls-scroll-v17", self.controls_page));
                area.show(ui, |ui| self.controls(ui, ctx));
            });
        egui::Panel::right("diagnostics")
            .default_size(380.0)
            .size_range(330.0..=700.0)
            .resizable(true)
            .frame(Frame::new().fill(panel()).inner_margin(12.0))
            .show(root_ui, |ui| {
                section(ui, "INSPECTOR");
                let kind = self.avatar_kind();
                self.input_monitor.navigation(ui, kind);
                let area = egui::ScrollArea::vertical()
                    .id_salt(("inspector-scroll-v17", self.input_monitor.tab));
                #[cfg(feature = "screenshots")]
                let area = if crate::smoke_mode()
                    && std::env::var_os("ARIA_SMOKE_IMAGE_CONTROLS").is_some()
                {
                    area.vertical_scroll_offset(650.0)
                } else {
                    area
                };
                area.show(ui, |ui| self.diagnostics(ui));
            });
        let output_settings = self.outputs.snapshot();
        let stage_output = output_settings.canvas(output_settings.selected);
        egui::CentralPanel::default()
            .frame(Frame::new().fill(bg()).inner_margin(14.0))
            .show(root_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Your stage");
                    crate::help::button(ui, "stage");
                    ui.label(
                        RichText::new(if self.live2d.is_some() {
                            "LIVE2D / CUBISM"
                        } else if self.vrm.is_some() {
                            "VRM / 3D HUMANOID"
                        } else if self.idle.is_some() {
                            "PNG / GIF PUPPET"
                        } else {
                            "MICA / TEST PUPPET"
                        })
                        .small()
                        .color(muted()),
                    );
                });
                ui.label(RichText::new(self.connection_status()).color(mint()));
                ui.horizontal_wrapped(|ui| {
                    crate::help::button(ui, "png-items");
                    if ui
                        .small_button("Drop PNG / GIF / moc3 here · objects")
                        .clicked()
                    {
                        self.input_monitor.tab = Tab::Items;
                    }
                });
                let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                let old_revision = self.items.revision;
                if !dropped_items.is_empty() {
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
                            std::mem::take(&mut dropped_items),
                            &mut self.input_monitor.saved.config,
                            position,
                        ) {
                            self.items.edited();
                        }
                        self.input_monitor.tab = Tab::Items;
                    } else {
                        self.items.message = Some(
                            "Drop objects inside Your stage, or use Add objects in the sidebar."
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
                self.items.models.sync(
                    self.render_state.as_ref(),
                    Path::new(self.settings.cubism_core.trim()),
                    &self.input_monitor.saved.config,
                );
                if ctx.current_pass_index() == 0 {
                    self.items.models.update(
                        &mut self.input_monitor.saved.config,
                        &self.live_inputs,
                        dt,
                    );
                }
                self.items.refresh(
                    &self.input_monitor.saved.config,
                    (self.live2d.as_ref(), self.vrm.as_ref()),
                );
                let scene = self.scene();
                let previous_selection = self.items.selected;
                #[cfg(feature = "screenshots")]
                if crate::smoke_mode()
                    && std::env::var_os("ARIA_SMOKE_ITEM").is_some()
                    && !self.input_monitor.saved.config.items.is_empty()
                {
                    let key = egui::Id::new("smoke-pinned-png");
                    if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                        self.items.prepare_smoke(
                            &mut self.input_monitor.saved.config,
                            (self.live2d.as_ref(), self.vrm.as_ref()),
                        );
                        ctx.data_mut(|d| d.insert_temp(key, true));
                    }
                }
                if self.items.stage(
                    ui,
                    rect,
                    &scene,
                    &mut self.input_monitor.saved.config,
                    (self.live2d.as_ref(), self.vrm.as_ref()),
                    self.settings.zoom,
                ) {
                    self.items.edited();
                }
                if self.items.selected.is_some() && self.items.selected != previous_selection {
                    self.input_monitor.tab = Tab::Items;
                }
                self.items.refresh(
                    &self.input_monitor.saved.config,
                    (self.live2d.as_ref(), self.vrm.as_ref()),
                );
                if self.items.revision != old_revision {
                    self.scene_revision = self.scene_revision.wrapping_add(1);
                }
                if ctx.current_pass_index() == 0
                    && self.effects.update(
                        ctx,
                        self.render_state.as_ref(),
                        Path::new(self.settings.cubism_core.trim()),
                        (
                            &self.input_monitor.saved.effects,
                            self.input_monitor.saved.config.pose.mode,
                        ),
                        self.live2d.as_ref(),
                        dt,
                    )
                {
                    self.scene_revision = self.scene_revision.wrapping_add(1);
                }
                let scene = self.scene();
                let painter = ui.painter_at(rect);
                painter.rect_filled(
                    rect,
                    12.0,
                    if stage_output.background == Background::Transparent {
                        theme::card_color()
                    } else {
                        stage_output.background.color(stage_output.key)
                    },
                );
                if stage_output.background == Background::Studio {
                    for i in 1..12 {
                        let x = rect.left() + rect.width() * i as f32 / 12.0;
                        painter.line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            Stroke::new(1.0_f32, theme::border().gamma_multiply(0.35)),
                        );
                    }
                    painter.circle_stroke(
                        rect.center(),
                        rect.width().min(rect.height()) * 0.38,
                        Stroke::new(1.0_f32, theme::border().gamma_multiply(0.6)),
                    );
                }
                scene.paint_subject(&painter, rect, self.settings.zoom);
                self.items
                    .selection(&painter, &scene, rect, self.settings.zoom);
                painter.text(
                    rect.left_bottom() + egui::vec2(16.0, -18.0),
                    egui::Align2::LEFT_BOTTOM,
                    if self.settings.source == Source::Demo {
                        "DEMO INPUT  /  Choose webcam, phone or microphone"
                    } else if self.settings.source == Source::Local {
                        "LOCAL INPUT  /  Microphone, image actions and hotkeys"
                    } else {
                        "TRACKING PREVIEW  /  Open the capture window for OBS"
                    },
                    egui::FontId::proportional(11.0),
                    muted(),
                );
            });
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && (std::env::var_os("ARIA_TEST_VRM").is_some()
                || (matches!(
                    std::env::var("ARIA_SMOKE_SCENARIO").as_deref(),
                    Ok("controller" | "tracking-guide-review")
                ) && self.live2d.is_some()))
            && self.started.elapsed() > Duration::from_secs(5)
        {
            assert!(
                self.vrm.is_some() || self.live2d.is_some(),
                "VRM smoke import failed: {:?}",
                self.status_message
            );
            let key = egui::Id::new("vrm-smoke-export");
            if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                if let Some(path) = std::env::var_os("ARIA_SMOKE_AVATAR_PNG") {
                    self.input_monitor.export_png = Some(path.into());
                }
                ctx.data_mut(|d| d.insert_temp(key, true));
            }
        }
        if let Some(path) = self.input_monitor.export_png.take() {
            let scene = self.scene();
            let result = if scene.items.is_empty()
                && scene.effects.is_empty()
                && scene.recoil == [0.0; 2]
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
        self.import_tour(ctx);
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && self.pending_image.is_none()
            && self.pending_vrm.is_none()
            && std::env::var("ARIA_SMOKE_SCENARIO").is_ok_and(|s| s.starts_with("tracking-guide"))
        {
            let key = egui::Id::new("tracking-guide-smoke");
            if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                self.input_monitor.saved.config.pose.mode = PoseMode::Live;
                self.start_tracking_guide();
                if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("tracking-guide-take") {
                    self.tracking_guide.rehearsal_take();
                }
                if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("tracking-guide-review") {
                    self.tracking_guide.rehearsal();
                    self.tracking_guide.preview = true;
                    eprintln!(
                        "Tracking guide rehearsal reached review; synthetic samples only; persistent profile unchanged"
                    );
                }
                ctx.data_mut(|d| d.insert_temp(key, true));
            }
        }
        self.tracking_guide_window(ctx);
        if !self.hotkeys.available() {
            self.input_monitor.hotkey_status =
                Some("Hotkey worker could not start. Preset buttons remain available.".into());
        }
        self.hotkeys.configure(self.input_monitor.hotkey_keys());
        if let Some(mut editor) = self.effects.editor.take() {
            let reply = editor.show(
                ctx,
                &self.scene(),
                self.render_state.as_ref(),
                Path::new(&self.settings.cubism_core),
                (self.live2d.as_ref(), dt),
                &self.input_monitor.saved,
            );
            if let Some(design) = reply.saved {
                self.input_monitor.saved.global_hotkeys |= design.hotkey.is_some();
                self.effects.selected = Some(design.id);
                if let Some(old) = self
                    .input_monitor
                    .saved
                    .effects
                    .designs
                    .iter_mut()
                    .find(|d| d.id == design.id)
                {
                    *old = design;
                } else {
                    self.input_monitor.saved.effects.designs.push(design);
                }
                self.input_monitor.save_requested = true;
                self.effects.message = Some("Toggle saved for this avatar".into());
            } else if !reply.closed {
                self.effects.editor = Some(editor);
            }
        }
        self.input_monitor.save_requested |= self.outputs.take_dirty();
        self.input_monitor.save_requested |= self.items.take_save();
        self.input_monitor.save_requested |= self.effects.take_save();
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
                && matches!(
                    std::env::var("ARIA_SMOKE_SCENARIO").as_deref(),
                    Ok("effects" | "output-effects" | "effects-audio")
                )
            {
                if self.effects.pause_smoke(self.live2d.is_some()) {
                    if let Some(path) = std::env::var_os("ARIA_SMOKE_AVATAR_PNG") {
                        self.input_monitor.export_png = Some(path.into());
                    }
                    self.scene_revision = self.scene_revision.wrapping_add(1);
                }
                if self.started.elapsed() > crate::screenshot::delay() {
                    assert!(
                        self.effects.paused,
                        "Effects smoke must advance and validate before capture"
                    );
                }
            }
            if crate::smoke_mode()
                && std::env::var_os("ARIA_SMOKE_ITEM").is_some()
                && self.started.elapsed()
                    > crate::screenshot::delay().saturating_sub(Duration::from_millis(750))
            {
                let key = egui::Id::new("smoke-verify-png");
                if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
                    let model_object = self
                        .input_monitor
                        .saved
                        .config
                        .items
                        .iter()
                        .any(|i| aria_core::items::is_model(&i.path));
                    assert_eq!(
                        self.items.draws.len(),
                        if model_object { 1 } else { 2 },
                        "All dropped object textures must load"
                    );
                    assert!(
                        self.items
                            .draws
                            .iter()
                            .all(|d| d.anchor != crate::items::Anchor::Missing && d.visible)
                    );
                    if std::env::var_os("ARIA_TEST_MODEL").is_some() {
                        assert!(
                            self.live2d.is_some(),
                            "Object drop must preserve the loaded avatar"
                        );
                    }
                    assert_eq!(
                        self.live2d.as_ref().map(|a| a.model_key.clone()),
                        ctx.data(
                            |d| d.get_temp::<Option<String>>(egui::Id::new("smoke-main-identity"))
                        )
                        .unwrap(),
                        "Stage drops must preserve the main profile identity"
                    );
                    eprintln!(
                        "Object drop and pin smoke verified: {:?}",
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
            if crate::smoke_mode()
                && matches!(
                    std::env::var("ARIA_SMOKE_SCENARIO").as_deref(),
                    Ok("image-actions") | Ok("output-images") | Ok("microphone")
                )
                && let Some(state) = &self.render_state
            {
                let scene = self.scene();
                let parameters = self.current_parameters();
                crate::image_actions::verify_smoke(
                    ctx,
                    state,
                    &scene,
                    &mut self.input_monitor.saved.config,
                    &parameters,
                    self.started.elapsed().as_secs_f32(),
                );
            }
            if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("odette-gifs") {
                if self.pending_image.is_none()
                    && self
                        .images
                        .all_loaded(&self.input_monitor.saved.config.images)
                {
                    let key = egui::Id::new("odette-ready");
                    let ready = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(key, Instant::now));
                    screenshot_capture(ctx, ready, false);
                }
            } else {
                screenshot_capture(ctx, self.started, false);
            }
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
        self.chats.show(
            ctx,
            &self.outputs.snapshot().chat,
            self.outputs.is_open(1),
            self.started,
        );
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
        RichText::new(label).size(11.0).strong().color(muted()),
        match label {
            "WORKSPACE" | "INSPECTOR" => "workspace",
            _ => "diagnostics",
        },
    );
}

fn meter(ui: &mut egui::Ui, label: &str, value: f32, min: f32, max: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.0).color(muted()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{value:.2}")).monospace().size(11.0));
        });
    });
    ui.add(
        egui::ProgressBar::new((value - min) / (max - min))
            .desired_height(5.0)
            .fill(mint()),
    );
    ui.add_space(3.0);
}

fn help_image_memory(ui: &mut egui::Ui, budget: &mut u32) {
    crate::help::control(ui, "gif-memory", |ui| {
        egui::ComboBox::from_id_salt("avatar-gif-memory")
            .selected_text(format!("{} MiB per GIF", budget))
            .show_ui(ui, |ui| {
                for n in [64, 128, 256, 512, 1024] {
                    ui.selectable_value(budget, n, format!("{n} MiB playback"));
                }
            })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_preferences_follow_avatars_while_theme_stays_global() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        settings.source = Source::Webcam;
        settings.camera.device = 2;
        settings.camera.fps = 24;
        let a = ModelPreferences::capture(&settings);
        settings.source = Source::Rtx;
        settings.camera.device = 5;
        settings.camera.fps = 60;
        let b = ModelPreferences::capture(&settings);
        settings.theme.active = theme::presets().remove(1);
        let encoded = serde_json::to_string(&(a, b)).unwrap();
        let (a, b): (ModelPreferences, ModelPreferences) = serde_json::from_str(&encoded).unwrap();
        a.restore(&mut settings);
        assert_eq!(settings.camera.device, 2);
        assert_eq!(settings.camera.fps, 24);
        assert!(settings.source == Source::Webcam);
        b.restore(&mut settings);
        assert_eq!(settings.camera.device, 5);
        assert!(settings.source == Source::Rtx);
        assert_eq!(settings.theme.active.name, "Sonoma Light");
    }
    #[test]
    fn vrm_camera_springs_and_reopen_path_stay_per_avatar() {
        let mut settings = Settings {
            vrm_avatar: Some("test-avatar.vrm".into()),
            ..Default::default()
        };
        for (key, yaw, wind) in [("vrm:one", 45., 0.3), ("vrm:two", -25., -0.4)] {
            let mut rig = SavedRig::default();
            rig.config.vrm.yaw = yaw;
            rig.config.vrm.portrait = 0.7;
            rig.config.physics.groups.insert(
                "vrm:spring:0".into(),
                aria_core::physics::GroupSettings {
                    wind,
                    ..Default::default()
                },
            );
            settings.saved_rigs.insert(key.into(), rig);
        }
        let mut memory = Memory::default();
        eframe::set_value(&mut memory, "settings", &settings);
        let restored: Settings = eframe::get_value(&memory, "settings").unwrap();
        assert_eq!(restored.vrm_avatar, settings.vrm_avatar);
        assert_eq!(restored.saved_rigs["vrm:one"].config.vrm.yaw, 45.);
        assert_eq!(
            restored.saved_rigs["vrm:two"].config.physics.groups["vrm:spring:0"].wind,
            -0.4
        );
        let legacy: RigConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.vrm, Default::default());
        let mut bad = legacy;
        bad.vrm.resolution = u32::MAX;
        assert!(bad.validate(&[]).is_err());
    }
    #[test]
    fn pitch_migration_runs_once_and_effect_libraries_stay_per_avatar() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.vts_pitch_revision, 0);
        for (name, source) in [("phone", Source::Vts), ("json", Source::Json)] {
            let mut p = ModelPreferences::capture(&Settings::default());
            p.source = source;
            p.calibration.x = 8.0;
            settings.model_preferences.insert(name.into(), p);
            let mut rig = SavedRig::default();
            rig.effects.designs[0].name = name.into();
            rig.effects.designs[0].count = if name == "phone" { 12 } else { 3 };
            settings.saved_rigs.insert(name.into(), rig);
        }
        settings.migrate_vts_pitch();
        settings.migrate_vts_pitch();
        assert_eq!(settings.model_preferences["phone"].calibration.x, -8.0);
        assert_eq!(settings.model_preferences["json"].calibration.x, 8.0);
        let mut memory = Memory::default();
        eframe::set_value(&mut memory, "settings", &settings);
        let restored: Settings = eframe::get_value(&memory, "settings").unwrap();
        assert_eq!(restored.saved_rigs["phone"].effects.designs[0].count, 12);
        assert_eq!(restored.saved_rigs["json"].effects.designs[0].count, 3);
        assert_eq!(restored.vts_pitch_revision, 1);
    }
    #[derive(Default)]
    struct Memory(BTreeMap<String, String>);
    impl eframe::Storage for Memory {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
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
            saved
                .config
                .images
                .states
                .push(aria_core::image_actions::State {
                    path: format!("{key}.gif").into(),
                    name: key.into(),
                    gif_speed: strength,
                    ..Default::default()
                });
            saved.microphone.gain_db = angle;
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
        a_outputs.chat.styles[0].background = [75, 20, 110];
        a_outputs.chat.styles[0].opacity = 0.42;
        a_outputs.chat.styles[1].visible = false;
        a_outputs.chat.docked = false;
        settings.chat_accounts.services[0].client_id = "global-test-client".into();
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
            assert_eq!(
                restored.chat_accounts.services[0].client_id,
                "global-test-client"
            );
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
        assert_eq!(
            restored.saved_rigs["avatar-a"].config.images.states[0].gif_speed,
            0.4
        );
        assert_eq!(
            restored.saved_rigs["avatar-b"].config.images.states[0].gif_speed,
            1.7
        );
        assert_eq!(restored.saved_rigs["avatar-a"].microphone.gain_db, 12.0);
        assert_eq!(restored.saved_rigs["avatar-b"].microphone.gain_db, -8.0);
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
