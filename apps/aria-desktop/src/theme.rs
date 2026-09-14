use eframe::egui::{self, Color32, RichText, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub light: bool,
    #[serde(default = "default_glass")]
    pub glass: bool,
    pub background: [u8; 3],
    pub panel: [u8; 3],
    pub card: [u8; 3],
    pub accent: [u8; 3],
    pub text: [u8; 3],
    pub muted: [u8; 3],
    pub border: [u8; 3],
    pub teal: [u8; 3],
    pub purple: [u8; 3],
    pub orange: [u8; 3],
    pub pink: [u8; 3],
}
fn default_glass() -> bool {
    true
}
impl Palette {
    pub const DARK: Self = Self {
        light: false,
        glass: true,
        background: [23, 25, 32],
        panel: [30, 33, 42],
        card: [40, 44, 56],
        accent: [100, 173, 255],
        text: [240, 244, 250],
        muted: [169, 179, 197],
        border: [61, 66, 83],
        teal: [100, 215, 200],
        purple: [188, 157, 255],
        orange: [255, 183, 112],
        pink: [244, 146, 188],
    };
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedTheme {
    pub name: String,
    pub colors: Palette,
}
pub fn presets() -> Vec<NamedTheme> {
    let light = Palette {
        light: true,
        glass: true,
        background: [236, 238, 243],
        panel: [247, 248, 251],
        card: [255, 255, 255],
        text: [29, 32, 39],
        muted: [82, 91, 109],
        border: [200, 205, 218],
        accent: [0, 99, 210],
        teal: [0, 117, 108],
        purple: [118, 63, 180],
        orange: [155, 79, 0],
        pink: [174, 50, 108],
    };
    [
        ("Sonoma Dark", Palette::DARK),
        ("Sonoma Light", light),
        (
            "Sakura",
            Palette {
                background: [250, 235, 242],
                panel: [255, 244, 249],
                accent: [178, 45, 104],
                ..light
            },
        ),
        (
            "Ocean",
            Palette {
                background: [11, 24, 36],
                panel: [17, 34, 49],
                card: [24, 45, 61],
                accent: [83, 204, 235],
                ..Palette::DARK
            },
        ),
        (
            "Forest",
            Palette {
                background: [18, 29, 25],
                panel: [26, 39, 32],
                card: [35, 52, 42],
                accent: [154, 216, 147],
                ..Palette::DARK
            },
        ),
        (
            "High Contrast",
            Palette {
                glass: false,
                background: [0, 0, 0],
                panel: [8, 8, 8],
                card: [16, 16, 16],
                text: [255, 255, 255],
                muted: [225, 225, 225],
                border: [170, 170, 170],
                accent: [255, 228, 74],
                ..Palette::DARK
            },
        ),
        (
            "Glass Dark",
            Palette {
                background: [18, 22, 32],
                panel: [30, 37, 51],
                card: [43, 51, 67],
                border: [74, 87, 110],
                accent: [132, 194, 255],
                ..Palette::DARK
            },
        ),
        (
            "Glass Light",
            Palette {
                background: [222, 231, 243],
                panel: [239, 245, 252],
                card: [255, 255, 255],
                border: [183, 198, 218],
                accent: [0, 94, 190],
                ..light
            },
        ),
    ]
    .into_iter()
    .map(|(name, colors)| NamedTheme {
        name: name.into(),
        colors,
    })
    .collect()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub active: NamedTheme,
    pub custom: Vec<NamedTheme>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            active: presets().remove(6),
            custom: vec![],
        }
    }
}
impl Settings {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        crate::help::label(ui, "Theme & colors", "themes");
        let before = self.active.colors;
        egui::ComboBox::from_id_salt("theme-picker")
            .selected_text(&self.active.name)
            .show_ui(ui, |ui| {
                for theme in presets().into_iter().chain(self.custom.iter().cloned()) {
                    if ui
                        .selectable_label(self.active.name == theme.name, &theme.name)
                        .clicked()
                    {
                        self.active = theme;
                    }
                }
            });
        caption(
            ui,
            "Applies to every avatar on this PC. Glass uses soft tint and light edges; your capture background stays under your control.",
        );
        ui.checkbox(&mut self.active.colors.glass, "Glass surfaces")
            .on_hover_text("Translucent cards and a subtle static color wash. Uses the existing UI renderer, without blur passes or background animation. Turn off for solid surfaces. High Contrast always stays solid.");
        egui::CollapsingHeader::new("Make your own theme").show(ui,|ui|{
            ui.label("Theme name"); ui.text_edit_singleline(&mut self.active.name);
            self.active.name.truncate(self.active.name.char_indices().nth(64).map_or(self.active.name.len(), |(i,_)|i));
            ui.checkbox(&mut self.active.colors.light,"Light controls").on_hover_text("Choose light or dark built-in control icons. Each surface and accent can be edited below.");
            let p=&mut self.active.colors;
            for (label,color) in [("Background",&mut p.background),("Panels",&mut p.panel),("Cards",&mut p.card),("Accent",&mut p.accent),("Text",&mut p.text),("Secondary text",&mut p.muted),("Borders",&mut p.border),("Tracking",&mut p.teal),("Avatar",&mut p.purple),("Effects",&mut p.orange),("Chat / presets",&mut p.pink)] {
                ui.horizontal(|ui|{ui.color_edit_button_srgb(color);ui.label(label);ui.monospace(crate::chroma::hex(*color));});
            }
            if contrast(p.text,p.card)<4.5 { ui.label("Text contrast is low. Darken text or lighten cards (or the reverse) for easier reading."); }
            if ui.add_enabled(!self.active.name.trim().is_empty() && (self.custom.len()<32 || self.custom.iter().any(|t|t.name==self.active.name)),egui::Button::new("Save custom theme")).clicked() {
                if presets().iter().any(|t|t.name==self.active.name){self.active.name.push_str(" (custom)");}
                if let Some(old)=self.custom.iter_mut().find(|t|t.name==self.active.name){*old=self.active.clone();} else {self.custom.push(self.active.clone());}
            }
            if ui.button("Delete saved custom theme").clicked(){self.custom.retain(|t|t.name!=self.active.name);}
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Export theme…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("ARIA theme", &["json"])
                    .set_file_name("my-theme.json")
                    .save_file()
            {
                match serde_json::to_vec_pretty(&self.active)
                    .map_err(anyhow::Error::from)
                    .and_then(|v| std::fs::write(path, v).map_err(Into::into))
                {
                    Ok(()) => {}
                    Err(e) => {
                        ui.label(e.to_string());
                    }
                }
            }
            if ui.button("Import theme…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("ARIA theme", &["json"])
                    .pick_file()
            {
                let result = (|| -> anyhow::Result<NamedTheme> {
                    anyhow::ensure!(
                        std::fs::metadata(&path)?.len() <= 8192,
                        "Theme exceeds 8 KiB"
                    );
                    parse_theme(&std::fs::read(path)?)
                })();
                match result {
                    Ok(t) => self.active = t,
                    Err(e) => {
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(egui::Id::new("theme-error"), e.to_string())
                        });
                    }
                }
            }
            if ui.button("Reset").clicked() {
                self.active = presets().remove(6);
            }
        });
        if let Some(e) = ui
            .ctx()
            .data(|d| d.get_temp::<String>(egui::Id::new("theme-error")))
        {
            ui.label(e);
        }
        if before != self.active.colors {
            apply(ui.ctx(), self.active.colors);
        }
    }
}
pub fn parse_theme(bytes: &[u8]) -> anyhow::Result<NamedTheme> {
    anyhow::ensure!(bytes.len() <= 8192, "Theme exceeds 8 KiB");
    let t: NamedTheme = serde_json::from_slice(bytes)?;
    anyhow::ensure!(
        !t.name.trim().is_empty() && t.name.chars().count() <= 64,
        "Theme name must have 1–64 characters"
    );
    Ok(t)
}
fn contrast(a: [u8; 3], b: [u8; 3]) -> f32 {
    let luminance = |rgb: [u8; 3]| {
        rgb.into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(c, w)| {
                let c = f32::from(c) / 255.;
                w * if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            })
            .sum::<f32>()
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

thread_local! { static COLORS: std::cell::Cell<Palette> = const { std::cell::Cell::new(Palette::DARK) }; }
macro_rules! colors {
    ($($name:ident: $field:ident),*) => { $(pub fn $name() -> Color32 { COLORS.with(|p| { let c=p.get().$field; Color32::from_rgb(c[0],c[1],c[2]) }) })* };
}
colors!(bg: background, panel: panel, card_color: card, mint: accent, muted: muted, text_color: text, border: border, teal: teal, purple: purple, orange: orange, pink: pink);

fn glass_enabled() -> bool {
    COLORS.with(|p| {
        let p = p.get();
        // Preserve the older High Contrast palette even when its saved JSON lacks `glass`.
        p.glass && !(p.background == [0; 3] && p.card == [16; 3] && p.text == [255; 3])
    })
}

fn translucent(color: Color32, alpha: u8) -> Color32 {
    if glass_enabled() {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    } else {
        color
    }
}

pub fn surface_edge() -> Stroke {
    Stroke::new(1.0, border())
}

pub fn glass_card() -> egui::Frame {
    egui::Frame::new()
        .fill(translucent(card_color(), 224))
        .stroke(surface_edge())
        .corner_radius(13)
        .inner_margin(8.0)
}

pub fn chrome(margin: f32) -> egui::Frame {
    egui::Frame::new()
        .fill(translucent(panel(), 210))
        .stroke(surface_edge())
        .inner_margin(margin)
}

/// A static four-vertex wash behind UI panels; no texture, blur or extra render target.
pub fn workspace_backdrop(ui: &egui::Ui) {
    if !glass_enabled() {
        return;
    }
    let rect = ui.max_rect();
    let tint = |color: Color32| bg().lerp_to_gamma(color, 0.10);
    let mut mesh = egui::Mesh::default();
    for (pos, color) in [
        (rect.left_top(), tint(purple())),
        (rect.right_top(), tint(teal())),
        (rect.right_bottom(), bg()),
        (rect.left_bottom(), tint(mint())),
    ] {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// Solid category accents; no blur, extra render targets or animated decoration.
pub fn accent(title: &str) -> Color32 {
    let title = title.to_ascii_lowercase();
    if ["physics", "spring", "tracking", "input", "microphone"]
        .iter()
        .any(|s| title.contains(s))
    {
        teal()
    } else if ["expression", "image", "appearance", "avatar", "vrm", "view"]
        .iter()
        .any(|s| title.contains(s))
    {
        purple()
    } else if ["object", "pin", "placement", "throw", "liquid", "effect"]
        .iter()
        .any(|s| title.contains(s))
    {
        orange()
    } else if ["chat", "preset", "pose", "hotkey", "keyboard"]
        .iter()
        .any(|s| title.contains(s))
    {
        pink()
    } else {
        mint()
    }
}
fn wash(color: Color32) -> Color32 {
    Color32::from_rgb(
        ((u16::from(card_color().r()) * 4 + u16::from(color.r())) / 5) as u8,
        ((u16::from(card_color().g()) * 4 + u16::from(color.g())) / 5) as u8,
        ((u16::from(card_color().b()) * 4 + u16::from(color.b())) / 5) as u8,
    )
}

pub fn install(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot")
        && let Ok(bytes) = std::fs::read(std::path::Path::new(&root).join("Fonts/segoeui.ttf"))
    {
        // Use the installed Windows font; do not redistribute it in the bundle.
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "windows-ui".into(),
            egui::FontData::from_owned(bytes).into(),
        );
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "windows-ui".into());
        ctx.set_fonts(fonts);
    }
}

pub fn apply(ctx: &egui::Context, palette: Palette) {
    COLORS.with(|p| p.set(palette));
    ctx.set_theme(if palette.light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    });
    let mut style = (*ctx.global_style()).clone();
    style.visuals = if palette.light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    style.visuals.panel_fill = translucent(panel(), 210);
    // Popups stay nearly opaque so text beneath cannot compete with their content.
    style.visuals.window_fill = translucent(card_color(), 248);
    style.visuals.extreme_bg_color = bg();
    style.visuals.faint_bg_color = card_color();
    style.visuals.override_text_color = Some(text_color());
    style.visuals.selection.bg_fill = wash(mint());
    style.visuals.selection.stroke = Stroke::new(1.0_f32, mint());
    style.visuals.hyperlink_color = mint();
    style.visuals.window_corner_radius = 16.into();
    style.visuals.window_stroke = Stroke::new(1.0_f32, border());
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.open,
        &mut style.visuals.widgets.noninteractive,
    ] {
        widget.corner_radius = 9.into();
        widget.bg_stroke = Stroke::new(1.0_f32, border());
        widget.fg_stroke = Stroke::new(1.0_f32, text_color());
    }
    style.visuals.widgets.inactive.weak_bg_fill = wash(border());
    style.visuals.widgets.inactive.bg_fill = wash(border());
    style.visuals.widgets.hovered.weak_bg_fill = wash(mint());
    style.visuals.widgets.hovered.bg_fill = wash(mint());
    style.visuals.widgets.active.bg_fill = wash(mint());
    style.visuals.widgets.active.weak_bg_fill = wash(mint());
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, mint());
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, mint());
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border());
    style.animation_time = 0.0;
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    style.spacing.item_spacing = egui::vec2(7.0, 6.0);
    style.spacing.button_padding = egui::vec2(9.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.slider_width = 90.0;
    style.spacing.indent = 12.0;
    for (text, size) in [
        (egui::TextStyle::Body, 13.0),
        (egui::TextStyle::Button, 13.0),
        (egui::TextStyle::Small, 11.0),
        (egui::TextStyle::Heading, 18.0),
    ] {
        style
            .text_styles
            .insert(text, egui::FontId::proportional(size));
    }
    ctx.set_global_style(style);
}

