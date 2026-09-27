//! Studio navigation and local session utilities. No browser runtime or remote content.
use super::*;

const NOTES_KEY: &str = "aria-studio-notes-v1";

pub(super) struct Studio {
    #[cfg(windows)]
    browser: crate::browser::Launcher,
    pub focus: bool,
    pub metrics: bool,
    pub search_open: bool,
    query: String,
    selected: usize,
    search_focus: bool,
    notes: String,
    timer: SessionTimer,
}
impl Studio {
    pub fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        Self {
            #[cfg(windows)]
            browser: Default::default(),
            focus: false,
            metrics: false,
            search_open: false,
            query: String::new(),
            selected: 0,
            search_focus: false,
            // Kept separate from avatar profiles, API state and support exports.
            notes: storage
                .and_then(|s| s.get_string(NOTES_KEY))
                .unwrap_or_default()
                .chars()
                .take(2000)
                .collect(),
            timer: SessionTimer::default(),
        }
    }
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        storage.set_string(NOTES_KEY, self.notes.clone());
    }
    fn open_search(&mut self) {
        self.search_open = true;
        self.search_focus = true;
        self.query.clear();
        self.selected = 0;
    }
}

struct SessionTimer {
    minutes: u32,
    remaining: Duration,
    running_since: Option<Instant>,
}
impl Default for SessionTimer {
    fn default() -> Self {
        Self {
            minutes: 25,
            remaining: Duration::from_secs(25 * 60),
            running_since: None,
        }
    }
}
impl SessionTimer {
    fn remaining_at(&self, now: Instant) -> Duration {
        self.remaining.saturating_sub(
            self.running_since
                .map_or(Duration::ZERO, |at| now.saturating_duration_since(at)),
        )
    }
    fn toggle(&mut self, now: Instant) {
        if self.running_since.is_some() {
            self.remaining = self.remaining_at(now);
            self.running_since = None;
        } else if !self.remaining.is_zero() {
            self.running_since = Some(now);
        }
    }
    fn reset(&mut self) {
        self.minutes = self.minutes.clamp(1, 180);
        self.remaining = Duration::from_secs(u64::from(self.minutes) * 60);
        self.running_since = None;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Command {
    Page(ControlsPage),
    Tool(Tab),
    Import,
    Save,
    Guide,
    Output,
    Help,
    Diagnostics,
    Focus,
    Hotkeys,
}
struct SearchItem {
    label: &'static str,
    detail: &'static str,
    keywords: &'static str,
    command: Command,
}
const TOOLS: &[SearchItem] = &[
    SearchItem {
        label: "Events & gestures",
        detail: "Rewards, commands, stream events and held expressions",
        keywords: "bonk bits subs raid trigger cooldown simulate",
        command: Command::Page(ControlsPage::Events),
    },
    SearchItem {
        label: "Scenes",
        detail: "Save and recall streaming layouts",
        keywords: "chat game opening ending transition",
        command: Command::Page(ControlsPage::Scenes),
    },
    SearchItem {
        label: "Music & playlists",
        detail: "Local playback and speech ducking",
        keywords: "bgm audio song shuffle volume",
        command: Command::Page(ControlsPage::Music),
    },
    SearchItem {
        label: "Home",
        detail: "Setup checklist and session tools",
        keywords: "start dashboard notes timer",
        command: Command::Page(ControlsPage::Home),
    },
    SearchItem {
        label: "Stage",
        detail: "Preview and edit your avatar",
        keywords: "studio canvas preview",
        command: Command::Page(ControlsPage::Stage),
    },
    SearchItem {
        label: "Avatar library",
        detail: "Load and manage independent profiles",
        keywords: "profiles model switch multi avatar",
        command: Command::Page(ControlsPage::Profiles),
    },
    SearchItem {
        label: "Import avatar",
        detail: "Live2D, VRM, GLB or PNG / GIF",
        keywords: "add model open load",
        command: Command::Import,
    },
    SearchItem {
        label: "Avatar settings",
        detail: "Appearance, lighting and import options",
        keywords: "avatar zoom runtime model",
        command: Command::Page(ControlsPage::Avatar),
    },
    SearchItem {
        label: "Connect tracking",
        detail: "Phone, webcam or external input",
        keywords: "iphone android vts ifacialmocap camera mediapipe nvidia",
        command: Command::Page(ControlsPage::Tracking),
    },
    SearchItem {
        label: "Guided tracking setup",
        detail: "Calibrate movement for this avatar",
        keywords: "calibration neutral face range",
        command: Command::Guide,
    },
    SearchItem {
        label: "Physics",
        detail: "Bouncy, Natural, Authored or 3D springs",
        keywords: "hair bounce jiggle spring motion",
        command: Command::Tool(Tab::Physics),
    },
    SearchItem {
        label: "Expressions",
        detail: "Face expressions and animations",
        keywords: "emotion toggle hotkey motion",
        command: Command::Tool(Tab::Expressions),
    },
    SearchItem {
        label: "Customize appearance",
        detail: "Live2D colors and saved looks",
        keywords: "clothes hair eyes outfit color",
        command: Command::Tool(Tab::Customize),
    },
    SearchItem {
        label: "Layers",
        detail: "Select and edit Live2D parts",
        keywords: "mesh part opacity hide visibility",
        command: Command::Tool(Tab::Layers),
    },
    SearchItem {
        label: "Artwork & actions",
        detail: "PNG / GIF states and triggers",
        keywords: "image pngtuber talking idle",
        command: Command::Tool(Tab::Images),
    },
    SearchItem {
        label: "3D view",
        detail: "VRM / GLB camera and avatar controls",
        keywords: "vrm glb camera orbit",
        command: Command::Tool(Tab::Vrm),
    },
    SearchItem {
        label: "Tracking inputs",
        detail: "Map parameters and monitor live input",
        keywords: "rig bindings parameter",
        command: Command::Tool(Tab::Inputs),
    },
    SearchItem {
        label: "Microphone",
        detail: "Audio lip sync and input device",
        keywords: "mic mouth voice audio lipsync",
        command: Command::Tool(Tab::Microphone),
    },
    SearchItem {
        label: "Controller",
        detail: "Gamepad mappings and movement",
        keywords: "gamepad xbox playstation",
        command: Command::Tool(Tab::Controller),
    },
    SearchItem {
        label: "Stage objects",
        detail: "Props, pinning and placement",
        keywords: "items png gif objects prop",
        command: Command::Tool(Tab::Items),
    },
    SearchItem {
        label: "Effects",
        detail: "Throws, sprays and reactions",
        keywords: "confetti bread particles",
        command: Command::Tool(Tab::Effects),
    },
    SearchItem {
        label: "Pose controls",
        detail: "Freeze and pose the selected avatar",
        keywords: "pose frozen manual",
        command: Command::Tool(Tab::Pose),
    },
    SearchItem {
        label: "Presets",
        detail: "Save and recall movement and appearance",
        keywords: "looks movement poses",
        command: Command::Tool(Tab::Presets),
    },
    SearchItem {
        label: "Hotkeys & actions",
        detail: "Keyboard shortcuts and action sequences",
        keywords: "shortcut keybind macro trigger",
        command: Command::Hotkeys,
    },
    SearchItem {
        label: "Outputs & OBS",
        detail: "Resolution, transparency and performance",
        keywords: "spout syphon linux capture portrait landscape fps",
        command: Command::Page(ControlsPage::Output),
    },
    SearchItem {
        label: "Open selected output",
        detail: "Open the configured preview canvas",
        keywords: "obs output preview broadcast",
        command: Command::Output,
    },
    SearchItem {
        label: "Streaming chat",
        detail: "Connect chat and adjust its appearance",
        keywords: "twitch youtube messages",
        command: Command::Page(ControlsPage::Chat),
    },
    SearchItem {
        label: "Settings",
        detail: "Themes, Streamer.bot and local API",
        keywords: "theme colors integration streamerbot api",
        command: Command::Page(ControlsPage::Settings),
    },
    SearchItem {
        label: "Save avatar profile",
        detail: "Save the selected avatar's settings",
        keywords: "save persist profile",
        command: Command::Save,
    },
    SearchItem {
        label: "Focus stage",
        detail: "Hide controls; Escape returns to the workspace",
        keywords: "focus distraction fullscreen",
        command: Command::Focus,
    },
    SearchItem {
        label: "Diagnostics",
        detail: "Troubleshoot and export a support report",
        keywords: "errors logs support debug",
        command: Command::Diagnostics,
    },
    SearchItem {
        label: "Help & documentation",
        detail: "Guides and feature explanations",
        keywords: "manual tutorial help",
        command: Command::Help,
    },
];

fn available(command: Command, kind: Option<crate::avatar_import::Kind>) -> bool {
    use crate::avatar_import::Kind;
    match command {
        Command::Tool(Tab::Layers | Tab::Customize) => kind == Some(Kind::Live2d),
        Command::Tool(Tab::Images) => kind.is_none() || kind == Some(Kind::Images),
        Command::Tool(Tab::Vrm) => kind.is_some_and(Kind::is_3d),
        Command::Tool(Tab::Physics | Tab::Expressions) => kind != Some(Kind::Images),
        _ => true,
    }
}
fn search(query: &str, kind: Option<crate::avatar_import::Kind>) -> Vec<&'static SearchItem> {
    let query = query.to_lowercase();
    let words: Vec<_> = query.split_whitespace().collect();
    TOOLS
        .iter()
        .filter(|item| {
            let text = format!("{} {} {}", item.label, item.detail, item.keywords).to_lowercase();
            available(item.command, kind) && words.iter().all(|w| text.contains(w))
        })
        .collect()
}

fn title(page: ControlsPage) -> &'static str {
    match page {
        ControlsPage::Home => "Home",
        ControlsPage::Stage => "Stage",
        ControlsPage::Profiles => "Avatar library",
        ControlsPage::Scenes => "Scenes",
        ControlsPage::Music => "Music & playlists",
        ControlsPage::Events => "Events & gestures",
        ControlsPage::Avatar => "Avatar settings",
        ControlsPage::Tracking => "Tracking",
        ControlsPage::Output => "Outputs & OBS",
        ControlsPage::Chat => "Streaming chat",
        ControlsPage::Settings => "Settings",
    }
}

