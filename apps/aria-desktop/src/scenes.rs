//! Named composition snapshots. Switching never starts an output or changes capture resolution.
use crate::output::{CanvasSettings, OutputSettings, OutputWindows, Transform};
use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: u64,
    pub name: String,
    pub layout: OutputSettings,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub scenes: Vec<Scene>,
    pub transition_seconds: f32,
    next_id: u64,
}
impl Library {
    pub fn capture(&mut self, name: String, layout: OutputSettings) -> anyhow::Result<u64> {
        anyhow::ensure!(self.scenes.len() < 64, "At most 64 scenes can be saved");
        self.next_id = self
            .next_id
            .max(self.scenes.iter().map(|s| s.id).max().unwrap_or(0))
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Scene IDs exhausted"))?;
        let id = self.next_id;
        self.scenes.push(Scene {
            id,
            name: name.chars().take(80).collect(),
            layout: layout.sanitized(),
        });
        Ok(id)
    }
}
struct Transition {
    from: OutputSettings,
    to: OutputSettings,
    elapsed: f32,
    duration: f32,
}
#[derive(Default)]
pub struct Scenes {
    pub active: Option<u64>,
    transition: Option<Transition>,
    name: String,
    pub error: Option<String>,
}
impl Scenes {
    pub fn recall(
        &mut self,
        id: u64,
        library: &Library,
        outputs: &mut OutputWindows,
    ) -> anyhow::Result<()> {
        let scene = library
            .scenes
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| anyhow::anyhow!("Scene no longer exists"))?;
        let from = outputs.snapshot();
        let mut to = from.clone();
        for i in 0..3 {
            copy_layout(to.canvas_mut(i), scene.layout.canvas(i));
        }
        self.active = Some(id);
        self.error = None;
        let seconds = if library.transition_seconds.is_finite() {
            library.transition_seconds.clamp(0., 5.)
        } else {
            0.
        };
        if seconds <= 0. {
            self.transition = None;
            outputs.reset(to);
        } else {
            self.transition = Some(Transition {
                from,
                to,
                elapsed: 0.,
                duration: seconds,
            });
        }
        Ok(())
    }
    /// Returns true at completion so the caller can persist final output layouts.
    pub fn update(&mut self, outputs: &mut OutputWindows, dt: f32) -> bool {
        let Some(transition) = &mut self.transition else {
            return false;
        };
        if !dt.is_finite() || dt <= 0. {
            return false;
        }
        transition.elapsed += dt;
        let t = (transition.elapsed / transition.duration).clamp(0., 1.);
        let smooth = t * t * (3. - 2. * t);
        let mut config = transition.to.clone();
        for i in 0..3 {
            blend_layout(
                config.canvas_mut(i),
                transition.from.canvas(i),
                transition.to.canvas(i),
                smooth,
            );
        }
        outputs.reset(config);
        if t >= 1. {
            self.transition = None;
            true
        } else {
            false
        }
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        library: &mut Library,
        outputs: &mut OutputWindows,
    ) -> bool {
        let mut dirty = false;
        ui.heading("Scenes");
        crate::theme::caption(
            ui,
            "Save layouts for Chat, Game, Opening or Ending. Arrange loaded avatars in the output windows, then capture a scene. All three canvas layouts switch together.",
        );
        crate::theme::caption(
            ui,
            "Scenes recall avatar positions, sizes, locks and backgrounds. They do not load avatars, start broadcasts or change output resolution.",
        );
        if let Some(error) = &self.error {
            ui.colored_label(crate::theme::orange(), error);
        }
        dirty |= ui
            .add(
                egui::Slider::new(&mut library.transition_seconds, 0.0..=5.0)
                    .text("Move transition · seconds"),
            )
            .changed();
        ui.add(
            egui::TextEdit::singleline(&mut self.name)
                .hint_text("Scene name")
                .char_limit(80),
        );
        if ui
            .add_enabled(
                !self.name.trim().is_empty() && library.scenes.len() < 64,
                egui::Button::new("Capture current layout"),
            )
            .clicked()
        {
            match library.capture(self.name.trim().into(), outputs.snapshot()) {
                Ok(id) => {
                    self.active = Some(id);
                    self.name.clear();
                    dirty = true;
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
        let mut recall = None;
        let mut remove = None;
        for scene in &mut library.scenes {
            crate::theme::card(ui, |ui| {
                ui.push_id(scene.id, |ui| {
                    dirty |= ui
                        .add(egui::TextEdit::singleline(&mut scene.name).char_limit(80))
                        .changed();
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .selectable_label(self.active == Some(scene.id), "Go to scene")
                            .clicked()
                        {
                            recall = Some(scene.id);
                        }
                        if ui.button("Update from current").clicked() {
                            scene.layout = outputs.snapshot();
                            dirty = true;
                        }
                        if ui.small_button("Remove").clicked() {
                            remove = Some(scene.id);
                        }
                    });
                    crate::theme::caption(
                        ui,
                        "Bind this scene in Hotkeys & actions, or add it to a timed graph.",
                    );
                });
            });
        }
        if let Some(id) = remove {
            library.scenes.retain(|s| s.id != id);
            if self.active == Some(id) {
                self.active = None;
            }
            dirty = true;
        }
        if let Some(id) = recall {
            match self.recall(id, library, outputs) {
                Ok(()) => dirty = true,
                Err(e) => self.error = Some(e.to_string()),
            }
        }
        dirty
    }
}
fn copy_layout(to: &mut CanvasSettings, from: &CanvasSettings) {
    to.avatars = from.avatars.clone();
    to.locked_avatars = from.locked_avatars.clone();
    to.selected_avatar = from.selected_avatar;
    to.background = from.background;
    to.key = from.key;
    to.position = from.position;
    to.zoom = from.zoom;
    to.locked = from.locked;
}
fn blend_layout(to: &mut CanvasSettings, from: &CanvasSettings, end: &CanvasSettings, t: f32) {
    let mix = |a: f32, b: f32| a + (b - a) * t;
    to.position = std::array::from_fn(|i| mix(from.position[i], end.position[i]));
    to.zoom = mix(from.zoom, end.zoom);
    for id in from.avatars.keys().chain(end.avatars.keys()) {
        let a = from.avatars.get(id).copied().unwrap_or(Transform {
            position: from.position,
            zoom: from.zoom,
        });
        let b = end.avatars.get(id).copied().unwrap_or(Transform {
            position: end.position,
            zoom: end.zoom,
        });
        to.avatars.insert(
            *id,
            Transform {
                position: std::array::from_fn(|i| mix(a.position[i], b.position[i])),
                zoom: mix(a.zoom, b.zoom),
            },
        );
    }
    // Use the exact snapshot at completion, including absent per-avatar overrides.
    if t >= 1. {
        to.avatars = end.avatars.clone();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scene_recall_preserves_capture_state_and_transition_finishes_exactly() {
        let mut library = Library {
            transition_seconds: 0.5,
            ..Default::default()
        };
        let mut target = OutputSettings::default();
        target.canvases[0].position = [0.5, -0.3];
        target.canvases[0].long_edge = 640;
        target.canvases[0].send_to_obs = false;
        let id = library.capture("Game".into(), target).unwrap();
        let mut outputs = OutputWindows::new(OutputSettings::default());
        outputs.set_open(0, true);
        let mut scenes = Scenes::default();
        scenes.recall(id, &library, &mut outputs).unwrap();
        assert!(!scenes.update(&mut outputs, 0.25));
        assert_eq!(outputs.snapshot().canvases[0].position, [0.25, -0.15]);
        assert!(scenes.update(&mut outputs, 0.25));
        let final_state = outputs.snapshot();
        assert_eq!(final_state.canvases[0].position, [0.5, -0.3]);
        assert_eq!(final_state.canvases[0].long_edge, 1920);
        assert!(final_state.canvases[0].send_to_obs);
        assert!(outputs.is_open(0));
        library.scenes.clear();
        assert!(scenes.recall(id, &library, &mut outputs).is_err());
        let replacement = library
            .capture("New".into(), OutputSettings::default())
            .unwrap();
        assert_ne!(replacement, id);
    }
}
