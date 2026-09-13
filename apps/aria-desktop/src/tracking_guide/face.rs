//! Small vector illustration driven by actual tracking inputs; no camera frames or GPU textures.
use crate::theme;
use aria_core::rig::Inputs;
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};

fn value(inputs: &Inputs, names: &[&str], fallback: f32) -> f32 {
    names
        .iter()
        .find_map(|name| inputs.get(*name).copied().filter(|v| v.is_finite()))
        .unwrap_or(fallback)
}
#[derive(Default)]
struct Pose {
    yaw: f32,
    pitch: f32,
    roll: f32,
    eyes: [f32; 2],
    gaze: [f32; 2],
    brows: [f32; 2],
    mouth: f32,
    smile: f32,
}
impl Pose {
    fn read(inputs: &Inputs) -> Self {
        Self {
            yaw: value(inputs, &["FaceAngleX", "ParamAngleX"], 0.0).clamp(-50.0, 50.0),
            pitch: value(inputs, &["FaceAngleY", "ParamAngleY"], 0.0).clamp(-35.0, 35.0),
            roll: value(inputs, &["FaceAngleZ", "ParamAngleZ"], 0.0).clamp(-45.0, 45.0),
            eyes: [
                value(inputs, &["EyeOpenLeft", "ParamEyeLOpen"], 1.0).clamp(0.0, 1.0),
                value(inputs, &["EyeOpenRight", "ParamEyeROpen"], 1.0).clamp(0.0, 1.0),
            ],
            gaze: [
                value(inputs, &["ParamEyeBallX"], 0.0).clamp(-1.0, 1.0),
                value(inputs, &["ParamEyeBallY"], 0.0).clamp(-1.0, 1.0),
            ],
            brows: [
                value(inputs, &["BrowLeftY", "ParamBrowLY", "Brows"], 0.0).clamp(-1.0, 1.0),
                value(inputs, &["BrowRightY", "ParamBrowRY", "Brows"], 0.0).clamp(-1.0, 1.0),
            ],
            mouth: value(inputs, &["MouthOpen", "ParamMouthOpenY", "JawOpen"], 0.0).clamp(0.0, 1.0),
            smile: value(inputs, &["ParamMouthForm", "MouthSmile"], 0.0).clamp(-1.0, 1.0),
        }
    }
    fn project(&self, p: [f32; 3], center: Pos2, scale: f32) -> Pos2 {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let (sr, cr) = self.roll.to_radians().sin_cos();
        let x = p[0] * cy + p[2] * sy;
        let z = -p[0] * sy + p[2] * cy;
        let y = p[1] * cp - z * sp;
        center + Vec2::new(x * cr - y * sr, x * sr + y * cr) * scale
    }
}