impl AriaApp {
    pub(super) fn show_tool(&mut self, tab: Tab) {
        self.controls_page = ControlsPage::Stage;
        self.input_monitor.tab = tab;
        self.studio.focus = false;
    }
    fn run_studio_command(&mut self, command: Command, ctx: &egui::Context) {
        match command {
            Command::Page(page) => {
                self.controls_page = page;
                self.studio.focus = false;
            }
            Command::Tool(tab) => self.show_tool(tab),
            Command::Import => {
                self.studio.focus = false;
                self.importer.start(None);
            }
            Command::Save => {
                self.input_monitor.save_requested = true;
            }
            Command::Guide => {
                self.studio.focus = false;
                self.start_tracking_guide();
            }
            Command::Output => self
                .outputs
                .set_open(self.outputs.snapshot().selected, true),
            Command::Help => crate::help::open(ctx, "welcome", String::new()),
            Command::Diagnostics => self.support.open = true,
            Command::Focus => {
                self.controls_page = ControlsPage::Stage;
                self.studio.focus = !self.studio.focus;
            }
            Command::Hotkeys => {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("open-actions"), true));
            }
        }
    }
    pub(super) fn studio_shortcuts(&mut self, ctx: &egui::Context) {
        // Preserve existing user shortcuts and the recorder's ownership of keys.
        if self.action_editor.recording() {
            return;
        }
        let search_key_bound = self.custom_hotkeys().iter().any(|binding| {
            let key = binding.shortcut;
            key.key == 0x4b && key.ctrl && !key.alt && !key.shift && !key.win
        });
        if !search_key_bound
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K))
        {
            if self.studio.search_open {
                self.studio.search_open = false;
            } else {
                self.studio.open_search();
            }
        }
        if self.studio.focus
            && !self.studio.search_open
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.studio.focus = false;
        }
    }
    pub(super) fn studio_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.allocate_ui(egui::vec2(154., 38.), |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Aria")
                            .size(29.)
                            .strong()
                            .italics()
                            .color(mint()),
                    );
                    ui.vertical(|ui| {
                        ui.add_space(7.);
                        ui.label(RichText::new("AVATAR").size(9.).color(theme::teal()));
                        ui.label(RichText::new("STUDIO").size(9.).color(theme::muted()));
                    });
                });
            });
            for (page, label) in [
                (ControlsPage::Home, "Home"),
                (ControlsPage::Stage, "My stage"),
                (ControlsPage::Profiles, "Avatars"),
                (ControlsPage::Output, "Outputs"),
            ] {
                let active = self.controls_page == page;
                if ui
                    .add_sized(
                        [90., 34.],
                        egui::Button::new(label)
                            .selected(active)
                            .corner_radius(10)
                            .fill(if active {
                                theme::wash(mint())
                            } else {
                                Color32::TRANSPARENT
                            })
                            .stroke(if active {
                                Stroke::new(1., mint())
                            } else {
                                Stroke::NONE
                            }),
                    )
                    .clicked()
                {
                    self.run_studio_command(Command::Page(page), ui.ctx());
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(if self.studio.focus {
                        "Exit focus"
                    } else {
                        "Focus stage"
                    })
                    .clicked()
                {
                    self.run_studio_command(Command::Focus, ui.ctx());
                }
                if ui
                    .button("Save profile")
                    .on_hover_text("Save settings for the selected avatar")
                    .clicked()
                {
                    self.run_studio_command(Command::Save, ui.ctx());
                }
            });
        });
        ui.add_space(5.);
        ui.horizontal(|ui| {
            ui.allocate_ui(egui::vec2(154., 28.), |ui| {
                ui.label(RichText::new(title(self.controls_page)).color(theme::muted()));
            });
            let key = if cfg!(target_os = "macos") {
                "Cmd K"
            } else {
                "Ctrl K"
            };
            if ui
                .add_sized(
                    [ui.available_width(), 28.],
                    egui::Button::new(format!(
                        "Find a tool, setting or action…                                      {key}"
                    ))
                    .fill(theme::bg())
                    .corner_radius(14)
                    .stroke(theme::surface_edge()),
                )
                .clicked()
            {
                self.studio.open_search();
            }
        });
    }
    pub(super) fn studio_rail(&mut self, ui: &mut egui::Ui) {
        #[cfg(windows)]
        self.studio.browser.ui(ui);
        egui::ScrollArea::vertical()
            .id_salt("studio-rail-scroll")
            .show(ui, |ui| {
                ui.add_space(7.);
                for (page, label) in [
                    (ControlsPage::Home, "Home"),
                    (ControlsPage::Stage, "Stage"),
                    (ControlsPage::Scenes, "Scenes"),
                    (ControlsPage::Music, "Music & playlists"),
                    (ControlsPage::Events, "Events & gestures"),
                    (ControlsPage::Profiles, "Avatar library"),
                    (ControlsPage::Avatar, "Avatar settings"),
                    (ControlsPage::Tracking, "Tracking"),
                    (ControlsPage::Output, "Outputs & OBS"),
                    (ControlsPage::Chat, "Streaming chat"),
                    (ControlsPage::Settings, "Settings"),
                ] {
                    let selected = self.controls_page == page;
                    let response = ui.add_sized(
                        [ui.available_width(), 38.0],
                        egui::Button::new(format!("      {label}"))
                            .corner_radius(10)
                            .selected(selected)
                            .fill(if selected {
                                theme::wash(mint())
                            } else {
                                Color32::TRANSPARENT
                            })
                            .stroke(if selected {
                                Stroke::new(1., mint())
                            } else {
                                Stroke::NONE
                            }),
                    );
                    let center = response.rect.left_center() + egui::vec2(16., 0.);
                    let stroke = Stroke::new(1.3, if selected { mint() } else { theme::muted() });
                    let icon = egui::Rect::from_center_size(center, egui::vec2(12., 12.));
                    ui.painter()
                        .rect_stroke(icon, 3, stroke, egui::StrokeKind::Inside);
                    if matches!(page, ControlsPage::Stage | ControlsPage::Output) {
                        ui.painter().line_segment(
                            [center + egui::vec2(0., 6.), center + egui::vec2(0., 9.)],
                            stroke,
                        );
                        ui.painter().line_segment(
                            [center + egui::vec2(-4., 9.), center + egui::vec2(4., 9.)],
                            stroke,
                        );
                    } else if page == ControlsPage::Home {
                        ui.painter()
                            .line_segment([icon.left_top(), center + egui::vec2(0., -10.)], stroke);
                        ui.painter().line_segment(
                            [center + egui::vec2(0., -10.), icon.right_top()],
                            stroke,
                        );
                    } else {
                        ui.painter().circle_filled(center, 2., stroke.color);
                    }
                    if response.clicked() {
                        self.run_studio_command(Command::Page(page), ui.ctx());
                    }
                }
                ui.add_space(16.0);
                theme::caption(ui, "QUICK TOOLS");
                for (label, command) in [
                    ("+ Import avatar", Command::Import),
                    ("Hotkeys & actions", Command::Hotkeys),
                    ("Help & guides", Command::Help),
                    ("Diagnostics", Command::Diagnostics),
                ] {
                    if ui
                        .add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::new(label).frame(false),
                        )
                        .clicked()
                    {
                        self.run_studio_command(command, ui.ctx());
                    }
                }
                ui.add_space(18.0);
                theme::card(ui, |ui| {
                    theme::caption(ui, "EDITING");
                    ui.add(
                        egui::Label::new(RichText::new(self.profile_name()).strong()).truncate(),
                    )
                    .on_hover_text(self.profile_name());
                    theme::caption(
                        ui,
                        format!(
                            "{} avatars loaded",
                            self.profiles.parked.len()
                                + usize::from(self.profiles.current.is_some())
                        ),
                    );
                });
                ui.add_space(10.0);
                theme::caption(ui, format!("Alpha {}", env!("CARGO_PKG_VERSION")));
                ui.add_space(8.);
                ui.allocate_ui(egui::vec2(crate::socials::WIDTH, 28.), crate::socials::show);
                ui.allocate_ui(egui::vec2(crate::bread::WIDTH, 28.), |ui| {
                    if crate::bread::button(ui) {
                        self.effects
                            .throw_bread(ui.ctx(), self.render_state.as_ref());
                    }
                });
            });
    }
    pub(super) fn studio_footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .small_button(self.connection_status())
                .on_hover_text("Open tracking settings")
                .clicked()
            {
                self.run_studio_command(Command::Page(ControlsPage::Tracking), ui.ctx());
            }
            ui.separator();
            ui.small(format!(
                "{:.0} / {} FPS",
                self.render_fps, self.settings.fps
            ));
            let remaining = self.studio.timer.remaining_at(Instant::now());
            if self.studio.timer.running_since.is_some() {
                ui.small(if remaining.is_zero() {
                    "Timer finished".into()
                } else {
                    format!(
                        "Timer {:02}:{:02}",
                        remaining.as_secs() / 60,
                        remaining.as_secs() % 60
                    )
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.toggle_value(&mut self.studio.metrics, "Performance graphs");
                let open = (0..3).filter(|&i| self.outputs.is_open(i)).count();
                if ui.small_button(format!("Outputs: {open} open")).clicked() {
                    self.run_studio_command(Command::Page(ControlsPage::Output), ui.ctx());
                }
            });
        });
        if self.studio.metrics {
            egui::ScrollArea::horizontal()
                .id_salt("studio-metrics-scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_width(670.);
                        self.metrics.footer(ui, &self.gpu);
                    });
                });
        }
    }
    pub(super) fn studio_context(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.controls_page == ControlsPage::Home {
            self.studio_utilities(ui);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.strong(if self.controls_page == ControlsPage::Stage {
                "Inspector"
            } else {
                title(self.controls_page)
            });
            crate::help::button(ui, "workspace");
        });
        ui.add(egui::Label::new(RichText::new(self.profile_name()).color(mint())).truncate());
        theme::caption(
            ui,
            if matches!(
                self.controls_page,
                ControlsPage::Avatar | ControlsPage::Tracking | ControlsPage::Stage
            ) {
                "Changes apply to this avatar."
            } else {
                "Shared workspace settings."
            },
        );
        ui.add_space(8.0);
        if self.controls_page == ControlsPage::Stage {
            let kind = self.avatar_kind();
            self.input_monitor.navigation(ui, kind);
            egui::ScrollArea::vertical()
                .id_salt(("studio-inspector", self.input_monitor.tab))
                .show(ui, |ui| self.diagnostics(ui));
        } else {
            egui::ScrollArea::vertical()
                .id_salt(("studio-controls", self.controls_page))
                .show(ui, |ui| self.controls(ui, ctx));
        }
    }
    pub(super) fn studio_home(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("studio-home")
            .show(ui, |ui| {
                ui.add_space(10.0);
                theme::caption(ui, "YOUR AVATAR. YOUR STUDIO.");
                ui.label(RichText::new("Make yourself at home.").size(29.).strong());
                ui.label("Get ready, shape your performance, and bring it to your stream.");
                ui.add_space(20.0);
                theme::card(ui, |ui| {
                    theme::caption(ui, "CONTINUE YOUR SESSION");
                    ui.label(RichText::new(self.profile_name()).size(21.).strong());
                    ui.label(self.connection_status());
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add(
                                egui::Button::new("Open stage  →")
                                    .fill(theme::wash(mint()))
                                    .min_size(egui::vec2(140., 34.)),
                            )
                            .clicked()
                        {
                            self.run_studio_command(Command::Page(ControlsPage::Stage), ui.ctx());
                        }
                        if ui.button("+ Import avatar").clicked() {
                            self.run_studio_command(Command::Import, ui.ctx());
                        }
                        if ui.button("Avatar library").clicked() {
                            self.run_studio_command(
                                Command::Page(ControlsPage::Profiles),
                                ui.ctx(),
                            );
                        }
                    });
                });
                ui.add_space(18.0);
                ui.heading("Get stream-ready");
                theme::caption(
                    ui,
                    "A quick check for this avatar. Open each step whenever you need it.",
                );
                ui.add_space(8.0);
                let model = self.avatar_kind().is_some();
                let tracking = if self.settings.source == Source::Demo {
                    "Demo input · choose your own tracker"
                } else {
                    self.connection_status()
                };
                let tracking = tracking.to_owned();
                for (number, label, detail, command) in [
                    (
                        "01",
                        "Choose your avatar",
                        if model {
                            "Your avatar is loaded"
                        } else {
                            "Try Mica, or bring your own Live2D, VRM or images"
                        },
                        Command::Page(ControlsPage::Avatar),
                    ),
                    (
                        "02",
                        "Connect & calibrate",
                        tracking.as_str(),
                        Command::Page(ControlsPage::Tracking),
                    ),
                    (
                        "03",
                        "Prepare your output",
                        "Choose landscape, portrait or freeform; check the preview in OBS",
                        Command::Page(ControlsPage::Output),
                    ),
                ] {
                    theme::card(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(number).size(20.).color(mint()));
                            ui.vertical(|ui| {
                                ui.strong(label);
                                theme::caption(ui, detail);
                            });
                        });
                        if ui
                            .button(format!(
                                "{}  →",
                                match command {
                                    Command::Page(ControlsPage::Avatar) => "Avatar settings",
                                    Command::Page(ControlsPage::Tracking) => "Tracking settings",
                                    _ => "Output settings",
                                }
                            ))
                            .clicked()
                        {
                            self.run_studio_command(command, ui.ctx());
                        }
                    });
                    ui.add_space(5.0);
                }
                ui.add_space(14.0);
                ui.heading("Make it yours");
                ui.horizontal_wrapped(|ui| {
                    for (label, command) in [
                        ("Physics & motion", Command::Tool(Tab::Physics)),
                        ("Expressions", Command::Tool(Tab::Expressions)),
                        ("Props & effects", Command::Tool(Tab::Items)),
                        ("Hotkeys", Command::Hotkeys),
                        ("Themes", Command::Page(ControlsPage::Settings)),
                    ] {
                        if available(command, self.avatar_kind())
                            && ui
                                .add(egui::Button::new(label).min_size(egui::vec2(110., 34.)))
                                .clicked()
                        {
                            self.run_studio_command(command, ui.ctx());
                        }
                    }
                });
                ui.add_space(12.0);
                theme::caption(
                    ui,
                    "Find any tool with Ctrl K / Cmd K. Focus stage gives your avatar more room.",
                );
            });
    }
    fn studio_utilities(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("studio-utilities")
            .show(ui, |ui| {
                ui.heading("Session tools");
                theme::caption(ui, "A little space to prepare.");
                ui.add_space(12.0);
                theme::card(ui, |ui| {
                    ui.strong("Quick notes");
                    theme::caption(ui, "Talking points, reminders, ideas…");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.studio.notes)
                            .desired_rows(6)
                            .desired_width(f32::INFINITY)
                            .char_limit(2000)
                            .hint_text("What are you making today?"),
                    );
                    theme::caption(
                        ui,
                        format!(
                            "{} / 2000 · saved only on this PC",
                            self.studio.notes.chars().count()
                        ),
                    );
                });
                ui.add_space(10.0);
                theme::card(ui, |ui| {
                    ui.strong("Session timer");
                    let now = Instant::now();
                    let remaining = self.studio.timer.remaining_at(now);
                    ui.label(
                        RichText::new(format!(
                            "{:02}:{:02}",
                            remaining.as_secs() / 60,
                            remaining.as_secs() % 60
                        ))
                        .size(32.)
                        .color(mint()),
                    );
                    if remaining.is_zero() {
                        ui.label("Time for a break.");
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.studio.timer.minutes)
                                    .range(1..=180)
                                    .suffix(" min"),
                            )
                            .changed()
                        {
                            self.studio.timer.reset();
                        }
                        if ui
                            .add_enabled(
                                !remaining.is_zero(),
                                egui::Button::new(if self.studio.timer.running_since.is_some() {
                                    "Pause"
                                } else {
                                    "Start"
                                }),
                            )
                            .clicked()
                        {
                            self.studio.timer.toggle(now);
                        }
                        if ui.button("Reset").clicked() {
                            self.studio.timer.reset();
                        }
                    });
                    theme::caption(ui, "Local reminder only; never starts or stops a stream.");
                });
                ui.add_space(10.0);
                theme::card(ui, |ui| {
                    ui.strong("Session health");
                    ui.label(format!(
                        "{:.0} FPS · {} target",
                        self.render_fps, self.settings.fps
                    ));
                    if let Some(ram) = self.metrics.usage.managed_ram_bytes {
                        ui.label(format!("{:.0} MiB memory", ram as f64 / 1048576.));
                    }
                    if let Some(cpu) = self.metrics.usage.managed_cpu_percent {
                        ui.label(format!("{cpu:.1}% CPU"));
                    }
                    if ui.button("Show performance graphs").clicked() {
                        self.studio.metrics = true;
                    }
                    if ui.button("Troubleshoot a problem").clicked() {
                        self.support.open = true;
                    }
                });
            });
    }
    pub(super) fn studio_search(&mut self, ctx: &egui::Context) {
        if !self.studio.search_open {
            return;
        }
        let kind = self.avatar_kind();
        let mut command = None;
        let response = egui::Modal::new(egui::Id::new("studio-search-modal")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 60.).clamp(280., 560.));
            ui.heading("Find a tool");
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.studio.query)
                    .desired_width(f32::INFINITY)
                    .hint_text("Try physics, microphone, OBS, hotkeys…")
                    .char_limit(120),
            );
            let mut reveal = std::mem::take(&mut self.studio.search_focus);
            if reveal {
                field.request_focus();
            }
            if field.changed() {
                reveal = true;
                self.studio.selected = 0;
            }
            let results = search(&self.studio.query, kind);
            if !results.is_empty() {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                    reveal = true;
                    self.studio.selected = (self.studio.selected + 1).min(results.len() - 1);
                }
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                    reveal = true;
                    self.studio.selected = self.studio.selected.saturating_sub(1);
                }
                self.studio.selected = self.studio.selected.min(results.len() - 1);
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                    command = Some(results[self.studio.selected].command);
                }
            } else {
                ui.label("No matching tools for this avatar. Try a shorter search.");
            }
            egui::ScrollArea::vertical()
                .id_salt("studio-search-results")
                .max_height((ctx.content_rect().height() - 220.).clamp(180., 420.))
                .show(ui, |ui| {
                    for (index, item) in results.iter().enumerate() {
                        let row = ui.add_sized(
                            [ui.available_width(), 48.],
                            egui::Button::new(format!("{}\n{}", item.label, item.detail))
                                .selected(index == self.studio.selected),
                        );
                        if reveal && index == self.studio.selected {
                            row.scroll_to_me(Some(egui::Align::Center));
                        }
                        if row.clicked() {
                            command = Some(item.command);
                        }
                    }
                });
            theme::caption(ui, "↑ ↓ choose · Enter opens · Escape closes");
        });
        if response.should_close() || command.is_some() {
            self.studio.search_open = false;
        }
        if let Some(command) = command {
            self.run_studio_command(command, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_resolves_user_vocabulary_and_filters_incompatible_tools() {
        use crate::avatar_import::Kind;
        assert_eq!(
            search("HAIR bounce", Some(Kind::Live2d))[0].command,
            Command::Tool(Tab::Physics)
        );
        assert_eq!(
            search("lipsync", None)[0].command,
            Command::Tool(Tab::Microphone)
        );
        assert!(search("layers", Some(Kind::Images)).is_empty());
        assert!(!search("layers", Some(Kind::Live2d)).is_empty());
        assert!(search("3D view", Some(Kind::Live2d)).is_empty());
        assert_eq!(
            search("3D view", Some(Kind::Vrm))[0].command,
            Command::Tool(Tab::Vrm)
        );
        assert!(search("no such tool xyz", None).is_empty());
    }
    #[test]
    fn session_timer_pauses_resumes_and_saturates_without_frame_counting() {
        let mut timer = SessionTimer {
            minutes: 1,
            ..Default::default()
        };
        timer.reset();
        let now = Instant::now();
        timer.toggle(now);
        assert_eq!(
            timer.remaining_at(now + Duration::from_secs(17)),
            Duration::from_secs(43)
        );
        timer.toggle(now + Duration::from_secs(17));
        assert_eq!(
            timer.remaining_at(now + Duration::from_secs(90)),
            Duration::from_secs(43)
        );
        timer.toggle(now + Duration::from_secs(90));
        assert_eq!(
            timer.remaining_at(now + Duration::from_secs(200)),
            Duration::ZERO
        );
        timer.reset();
        assert_eq!(timer.remaining_at(now), Duration::from_secs(60));
        assert!(timer.running_since.is_none());
    }
}
