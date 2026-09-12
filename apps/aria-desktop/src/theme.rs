use eframe::egui::{self, Color32, RichText, Stroke};

pub const BG: Color32 = Color32::from_rgb(23, 25, 32);
pub const PANEL: Color32 = Color32::from_rgb(30, 33, 42);
pub const CARD: Color32 = Color32::from_rgb(40, 44, 56);
pub const MINT: Color32 = Color32::from_rgb(100, 173, 255);
pub const MUTED: Color32 = Color32::from_rgb(169, 179, 197);
pub const TEXT: Color32 = Color32::from_rgb(240, 244, 250);
pub const BORDER: Color32 = Color32::from_rgb(61, 66, 83);
pub const TEAL: Color32 = Color32::from_rgb(100, 215, 200);
pub const PURPLE: Color32 = Color32::from_rgb(188, 157, 255);
pub const ORANGE: Color32 = Color32::from_rgb(255, 183, 112);
pub const PINK: Color32 = Color32::from_rgb(244, 146, 188);

/// Solid category accents; no blur, extra render targets or animated decoration.
pub fn accent(title: &str) -> Color32 {
    let title = title.to_ascii_lowercase();
    if ["physics", "spring", "tracking", "input", "microphone"]
        .iter()
        .any(|s| title.contains(s))
    {
        TEAL
    } else if ["expression", "image", "appearance", "avatar", "vrm", "view"]
        .iter()
        .any(|s| title.contains(s))
    {
        PURPLE
    } else if ["object", "pin", "placement", "throw", "liquid", "effect"]
        .iter()
        .any(|s| title.contains(s))
    {
        ORANGE
    } else if ["chat", "preset", "pose", "hotkey", "keyboard"]
        .iter()
        .any(|s| title.contains(s))
    {
        PINK
    } else {
        MINT
    }
}
fn wash(color: Color32) -> Color32 {
    Color32::from_rgb(
        ((u16::from(CARD.r()) * 4 + u16::from(color.r())) / 5) as u8,
        ((u16::from(CARD.g()) * 4 + u16::from(color.g())) / 5) as u8,
        ((u16::from(CARD.b()) * 4 + u16::from(color.b())) / 5) as u8,
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
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = CARD;
    style.visuals.extreme_bg_color = BG;
    style.visuals.faint_bg_color = CARD;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = Color32::from_rgb(39, 72, 110);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, MINT);
    style.visuals.hyperlink_color = MINT;
    style.visuals.window_corner_radius = 12.into();
    style.visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.open,
        &mut style.visuals.widgets.noninteractive,
    ] {
        widget.corner_radius = 7.into();
        widget.bg_stroke = Stroke::new(1.0_f32, BORDER);
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT);
    }
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(50, 55, 70);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(50, 55, 70);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(64, 72, 91);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(64, 72, 91);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(42, 84, 132);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(42, 84, 132);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
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
    ctx.set_style(style);
}

pub fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(10)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

pub fn open_category(ctx: &egui::Context, title: &str) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(("open-category", title)), true));
}

pub fn category(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
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
            ui.painter().rect_filled(badge, 3.0, color);
            let (_, toggle) = ui.allocate_exact_size(egui::vec2(16.0, 22.0), egui::Sense::click());
            egui::collapsing_header::paint_default_icon(ui, state.openness(ui.ctx()), &toggle);
            if toggle.clicked() {
                state.toggle(ui);
            }
            if ui
                .add(
                    egui::Label::new(RichText::new(title).strong().color(color))
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
    ui.label(RichText::new(text).small().color(MUTED));
}

/// Compact, keyboard-accessible category navigation; only the selected page is laid out.
pub fn segments<T: Copy + PartialEq>(ui: &mut egui::Ui, selected: &mut T, options: &[(T, &str)]) {
    egui::Frame::new()
        .fill(BG)
        .corner_radius(7)
        .inner_margin(3.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let width =
                (ui.available_width() - (options.len() - 1) as f32 * 2.0) / options.len() as f32;
            ui.horizontal(|ui| {
                for &(value, label) in options {
                    let active = *selected == value;
                    let text =
                        RichText::new(label)
                            .size(12.0)
                            .color(if active { TEXT } else { MUTED });
                    let color = accent(label);
                    let button = egui::Button::new(text)
                        .fill(if active {
                            wash(color)
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .stroke(Stroke::NONE)
                        .corner_radius(5)
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
