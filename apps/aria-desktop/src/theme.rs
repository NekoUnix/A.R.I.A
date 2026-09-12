use eframe::egui::{self, Color32, RichText, Stroke};

pub const BG: Color32 = Color32::from_rgb(13, 16, 26);
pub const PANEL: Color32 = Color32::from_rgb(19, 23, 36);
pub const CARD: Color32 = Color32::from_rgb(26, 32, 48);
pub const MINT: Color32 = Color32::from_rgb(119, 231, 207);
pub const MUTED: Color32 = Color32::from_rgb(157, 172, 195);
pub const TEXT: Color32 = Color32::from_rgb(227, 234, 247);
pub const BORDER: Color32 = Color32::from_rgb(46, 57, 78);

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
    style.visuals.selection.bg_fill = Color32::from_rgb(36, 76, 76);
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
        widget.corner_radius = 6.into();
        widget.bg_stroke = Stroke::new(1.0_f32, BORDER);
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT);
    }
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(36, 44, 62);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 44, 62);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(51, 66, 85);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(51, 66, 85);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(45, 98, 94);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(45, 98, 94);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
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
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
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
                "PNG items" | "Pin to avatar" | "Input toggle"
            ));
        }
        ui.horizontal(|ui| {
            let (_, toggle) = ui.allocate_exact_size(egui::vec2(16.0, 22.0), egui::Sense::click());
            egui::collapsing_header::paint_default_icon(ui, state.openness(ui.ctx()), &toggle);
            if toggle.clicked() {
                state.toggle(ui);
            }
            if ui
                .add(
                    egui::Label::new(RichText::new(title).strong().color(TEXT))
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