pub fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    glass_card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

pub fn sync(ctx: &egui::Context, palette: Palette) {
    if COLORS.with(|p| p.get() != palette) {
        apply(ctx, palette);
    }
}

pub fn open_category(ctx: &egui::Context, title: &str) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(("open-category", title)), true));
}

pub fn category(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    title: &str,
    default_open: bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    card(ui, |ui| {
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.make_persistent_id(id),
            default_open,
        );
        if ui
            .ctx()
            .data_mut(|data| data.remove_temp::<bool>(egui::Id::new(("open-category", title))))
            .unwrap_or(false)
        {
            state.set_open(true);
        }
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("capture-controls")
        {
            state.set_open(title == "Capture & performance");
        }
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("items-pin-controls")
        {
            state.set_open(matches!(
                title,
                "Stage objects" | "Pin to avatar" | "Input toggle"
            ));
        }
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("object-controls")
        {
            state.set_open(matches!(title, "Live2D object" | "Object parameters"));
        }
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("chat-controls")
        {
            state.set_open(title == "Streaming chat");
        }
        ui.horizontal(|ui| {
            let color = accent(title);
            let (badge, _) = ui.allocate_exact_size(egui::vec2(7.0, 16.0), egui::Sense::hover());
            ui.painter().circle_filled(badge.center(), 3.5, color);
            let (_, toggle) = ui.allocate_exact_size(egui::vec2(16.0, 22.0), egui::Sense::click());
            egui::collapsing_header::paint_default_icon(ui, state.openness(ui.ctx()), &toggle);
            if toggle.clicked() {
                state.toggle(ui);
            }
            if ui
                .add(
                    egui::Label::new(RichText::new(title).strong().color(text_color()))
                        .sense(egui::Sense::click()),
                )
                .clicked()
            {
                state.toggle(ui);
            }
            crate::help::button(ui, crate::help::category_topic(title));
        });
        state.show_body_unindented(ui, add);
    });
    ui.add_space(4.0);
}