pub(super) fn show(ui: &mut egui::Ui, inputs: &Inputs, live: bool, cue: &str, rehearsal: bool) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 160.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let neutral = Inputs::new();
    let pose = Pose::read(if live { inputs } else { &neutral });
    let ink = if live {
        theme::accent("Tracking")
    } else {
        ui.visuals().weak_text_color()
    };
    let line = Stroke::new(1.7, ink);
    painter.rect_filled(rect, 12.0, ui.visuals().faint_bg_color);
    let center = Pos2::new(rect.left() + width.min(470.0) * 0.30, rect.center().y + 4.0);
    let point = |x, y, z| pose.project([x, y, z], center, 70.0);
    let oval: Vec<_> = (0..64)
        .map(|n| {
            let a = n as f32 * std::f32::consts::TAU / 64.0;
            point(a.cos() * 0.64, a.sin() * 0.91, 0.0)
        })
        .collect();
    painter.add(egui::Shape::convex_polygon(
        oval,
        ink.gamma_multiply(0.09),
        line,
    ));
    // These are illustration anchors, not detected camera landmarks.
    for x in [-0.64, 0.64] {
        painter.circle_stroke(point(x, 0.0, 0.0), 5.0, line);
    }
    for (side, x) in [0.27, -0.27].into_iter().enumerate() {
        let openness = pose.eyes[side];
        let contour: Vec<_> = (0..32)
            .map(|n| {
                let a = n as f32 * std::f32::consts::TAU / 32.0;
                point(
                    x + a.cos() * 0.16,
                    -0.20 + a.sin() * (0.012 + openness * 0.072),
                    0.07,
                )
            })
            .collect();
        painter.add(egui::Shape::closed_line(contour, line));
        if openness > 0.13 {
            painter.circle_filled(
                point(x + pose.gaze[0] * 0.085, -0.20 - pose.gaze[1] * 0.045, 0.08),
                (openness * 4.5).clamp(1.0, 4.5),
                ink,
            );
        }
        let brow_y = -0.38 - pose.brows[side] * 0.11;
        painter.line_segment(
            [
                point(x - 0.16, brow_y + 0.02, 0.04),
                point(x + 0.16, brow_y - 0.02, 0.04),
            ],
            Stroke::new(2.5, ink),
        );
    }
    painter.add(egui::Shape::line(
        vec![
            point(0.0, -0.15, 0.04),
            point(0.0, 0.12, 0.34),
            point(-0.065, 0.16, 0.08),
        ],
        line,
    ));
    let mouth: Vec<_> = (0..40)
        .map(|n| {
            let a = n as f32 * std::f32::consts::TAU / 40.0;
            let x = a.cos() * (0.19 + pose.smile.max(0.0) * 0.09);
            point(
                x,
                0.43 + a.sin() * (0.014 + pose.mouth * 0.14) - pose.smile * x.abs() * 0.36,
                0.06,
            )
        })
        .collect();
    painter.add(egui::Shape::closed_line(mouth, line));
    let label_x = if width > 500.0 {
        rect.left() + 245.0
    } else {
        rect.left() + width * 0.49
    };
    let text = ui.visuals().text_color();
    let label = |y, text: String, color| {
        painter.text(
            Pos2::new(label_x, rect.top() + y),
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::proportional(13.0),
            color,
        );
    };
    label(
        15.0,
        if rehearsal {
            "Demo rehearsal · simulated tracking"
        } else if live {
            "Live tracking face"
        } else {
            "Waiting for a live face"
        }
        .into(),
        ink,
    );
    if live {
        label(
            35.0,
            format!(
                "Turn {:+.1}°   Nod {:+.1}°   Tilt {:+.1}°",
                pose.yaw, pose.pitch, pose.roll
            ),
            text,
        );
        label(
            57.0,
            format!("Eyelids L {:.2}   R {:.2}", pose.eyes[0], pose.eyes[1]),
            text,
        );
        label(
            78.0,
            format!(
                "Mouth {:.2}   Gaze {:+.2}, {:+.2}",
                pose.mouth, pose.gaze[0], pose.gaze[1]
            ),
            text,
        );
    } else {
        label(46.0, "Capture waits while tracking is lost.".into(), text);
    }
    let focus = if cue.starts_with("yaw") {
        "Focus: head turn"
    } else if cue.starts_with("pitch") {
        "Focus: head nod"
    } else if cue.starts_with("roll") {
        "Focus: head tilt"
    } else if cue.starts_with("eye-") {
        "Focus: one eyelid"
    } else if cue.starts_with("gaze") {
        "Focus: eyes only; hold your head still"
    } else if cue == "neutral" {
        "Focus: a relaxed, steady face"
    } else {
        "Focus: the one expression described below"
    };
    label(104.0, focus.into(), ink);
    label(
        131.0,
        "Signal illustration · no camera image".into(),
        ui.visuals().weak_text_color(),
    );
    if !live {
        painter.circle_filled(rect.left_top() + egui::vec2(14.0, 14.0), 3.0, Color32::GRAY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracker_axes_are_independent_and_bad_values_remain_finite() {
        let c = Pos2::ZERO;
        let yaw = Pose::read(&Inputs::from([("FaceAngleX".into(), 30.0)]));
        let pitch = Pose::read(&Inputs::from([("FaceAngleY".into(), 30.0)]));
        assert_eq!(yaw.pitch, 0.0);
        assert_eq!(pitch.yaw, 0.0);
        assert!(yaw.project([0.0, 0.0, 0.3], c, 1.0).x.abs() > 0.1);
        assert!(pitch.project([0.0, 0.0, 0.3], c, 1.0).y.abs() > 0.1);
        let invalid = Pose::read(&Inputs::from([
            ("FaceAngleX".into(), f32::NAN),
            ("MouthOpen".into(), f32::INFINITY),
        ]));
        assert!(invalid.project([0.0, 0.0, 0.3], c, 1.0).is_finite());
        assert_eq!(invalid.mouth, 0.0);
    }
}
