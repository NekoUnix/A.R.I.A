use aria_core::vrm::Lighting;
use eframe::egui;
pub fn mesh(mut mesh: egui::Mesh, light: &Lighting) -> egui::Mesh {
    if light.enabled {
        for vertex in &mut mesh.vertices {
            let tint = light.artwork_color([vertex.uv.x, vertex.uv.y]);
            let [r, g, b, a] = vertex.color.to_array();
            vertex.color = egui::Color32::from_rgba_premultiplied(
                (r as f32 * tint[0]).round() as u8,
                (g as f32 * tint[1]).round() as u8,
                (b as f32 * tint[2]).round() as u8,
                a,
            );
        }
    }
    mesh
}
pub fn panel(ui: &mut egui::Ui, monitor: &mut crate::input_monitor::InputMonitor, is_3d: bool) {
    let before = monitor.saved.config.vrm.lighting;
    let settings = &mut monitor.saved.config.vrm.lighting;
    crate::theme::category(ui, "avatar-lighting", "Avatar lighting", false, |ui| {
        crate::help::button(ui, "avatar-lighting");
        ui.checkbox(&mut settings.enabled, "Custom lighting");
        ui.add_enabled_ui(settings.enabled, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut settings.directional, true, "Directional");
                ui.selectable_value(&mut settings.directional, false, "Even / all-over light");
            });
            ui.horizontal(|ui| {
                ui.label("Light color");
                ui.color_edit_button_rgb(&mut settings.color);
            });
            if settings.directional {
                ui.add(
                    egui::Slider::new(&mut settings.azimuth, -180.0..=180.0)
                        .text("Left / right direction"),
                );
                ui.add(
                    egui::Slider::new(&mut settings.elevation, -90.0..=90.0).text("Above / below"),
                );
                ui.add(egui::Slider::new(&mut settings.ambient, 0.0..=1.0).text("Fill light"));
            }
            if ui.button("White light").clicked() {
                settings.color = [1.; 3];
            }
        });
        ui.small(if is_3d { "Directional light follows 3D surfaces. Even light removes directional shading. Saved per avatar and used in every OBS output." } else { "2D artwork uses color tint and a soft directional gradient; it has no 3D surface normals. Even light colors the artwork uniformly. Saved per avatar and used in every OBS output." });
    });
    monitor.save_requested |= before != *settings;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colored_artwork_preserves_alpha_and_even_mode_has_no_direction() {
        let mut mesh = egui::Mesh::default();
        for uv in [egui::pos2(0., 0.), egui::pos2(1., 1.)] {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: uv,
                uv,
                color: egui::Color32::from_rgba_premultiplied(100, 80, 60, 128),
            });
        }
        let mut light = Lighting {
            enabled: true,
            color: [0.5, 1., 0.25],
            directional: false,
            ..Default::default()
        };
        let even = super::mesh(mesh.clone(), &light);
        assert_eq!(even.vertices[0].color, even.vertices[1].color);
        assert_eq!(even.vertices[0].color.to_array(), [50, 80, 15, 128]);
        light.directional = true;
        light.azimuth = 80.;
        let directional = super::mesh(mesh, &light);
        assert_ne!(directional.vertices[0].color, directional.vertices[1].color);
        assert!(directional.vertices.iter().all(|v| v.color.a() == 128));
    }
}
