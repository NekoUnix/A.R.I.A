//! Draft-based visual authoring. Preview playback owns separate effects; Save commits to this avatar.
use crate::{effects::Effects, effects_panel, help, output::Scene, theme};
use aria_core::{
    effects::{Design, Kind, Library, Liquid, Route, Selection},
    movement::{PoseMode, SavedRig},
    shortcuts::Shortcut,
};
use eframe::{
    egui::{self, Color32, Rect, Sense, Vec2, vec2},
    egui_wgpu::RenderState,
};
use std::path::Path;

pub struct Editor {
    pub draft: Design,
    pub selected: usize,
    tool: usize,
    tab: usize,
    preview: Box<Effects>,
    audition: crate::effect_audio::Audio,
    message: Option<String>,
    hex: String,
    shortcut: Shortcut,
    open: bool,
}
#[derive(Default)]
pub struct Reply {
    pub saved: Option<Design>,
    pub closed: bool,
}
impl Editor {
    #[cfg(feature = "screenshots")]
    pub fn deformation_smoke() -> Self {
        let mut d = Design {
            name: "Soft impact · avatar and object dents".into(),
            assets: vec![
                "builtin:ball".into(),
                "builtin:star".into(),
                "builtin:cube".into(),
            ],
            asset_counts: vec![1, 1, 1],
            interval: 0.3,
            flight: 0.4,
            size: 0.2,
            spread: 0.03,
            lifetime: 10.0,
            spin: 0.0,
            stickiness: Some(1.0),
            impact: 0.0,
            deformation: aria_core::deformation::Settings::gentle(),
            ..Default::default()
        };
        if let Some(path) = std::env::var_os("ARIA_SMOKE_EFFECT_ASSET") {
            d.assets.push(path.into());
            d.asset_counts.push(1);
        }
        d.deformation.avatar.depth = 0.9;
        d.deformation.avatar.radius = 0.2;
        d.deformation.avatar.squash = 0.6;
        d.deformation.avatar.hold = 4.0;
        d.deformation.object.depth = 0.9;
        d.deformation.object.squash = 0.85;
        d.deformation.object.hold = 4.0;
        let mut editor = Self::new(d);
        editor.tab = 6;
        editor.preview.pending.push(editor.draft.id);
        editor
    }
    #[cfg(feature = "screenshots")]
    pub fn smoke() -> Self {
        let mut d = Library::default().designs[3].clone();
        d.name = "Water & stars · three directions".into();
        d.routes = vec![
            Route {
                origin: [-0.75, -0.3],
                target: [-0.08, -0.16],
            },
            Route {
                origin: [0.75, -0.3],
                target: [0.08, -0.16],
            },
            Route {
                origin: [0.0, -0.75],
                target: [0.0, -0.16],
            },
        ];
        d.assets.push("builtin:star".into());
        d.asset_counts = vec![100, 6];
        if let Some(path) = std::env::var_os("ARIA_SMOKE_EFFECT_ASSET") {
            d.assets.push(path.into());
            d.asset_counts.push(1);
        }
        d.size = 0.052;
        d.spread = 0.16;
        d.interval = 0.008;
        d.flight = 0.65;
        d.lifetime = 10.0;
        d.fade_out = Some(0.7);
        d.impact = 0.0;
        let mut editor = Self::new(d);
        editor.preview.pending.push(editor.draft.id);
        editor.tab = std::env::var("ARIA_SMOKE_EDITOR_TAB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        editor
    }
    pub fn new(mut draft: Design) -> Self {
        draft.make_editable();
        Self {
            hex: format!(
                "#{:02X}{:02X}{:02X}",
                draft.tint[0], draft.tint[1], draft.tint[2]
            ),
            shortcut: draft.hotkey.unwrap_or(Shortcut {
                ctrl: true,
                alt: true,
                ..Default::default()
            }),
            draft,
            selected: 0,
            tool: 0,
            tab: 0,
            preview: Box::default(),
            audition: Default::default(),
            message: None,
            open: true,
        }
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        scene: &Scene,
        render: Option<&RenderState>,
        core: &Path,
        runtime: (Option<&crate::live2d::Avatar>, f32),
        saved: &SavedRig,
    ) -> Reply {
        let mut reply = Reply::default();
        let mut scene = scene.clone();
        scene.effects = Default::default();
        scene.recoil = [0.0; 2];
        let mut preview_design = self.draft.clone();
        preview_design.cooldown = 0.0;
        let library = Library {
            designs: vec![preview_design],
            volume: saved.effects.volume,
            muted: saved.effects.muted,
        };
        if ctx.current_pass_index() == 0 {
            self.preview.update(
                ctx,
                render,
                core,
                (&library, PoseMode::Live),
                runtime.0,
                runtime.1,
            );
        }
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("effect-editor")
            && !self.preview.paused
            && self
                .preview
                .simulation
                .particles
                .first()
                .is_some_and(|p| p.age > 1.1)
        {
            let pins = self
                .preview
                .simulation
                .particles
                .iter()
                .filter(|p| p.stuck)
                .count();
            assert!(
                self.preview.draws.len() > 50 && pins > 0,
                "Editor preview must render and attach: {:?}",
                self.preview.message
            );
            assert!(
                saved
                    .effects
                    .designs
                    .iter()
                    .all(|d| d.name != self.draft.name),
                "Draft must not alter saved library"
            );
            self.preview.paused = true;
            eprintln!(
                "Editor preview verified: {} drawings, {pins} pins, {} directions; draft isolated",
                self.preview.draws.len(),
                self.draft.routes.len()
            );
        }
        scene.effects = self.preview.draws.clone();
        scene.dents = self.preview.dents.clone();
        scene.recoil = self.preview.simulation.impulse;
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("effect-deformation")
            && !self.preview.paused
            && self
                .preview
                .simulation
                .particles
                .first()
                .is_some_and(|p| p.age > 1.6)
        {
            assert!(
                !self.preview.dents.is_empty()
                    && self.preview.draws.iter().any(|d| d.deformation.is_some()),
                "Both avatar and object deformation must be active"
            );
            if let Some(state) = render
                && let Some(path) = std::env::var_os("ARIA_SCREENSHOT_TO")
            {
                crate::deformation::verify_smoke(ctx, state, &scene, Path::new(&path));
            }
            self.preview.paused = true;
            eprintln!(
                "Deformation editor verified: {} avatar dents, {} independently deformed objects",
                self.preview.dents.len(),
                self.preview
                    .draws
                    .iter()
                    .filter(|d| d.deformation.is_some())
                    .count()
            );
        }
        let screen = ctx.content_rect();
        let height = (screen.height() - 150.0).clamp(360.0, 700.0);
        let mut open = self.open;
        egui::Window::new("Effect designer · launch, aim & preview").id(egui::Id::new("effect-designer"))
            .open(&mut open).default_size(vec2(1080.0,height+40.0)).min_width(780.0)
            .show(ctx,|ui| {
                ui.horizontal_wrapped(|ui| {
                    help::control(ui,"effect-editor",|ui|ui.add(egui::TextEdit::singleline(&mut self.draft.name).desired_width(220.0).char_limit(80)));
                    ui.label(if self.draft.kind==Kind::Spray {"Liquid spray"} else {"Throw toggle"});
                    ui.strong(format!("{} objects per trigger",self.draft.total_count()));
                    if help::control(ui,"effect-editor",|ui|ui.button("Preview burst")).clicked() {
                        match self.draft.validate() {Ok(())=>{
                            self.message=None;
                            self.preview.clear();self.preview.paused=false;
                            self.preview.pending.push(self.draft.id);
                        },Err(e)=>self.message=Some(e.to_string())}
                    }
                    help::control(ui,"effects",|ui|ui.checkbox(&mut self.preview.paused,"Pause"));
                    if help::control(ui,"effects",|ui|ui.button("Clear preview")).clicked(){self.preview.clear();}
                    if help::control(ui,"effect-assets",|ui|ui.button("Reload assets / audio")).clicked(){self.preview.reload();self.audition.clear();}
                });
                ui.columns(2,|cols| {
                    cols[0].heading("Mark paths on your avatar");
                    cols[0].horizontal_wrapped(|ui| {
                        for (i,label) in ["Drag markers","Place launch","Place aim"].into_iter().enumerate() {
                            ui.selectable_value(&mut self.tool,i,label);
                        }
                        help::button(ui,"effect-directions");
                    });
                    let (rect,_)=cols[0].allocate_exact_size(vec2(cols[0].available_width(),height-125.0),Sense::hover());
                    let painter=cols[0].painter_at(rect);
                    painter.rect_filled(rect,8.0,Color32::from_rgb(19,25,37));
                    scene.paint_subject(&painter,rect,0.62);
                    edit_canvas(&mut cols[0],rect,&scene,&mut self.draft.routes,&mut self.selected,self.tool,self.draft.arc);
                    cols[0].small("Green = launch • Pink = aim. Arrows show travel. Preview changes stay here until saved.");
                    cols[1].horizontal_wrapped(|ui| {
                        for (i,label) in ["Directions","Assets","Motion","Liquid","Sounds","Hotkey","Deformation"].into_iter().enumerate() {
                            ui.selectable_value(&mut self.tab,i,label);
                        }
                    });
                    egui::ScrollArea::vertical().id_salt("effect-editor-settings").max_height(height-75.0)
                        .show(&mut cols[1],|ui| {match self.tab {
                            0=>self.directions(ui),1=>self.assets(ui),2=>self.motion(ui),3=>self.liquid(ui),
                            4=>self.sounds(ui,saved.effects.volume),5=>self.hotkey(ui,saved),_=>self.deformation(ui),
                        }});
                });
                if let Some(m)=self.message.as_ref().or(self.preview.message.as_ref()) {ui.label(m);}
                if let Some(m)=&self.preview.audio.error {ui.colored_label(Color32::LIGHT_RED,m);}
                if let Some(m)=&self.audition.error {ui.colored_label(Color32::LIGHT_RED,m);}
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if help::control(ui,"effect-editor",|ui|ui.button("Save toggle")).clicked() {
                        match validate_save(&self.draft,saved) {
                            Ok(())=>{reply.saved=Some(self.draft.clone());reply.closed=true;},
                            Err(e)=>self.message=Some(e.to_string()),
                        }
                    }
                    if ui.button("Cancel changes").clicked(){reply.closed=true;}
                    if help::control(ui,"effect-designs",|ui|ui.button("Export draft…")).clicked()
                        && let Some(path)=rfd::FileDialog::new().set_file_name("my-toggle.aria-effect.json").save_file() {
                        self.message=Some(match effects_panel::export(&path,&self.draft){Ok(())=>"Exported design and settings; asset files remain separate.".into(),Err(e)=>e.to_string()});
                    }
                    help::button(ui,"effect-editor");
                });
            });
        self.open = open;
        reply.closed |= !open;
        reply
    }
    fn directions(&mut self, ui: &mut egui::Ui) {
        help::label(ui, "Launch paths", "effect-directions");
        ui.horizontal_wrapped(|ui| {
            for (label, origin) in [
                ("Left", [-0.75, 0.0]),
                ("Right", [0.75, 0.0]),
                ("Above", [0.0, -0.75]),
                ("Below", [0.0, 0.75]),
                ("Top left", [-0.65, -0.65]),
                ("Top right", [0.65, -0.65]),
            ] {
                if ui.small_button(label).clicked() && self.draft.routes.len() < 16 {
                    self.draft.routes.push(Route {
                        origin,
                        target: [0.0, -0.16],
                    });
                    self.selected = self.draft.routes.len() - 1;
                }
            }
        });
        let mut remove = None;
        for (i, r) in self.draft.routes.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.selected, i, format!("Direction {}", i + 1));
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
                if self.selected == i {
                    for (label, p) in [("Launch", &mut r.origin), ("Aim", &mut r.target)] {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            for v in p {
                                ui.add(egui::DragValue::new(v).range(-2.0..=2.0).speed(0.005));
                            }
                        });
                    }
                }
            });
        }
        if let Some(i) = remove
            && self.draft.routes.len() > 1
        {
            self.draft.routes.remove(i);
            self.selected = self.selected.min(self.draft.routes.len() - 1);
        }
        help::label(ui, "Use multiple directions", "effect-directions");
        for (value, label) in [
            (Selection::Cycle, "Distribute objects across paths"),
            (Selection::Random, "Pick a random path for each object"),
            (Selection::All, "Send each quantity from every path"),
        ] {
            ui.radio_value(&mut self.draft.route_selection, value, label);
        }
        ui.small("Click Place launch or Place aim, then click the image. Drag either marker to refine it. Add up to 16 directions.");
    }
    fn deformation(&mut self, ui: &mut egui::Ui) {
        help::label(ui, "Impact dents & deformation", "effect-deformation");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Gentle impact").clicked() {
                self.draft.deformation = aria_core::deformation::Settings::gentle();
            }
            if ui.button("Soft & elastic").clicked() {
                let mut settings = aria_core::deformation::Settings::gentle();
                settings.avatar.depth = 0.7;
                settings.avatar.squash = 0.5;
                settings.avatar.recovery = 1.6;
                settings.avatar.elasticity = 0.85;
                settings.object.depth = 0.6;
                settings.object.squash = 0.85;
                settings.object.recovery = 1.4;
                settings.object.elasticity = 0.85;
                self.draft.deformation = settings;
            }
            if ui.button("Disable both").clicked() {
                self.draft.deformation.avatar.enabled = false;
                self.draft.deformation.object.enabled = false;
            }
        });
        help::control(ui, "effect-deformation", |ui| {
            ui.checkbox(
                &mut self.draft.deformation.speed_sensitive,
                "Scale strength with impact speed",
            )
        });
        for (id, label, object, response) in [
            (
                "avatar-impact",
                "Avatar on stage",
                false,
                &mut self.draft.deformation.avatar,
            ),
            (
                "object-impact",
                "Thrown objects",
                true,
                &mut self.draft.deformation.object,
            ),
        ] {
            theme::category(ui, id, label, true, |ui| {
                help::control(ui, "effect-deformation", |ui| {
                    ui.checkbox(&mut response.enabled, "Enable deformation")
                });
                ui.add_enabled_ui(response.enabled, |ui| {
                    for (label, value, range) in [
                        ("Dent depth", &mut response.depth, 0.0..=1.0),
                        (
                            if object {
                                "Area / object height"
                            } else {
                                "Area / canvas height"
                            },
                            &mut response.radius,
                            if object { 0.1..=1.5 } else { 0.02..=0.6 },
                        ),
                        ("Squash & stretch", &mut response.squash, 0.0..=1.0),
                        ("Hold dent (s)", &mut response.hold, 0.0..=10.0),
                        ("Recover over (s)", &mut response.recovery, 0.05..=10.0),
                        ("Elastic spring-back", &mut response.elasticity, 0.0..=1.0),
                        ("Depth shading", &mut response.shading, 0.0..=1.0),
                    ] {
                        help::control(ui, "effect-deformation", |ui| {
                            ui.add(egui::Slider::new(value, range).text(label))
                        });
                    }
                });
                ui.small(format!(
                    "{:.2}s hold + {:.2}s recovery; then the original shape returns.",
                    response.hold, response.recovery
                ));
            });
        }
        ui.small("Preview burst shows the current settings. Pause holds deformation for inspection; Clear preview resets it. Each object deforms independently. Avatar dents follow the hit surface.");
        ui.small("Visual 2D dents and squash work with Live2D, PNG and rendered 3D props. Settings save with this toggle for this avatar.");
    }
    fn assets(&mut self, ui: &mut egui::Ui) {
        help::label(ui, "Assets and exact quantities", "effect-assets");
        let mut selection = None;
        ui.horizontal_wrapped(|ui| {
            if help::control(ui, "effect-assets", |ui| ui.button("Replace assets…")).clicked() {
                selection = Some(true);
            }
            if help::control(ui, "effect-assets", |ui| ui.button("Add assets…")).clicked() {
                selection = Some(false);
            }
        });
        if let Some(replace) = selection
            && let Some(paths) = rfd::FileDialog::new()
                .add_filter(
                    "Effect assets",
                    &[
                        "png", "gif", "moc3", "json", "glb", "gltf", "vrm", "fbx", "obj",
                    ],
                )
                .pick_files()
        {
            self.message = match self.choose_assets(paths, replace) {
                Ok(count) => Some(format!(
                    "Selected {count} files. Set quantities, then Preview burst and Save toggle."
                )),
                Err(error) => Some(format!("{error:#}")),
            };
        }
        ui.horizontal_wrapped(|ui| {
            for (label, p) in [
                ("Star", "star"),
                ("Ball", "ball"),
                ("Cube", "cube"),
                ("Anime water", "drop"),
            ] {
                if ui.small_button(label).clicked() && self.draft.assets.len() < 256 {
                    self.draft.assets.push(format!("builtin:{p}").into());
                    self.draft.asset_counts.push(1);
                }
            }
        });
        let mut remove = None;
        for (i, path) in self.draft.assets.iter().enumerate() {
            ui.push_id(i, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy())
                        .on_hover_text(path.display().to_string());
                    ui.label("Quantity");
                    ui.add(egui::DragValue::new(&mut self.draft.asset_counts[i]).range(0..=1000));
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
                if let Err(error) = crate::effects::validate_asset_path(path) {
                    ui.colored_label(Color32::LIGHT_RED, error.to_string());
                }
            });
        }
        if let Some(i) = remove {
            self.draft.assets.remove(i);
            self.draft.asset_counts.remove(i);
        }
        ui.small("Zero skips an asset. Quantities are exact; Every path multiplies them by the path count. The complete burst is limited to 1,000 objects.");
        ui.small("Live2D: keep the matching model3.json and textures. 3D: static GLB/glTF, VRM, FBX and OBJ props.");
        ui.small("Custom artwork stays in its original folder. Large PNG/GIF files load in the background on the first preview/throw. Errors identify the file; use Reload assets after repairing it.");
    }
    fn choose_assets(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        replace: bool,
    ) -> anyhow::Result<usize> {
        let available = if replace {
            256
        } else {
            256_usize.saturating_sub(self.draft.assets.len())
        };
        anyhow::ensure!(
            !paths.is_empty() && paths.len() <= available,
            "Choose 1–{available} assets"
        );
        let mut selected = Vec::new();
        for path in paths {
            crate::effects::validate_asset_path(&path)?;
            selected.push(path.canonicalize()?);
        }
        self.preview.clear();
        if replace {
            self.draft.assets.clear();
            self.draft.asset_counts.clear();
        }
        let count = selected.len();
        self.draft.assets.extend(selected);
        self.draft
            .asset_counts
            .extend(std::iter::repeat_n(1, count));
        Ok(count)
    }
    fn motion(&mut self, ui: &mut egui::Ui) {
        help::label(
            ui,
            "Speed, bounce, attachment and lifetime",
            "effect-impact",
        );
        for (label, value, range) in [
            ("Speed multiplier", &mut self.draft.speed, 0.1..=5.0),
            ("Base flight (s)", &mut self.draft.flight, 0.1..=5.0),
            ("Emission interval (s)", &mut self.draft.interval, 0.0..=5.0),
            ("Size", &mut self.draft.size, 0.005..=1.5),
            ("Size variation", &mut self.draft.size_variance, 0.0..=0.9),
            ("Aim spread", &mut self.draft.spread, 0.0..=1.0),
            ("Flight arc", &mut self.draft.arc, -1.0..=1.0),
            ("Spin (°/s)", &mut self.draft.spin, -1440.0..=1440.0),
            ("Bounce strength", &mut self.draft.bounce, 0.0..=2.0),
            ("Gravity", &mut self.draft.gravity, 0.0..=4.0),
            ("Air resistance", &mut self.draft.drag, 0.0..=5.0),
            ("Avatar recoil", &mut self.draft.impact, 0.0..=1.0),
            (
                "Stay after impact (s)",
                &mut self.draft.lifetime,
                0.1..=120.0,
            ),
            ("Cooldown (s)", &mut self.draft.cooldown, 0.0..=60.0),
        ] {
            help::control(ui, "effect-impact", |ui| {
                ui.add(egui::Slider::new(value, range).text(label))
            });
        }
        help::control(ui, "effect-impact", |ui| {
            ui.add(
                egui::Slider::new(self.draft.stickiness.as_mut().unwrap(), 0.0..=1.0)
                    .text("Stick probability"),
            )
        });
        help::control(ui, "effect-impact", |ui| {
            ui.add(
                egui::Slider::new(self.draft.fade_out.as_mut().unwrap(), 0.0..=30.0)
                    .text("Fade-out time (s)"),
            )
        });
        ui.small(format!("Flight {:.2}s + {:.2}s on scene per object. 0% sticking bounces; 100% sticks if the aim hits a surface.",self.draft.flight/self.draft.speed,self.draft.lifetime));
    }
    fn liquid(&mut self, ui: &mut egui::Ui) {
        help::label(ui, "Anime water material", "anime-liquid");
        let old_tint = self.draft.tint;
        ui.horizontal_wrapped(|ui| {
            if ui.button("Clear water").clicked() {
                self.draft.liquid = Liquid::default();
                self.draft.tint = [65, 175, 250, 220];
            }
            if ui.button("Thick paint").clicked() {
                self.draft.liquid = Liquid {
                    clarity: 0.0,
                    gloss: 0.45,
                    viscosity: 0.9,
                    drip: 0.012,
                    foam: 0.0,
                    trail: 0.2,
                };
                self.draft.tint = [225, 65, 150, 250];
            }
            if ui.button("Slime").clicked() {
                self.draft.liquid = Liquid {
                    clarity: 0.4,
                    gloss: 1.0,
                    viscosity: 0.8,
                    drip: 0.02,
                    foam: 0.1,
                    trail: 0.9,
                };
                self.draft.tint = [110, 245, 85, 220];
            }
        });
        if ui
            .color_edit_button_srgba_unmultiplied(&mut self.draft.tint)
            .changed()
            || self.draft.tint != old_tint
        {
            self.hex = format!(
                "#{:02X}{:02X}{:02X}",
                self.draft.tint[0], self.draft.tint[1], self.draft.tint[2]
            );
        }
        ui.horizontal(|ui| {
            ui.label("Hex color");
            if ui.text_edit_singleline(&mut self.hex).changed()
                && let Ok(rgb) = crate::chroma::parse_hex(&self.hex)
            {
                self.draft.tint[..3].copy_from_slice(&rgb);
            }
        });
        for (label, v) in [
            ("Gloss / highlights", &mut self.draft.liquid.gloss),
            ("Clarity", &mut self.draft.liquid.clarity),
            ("Viscosity", &mut self.draft.liquid.viscosity),
            ("Foam / splash crown", &mut self.draft.liquid.foam),
            ("Stream stretch", &mut self.draft.liquid.trail),
        ] {
            help::control(ui, "anime-liquid", |ui| {
                ui.add(egui::Slider::new(v, 0.0..=1.0).text(label))
            });
        }
        help::control(ui, "anime-liquid", |ui| {
            ui.add(egui::Slider::new(&mut self.draft.liquid.drip, 0.0..=0.2).text("Drip speed"))
        });
        help::control(ui, "anime-liquid", |ui| {
            ui.add(egui::Slider::new(&mut self.draft.splash, 0.0..=2.0).text("Splat expansion"))
        });
        ui.small("Anime water uses transparent depth, white glints, rim light, caustic bands and expanding crowns. Add Anime water in Assets and use Liquid spray mode. Custom assets keep their artwork with your tint.");
    }
    fn sounds(&mut self, ui: &mut egui::Ui, master: f32) {
        help::label(ui, "Replaceable launch and impact sounds", "effect-sounds");
        for (label, path) in [
            ("Launch", &mut self.draft.launch_sound),
            ("Impact", &mut self.draft.impact_sound),
        ] {
            ui.strong(label);
            ui.label(path.to_string_lossy());
            ui.horizontal_wrapped(|ui| {
                if ui.button("Choose clip…").clicked()
                    && let Some(p) = rfd::FileDialog::new()
                        .add_filter("Audio", &["wav", "mp3", "ogg", "flac"])
                        .pick_file()
                {
                    *path = p;
                }
                if ui.button("Listen").clicked() {
                    self.audition.play(path, self.draft.volume * master);
                }
                if ui.button("Silent").clicked() {
                    path.clear();
                }
                for name in ["whoosh", "pop", "spray", "splat"] {
                    if ui.small_button(name).clicked() {
                        *path = format!("builtin:{name}").into();
                    }
                }
            });
        }
        help::control(ui, "effect-sounds", |ui| {
            ui.add(egui::Slider::new(&mut self.draft.volume, 0.0..=1.0).text("Design volume"))
        });
    }
    fn hotkey(&mut self, ui: &mut egui::Ui, saved: &SavedRig) {
        help::label(ui, "User-defined trigger shortcut", "hotkeys");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.shortcut.ctrl, "Ctrl");
            ui.checkbox(&mut self.shortcut.alt, "Alt");
            ui.checkbox(&mut self.shortcut.shift, "Shift");
            ui.checkbox(&mut self.shortcut.win, "Win");
        });
        egui::ComboBox::from_id_salt("editor-shortcut")
            .selected_text(self.shortcut.label())
            .show_ui(ui, |ui| {
                for (key, label) in Shortcut::keys() {
                    ui.selectable_value(&mut self.shortcut.key, key, label);
                }
            });
        if ui.button("Assign to this toggle").clicked() {
            let old = self.draft.hotkey;
            self.draft.hotkey = Some(self.shortcut);
            if let Err(e) = validate_save(&self.draft, saved) {
                self.draft.hotkey = old;
                self.message = Some(e.to_string());
            }
        }
        if ui.button("Clear shortcut").clicked() {
            self.draft.hotkey = None;
        }
        if let Some(key) = self.draft.hotkey {
            ui.label(format!("Assigned: {}", key.label()));
        }
        ui.small("One press emits a burst. This shortcut becomes active when you save. Stream-tool events can trigger the saved ID through the local API.");
    }
}
pub fn validate_save(d: &Design, saved: &SavedRig) -> anyhow::Result<()> {
    d.validate()?;
    for (i, path) in d.assets.iter().enumerate() {
        if d.asset_counts.get(i) != Some(&0) {
            crate::effects::validate_asset_path(path)?;
        }
    }
    if let Some(key) = d.hotkey {
        anyhow::ensure!(
            key != Shortcut::pose()
                && !saved
                    .config
                    .images
                    .states
                    .iter()
                    .any(|s| s.hotkey == Some(key))
                && !saved
                    .expression_hotkeys
                    .values()
                    .chain(saved.item_hotkeys.values())
                    .chain(saved.layer_hotkeys.values())
                    .any(|k| *k == key)
                && !saved
                    .presets
                    .iter()
                    .filter_map(|p| p.hotkey)
                    .any(|k| Shortcut::preset(k) == key)
                && !saved
                    .effects
                    .designs
                    .iter()
                    .filter(|other| other.id != d.id)
                    .any(|other| other.hotkey == Some(key)),
            "Shortcut is assigned to another action"
        );
    }
    Ok(())
}
pub fn edit_canvas(
    ui: &mut egui::Ui,
    rect: Rect,
    scene: &Scene,
    routes: &mut [Route],
    selected: &mut usize,
    tool: usize,
    arc: f32,
) {
    let background = ui.interact(rect, ui.id().with("route-canvas"), Sense::click());
    let painter = ui.painter_at(rect);
    let rect = rect.translate(vec2(scene.recoil[0], scene.recoil[1]) * rect.height() * 0.62);
    let mut marker_active = false;
    for (i, r) in routes.iter_mut().enumerate() {
        let color = if *selected == i {
            theme::mint()
        } else {
            Color32::GRAY
        };
        let to_screen = |p| crate::items::effect_to_screen(scene, rect, 0.62, p);
        let start = to_screen(r.origin);
        let end = to_screen(r.target);
        let points: Vec<_> = (0..=24)
            .map(|n| {
                let t = n as f32 / 24.0;
                to_screen([
                    r.origin[0] + (r.target[0] - r.origin[0]) * t,
                    r.origin[1] + (r.target[1] - r.origin[1]) * t - arc * 4.0 * t * (1.0 - t),
                ])
            })
            .collect();
        let last = points[23];
        painter.add(egui::Shape::line(points, egui::Stroke::new(1.5_f32, color)));
        painter.arrow(last, end - last, egui::Stroke::new(2.0_f32, color));
        for (n, (point, pos, label, tint)) in [
            (&mut r.origin, start, "Launch", theme::mint()),
            (&mut r.target, end, "Aim", Color32::from_rgb(255, 125, 180)),
        ]
        .into_iter()
        .enumerate()
        {
            let response = ui.interact(
                Rect::from_center_size(pos, Vec2::splat(26.0)),
                ui.id().with(("route", i, n)),
                Sense::click_and_drag(),
            );
            if response.clicked() || response.dragged() {
                *selected = i;
                marker_active = true;
            }
            if response.dragged()
                && let Some(p) = response.interact_pointer_pos()
            {
                *point = crate::items::effect_from_screen(scene, rect, 0.62, p);
            }
            painter.circle_filled(pos, 6.0, tint);
            painter.circle_stroke(pos, 9.0, egui::Stroke::new(1.0_f32, tint));
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                (i + 1).to_string(),
                egui::FontId::proportional(9.0),
                theme::bg(),
            );
            if *selected == i {
                let right = pos.x > rect.center().x;
                painter.text(
                    pos + vec2(if right { -12.0 } else { 12.0 }, 10.0),
                    if right {
                        egui::Align2::RIGHT_TOP
                    } else {
                        egui::Align2::LEFT_TOP
                    },
                    label,
                    egui::FontId::proportional(11.0),
                    tint,
                );
            }
        }
    }
    if !marker_active
        && background.clicked()
        && let Some(p) = background.interact_pointer_pos()
        && let Some(route) = routes.get_mut(*selected)
    {
        let p = crate::items::effect_from_screen(scene, rect, 0.62, p);
        match tool {
            1 => route.origin = p,
            2 => route.target = p,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn choosing_custom_assets_replaces_defaults_preserves_quantities_and_saves_paths() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("Custom artwork ' sample.PNG");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 80, 40, 255]))
            .save_with_format(&png, image::ImageFormat::Png)
            .unwrap();
        let gif = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../templates/images/artwork/excited.gif");
        let mut editor = Editor::new(Design::default());
        editor.choose_assets(vec![png.clone()], true).unwrap();
        assert_eq!(editor.draft.assets, vec![png.canonicalize().unwrap()]);
        editor.draft.asset_counts[0] = 3;
        editor.choose_assets(vec![gif], false).unwrap();
        assert_eq!(editor.draft.asset_counts, [3, 1]);
        validate_save(&editor.draft, &SavedRig::default()).unwrap();
        let file = dir.path().join("my-throw.aria-effect.json");
        effects_panel::export(&file, &editor.draft).unwrap();
        assert_eq!(effects_panel::import(&file).unwrap(), editor.draft);
        let original = editor.draft.clone();
        assert!(
            editor
                .choose_assets(vec![dir.path().join("not-visual.exp3.json")], true)
                .is_err()
        );
        assert_eq!(
            editor.draft, original,
            "A rejected choice must not erase the previous assets"
        );
        std::fs::remove_file(png).unwrap();
        assert!(
            validate_save(&editor.draft, &SavedRig::default())
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
    }
    #[test]
    fn draft_edits_and_preview_do_not_change_saved_avatar_and_reject_conflicts() {
        let saved = SavedRig::default();
        let original = saved.effects.clone();
        let mut editor = Editor::new(saved.effects.designs[0].clone());
        editor.draft.name = "Draft only".into();
        editor.draft.asset_counts = vec![12];
        editor.preview.simulation.trigger(&editor.draft).unwrap();
        assert_eq!(saved.effects, original);
        editor.draft.hotkey = Some(Shortcut::pose());
        assert!(validate_save(&editor.draft, &saved).is_err());
        editor.draft.hotkey = None;
        assert!(validate_save(&editor.draft, &saved).is_ok());
        drop(editor);
        assert_eq!(saved.effects, original);
    }
    #[test]
    fn canvas_click_and_marker_drag_set_launch_and_aim_on_model_coordinates() {
        use egui::{Event, PointerButton, Pos2};
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 500.0));
        let scene = Scene {
            images: Default::default(),
            dents: Default::default(),
            effects: Default::default(),
            recoil: [0.02, -0.01],
            _model_lease: None,
            items: Default::default(),
            model: None,
            model_bounds: Rect::NOTHING,
            sprite: None,
            params: Default::default(),
        };
        let canvas = rect.translate(vec2(scene.recoil[0], scene.recoil[1]) * rect.height() * 0.62);
        let mut routes = vec![Route {
            origin: [-0.5, 0.0],
            target: [0.0, 0.0],
        }];
        let mut selected = 0;
        let mut time = 0.0;
        let mut run = |events, tool, routes: &mut Vec<Route>| {
            time += 0.1;
            let _ = crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ctx, |ui| {
                            edit_canvas(ui, rect, &scene, routes, &mut selected, tool, 0.0);
                        });
                    if ctx.current_pass_index() == 0 {
                        ctx.request_discard("Exercise multiple layout passes");
                    }
                },
            );
        };
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let aim = crate::items::effect_to_screen(&scene, canvas, 0.62, [0.2, -0.2]);
        run(vec![Event::PointerMoved(aim)], 2, &mut routes);
        run(vec![button(aim, true)], 2, &mut routes);
        run(vec![button(aim, false)], 2, &mut routes);
        assert!(
            (routes[0].target[0] - 0.2).abs() < 0.001 && (routes[0].target[1] + 0.2).abs() < 0.001
        );
        let start = crate::items::effect_to_screen(&scene, canvas, 0.62, routes[0].origin);
        let end = crate::items::effect_to_screen(&scene, canvas, 0.62, [-0.7, -0.35]);
        run(vec![Event::PointerMoved(start)], 0, &mut routes);
        run(vec![button(start, true)], 0, &mut routes);
        run(vec![Event::PointerMoved(end)], 0, &mut routes);
        run(vec![Event::PointerMoved(end)], 0, &mut routes);
        run(vec![button(end, false)], 0, &mut routes);
        assert!(
            (routes[0].origin[0] + 0.7).abs() < 0.001 && (routes[0].origin[1] + 0.35).abs() < 0.001,
            "{:?}",
            routes[0]
        );
    }
}