pub fn caption(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).small().color(muted()));
}

/// Compact, keyboard-accessible category navigation; only the selected page is laid out.
pub fn segments<T: Copy + PartialEq>(ui: &mut egui::Ui, selected: &mut T, options: &[(T, &str)]) {
    egui::Frame::new()
        .fill(translucent(bg(), 145))
        .stroke(surface_edge())
        .corner_radius(11)
        .inner_margin(3.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            // Five workspace pages must fit without widening the fixed sidebar.
            ui.spacing_mut().button_padding.x = 3.0;
            let width =
                (ui.available_width() - (options.len() - 1) as f32 * 2.0) / options.len() as f32;
            ui.horizontal(|ui| {
                for &(value, label) in options {
                    let active = *selected == value;
                    let text = RichText::new(label).size(12.0).color(if active {
                        text_color()
                    } else {
                        muted()
                    });
                    let color = accent(label);
                    let button = egui::Button::new(text)
                        .fill(if active {
                            wash(color)
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .stroke(if active { surface_edge() } else { Stroke::NONE })
                        .corner_radius(8)
                        .selected(active);
                    let response = ui
                        .scope(|ui| {
                            ui.visuals_mut().selection.bg_fill = wash(color);
                            ui.visuals_mut().selection.stroke = Stroke::new(1.0_f32, color);
                            ui.add_sized([width, 25.0], button)
                        })
                        .inner;
                    if response.clicked() {
                        *selected = value;
                    }
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_text_is_readable_and_custom_themes_round_trip() {
        for t in presets() {
            assert!(contrast(t.colors.text, t.colors.card) >= 4.5, "{}", t.name);
            let bytes = serde_json::to_vec(&t).unwrap();
            assert_eq!(parse_theme(&bytes).unwrap().colors, t.colors);
            let mut legacy = serde_json::to_value(&t).unwrap();
            legacy["colors"].as_object_mut().unwrap().remove("glass");
            let old = parse_theme(&serde_json::to_vec(&legacy).unwrap()).unwrap();
            assert_eq!(old.colors.text, t.colors.text);
            apply(&egui::Context::default(), old.colors);
            assert_eq!(glass_enabled(), t.name != "High Contrast");
            let opaque = Palette {
                glass: false,
                ..t.colors
            };
            apply(&egui::Context::default(), opaque);
            assert_eq!(translucent(card_color(), 224).a(), 255);
        }
        apply(&egui::Context::default(), Palette::DARK);
        assert!(parse_theme(&vec![b' '; 8193]).is_err());
        let mut t = presets().remove(0);
        t.name = String::new();
        assert!(parse_theme(&serde_json::to_vec(&t).unwrap()).is_err());
    }
}
