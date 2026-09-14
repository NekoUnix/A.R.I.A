//! Tiny built-in bread artwork; no downloads or emoji font dependency.
use eframe::egui::{self, Color32, Sense, Vec2};

pub const WIDTH: f32 = 28.;
pub fn button(ui: &mut egui::Ui) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Send bread"));
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 5., Color32::from_rgba_unmultiplied(235, 174, 85, 40));
    }
    let key = egui::Id::new("aria-footer-bread-art");
    let texture = ui
        .ctx()
        .data(|d| d.get_temp::<egui::TextureHandle>(key))
        .unwrap_or_else(|| {
            let image = egui::ColorImage::from_rgba_unmultiplied([128, 128], &pixels());
            let texture =
                ui.ctx()
                    .load_texture("Bread button", image, egui::TextureOptions::LINEAR);
            ui.ctx().data_mut(|d| d.insert_temp(key, texture.clone()));
            texture
        });
    ui.painter().image(
        texture.id(),
        rect.shrink(1.),
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
        Color32::WHITE,
    );
    response.on_hover_text("Send bread!\nLaunches a small burst across the avatar stage and all outputs. Uses built-in artwork; no files or setup needed. Effects pause while the screenshot pose is frozen.").clicked()
}

pub fn pixels() -> Vec<u8> {
    let mut rgba = vec![0; 128 * 128 * 4];
    for y in 0..128 {
        for x in 0..128 {
            let dx = (x as f32 + 0.5) / 64. - 1.;
            let dy = (y as f32 + 0.5) / 64. - 1.;
            // A bread slice: rounded domed crown, straight base and golden crust.
            let shape = |inset: f32| {
                let body = (dx.abs() - (0.62 - inset)).max((dy - 0.20).abs() - (0.52 - inset));
                let crown =
                    ((dx / (0.81 - inset)).powi(2) + ((dy + 0.34) / (0.46 - inset)).powi(2)).sqrt()
                        - 1.;
                body.min(crown * 0.4)
            };
            let outer = shape(0.);
            let inner = shape(0.10);
            let alpha = (-outer * 90.).clamp(0., 1.);
            let crumb = (1. - inner * 50.).clamp(0., 1.);
            let shade = (dy + 1.) * 0.5;
            let crust = [199. - shade * 24., 117. - shade * 19., 46. - shade * 10.];
            let cream = [255. - shade * 8., 231. - shade * 12., 177. - shade * 13.];
            let pore = [(-0.24, -0.15), (0.22, 0.1), (-0.18, 0.4), (0.29, 0.42)]
                .iter()
                .any(|(px, py)| (dx - px).powi(2) + (dy - py).powi(2) < 0.002);
            let i = (y * 128 + x) * 4;
            for c in 0..3 {
                rgba[i + c] = (crust[c] + (cream[c] - crust[c]) * crumb
                    - if pore { 22. * crumb } else { 0. }) as u8;
            }
            rgba[i + 3] = (alpha * 255.) as u8;
        }
    }
    rgba
}

pub fn design() -> aria_core::effects::Design {
    aria_core::effects::Design {
        id: u64::MAX,
        name: "Bread across the screen".into(),
        assets: vec!["builtin:bread".into()],
        count: 5,
        interval: 0.09,
        origin: [-1.15, -0.05],
        target: [1.15, 0.05],
        flight: 1.8,
        arc: 0.13,
        spread: 0.20,
        spin: 135.,
        size: 0.14,
        size_variance: 0.3,
        lifetime: 0.1,
        bounce: 0.,
        gravity: 0.,
        impact: 0.,
        stickiness: Some(0.),
        launch_sound: Default::default(),
        impact_sound: Default::default(),
        cooldown: 0.3,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bread_crosses_the_canvas_and_expires_with_a_bounded_burst() {
        let design = super::design();
        design.validate().unwrap();
        let mut simulation = aria_core::effects::Simulation::default();
        simulation.trigger(&design).unwrap();
        simulation.tick(0.01);
        assert!(simulation.particles[0].pose().0[0] < -0.9);
        for _ in 0..155 {
            simulation.tick(0.01);
        }
        assert!(simulation.particles.iter().any(|p| p.pose().0[0] > 0.7));
        assert!(simulation.particles.len() <= 5);
        assert_eq!(simulation.impulse, [0.; 2]);
        for _ in 0..120 {
            simulation.tick(0.01);
        }
        assert!(!simulation.active());
    }
}
