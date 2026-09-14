//! Small locally drawn links. Opening a page never changes the visitor's account.
use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke, Vec2};

pub const WIDTH: f32 = 116.0;
const LINKS: [(&str, &str, Color32); 4] = [
    (
        "Discord",
        "https://discord.gg/79GfpWtpct",
        Color32::from_rgb(130, 139, 255),
    ),
    (
        "X",
        "https://x.com/NekoUnix",
        Color32::from_rgb(198, 210, 228),
    ),
    (
        "YouTube",
        "https://www.youtube.com/@NekoUnix",
        Color32::from_rgb(255, 98, 113),
    ),
    (
        "Twitch",
        "https://www.twitch.tv/nekounix",
        Color32::from_rgb(185, 143, 255),
    ),
];

pub fn show(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.x = 4.0;
    for (index, (name, url, color)) in LINKS.iter().enumerate() {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                format!("NekoUnix on {name}"),
            )
        });
        if response.hovered() || response.has_focus() {
            ui.painter()
                .rect_filled(rect, 5.0, color.gamma_multiply(0.15));
        }
        let p = |x: f32, y: f32| -> Pos2 { rect.min + egui::vec2(4.0 + x, 4.0 + y) };
        let stroke = Stroke::new(1.6, *color);
        let line = |points: &[(f32, f32)]| {
            ui.painter().add(Shape::line(
                points.iter().map(|&(x, y)| p(x, y)).collect(),
                stroke,
            ))
        };
        match index {
            0 => {
                line(&[
                    (1., 12.),
                    (0., 8.),
                    (2., 3.),
                    (5., 2.),
                    (6., 4.),
                    (10., 4.),
                    (11., 2.),
                    (14., 3.),
                    (16., 8.),
                    (15., 12.),
                    (11., 14.),
                    (10., 12.),
                    (6., 12.),
                    (5., 14.),
                    (1., 12.),
                ]);
                ui.painter().circle_filled(p(5., 8.), 1.5, *color);
                ui.painter().circle_filled(p(11., 8.), 1.5, *color);
            }
            1 => {
                line(&[(1., 1.), (4.5, 1.), (15., 15.), (11.5, 15.), (1., 1.)]);
                line(&[(14., 1.), (2., 15.)]);
            }
            2 => {
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(p(0., 2.), p(16., 14.)),
                    4.0,
                    *color,
                );
                ui.painter().add(Shape::convex_polygon(
                    vec![p(6., 5.), p(6., 11.), p(11., 8.)],
                    Color32::from_rgb(25, 28, 36),
                    Stroke::NONE,
                ));
            }
            _ => {
                line(&[
                    (2., 1.),
                    (15., 1.),
                    (15., 11.),
                    (10., 15.),
                    (6., 15.),
                    (6., 12.),
                    (2., 12.),
                    (2., 1.),
                ]);
                line(&[(7., 4.), (7., 8.)]);
                line(&[(11., 4.), (11., 8.)]);
            }
        }
        if response.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(*url));
        }
        response.on_hover_text(format!("NekoUnix on {name}\nOpens your browser. Choose Follow, Subscribe or Join on the service; ARIA does not change your account."));
    }
}
