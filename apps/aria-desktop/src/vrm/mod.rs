//! Primary VRM avatars, independent of Cubism Core and of static 3D throw props.
pub mod asset;
#[cfg(test)]
mod fixtures;
pub mod glb;
pub mod motion;
pub mod panel;
mod pins;
mod render;
pub mod secondary;
pub mod spring;
use anyhow::Result;
use aria_core::{
    movement::{PoseMode, RigConfig},
    rig::{Binding, Inputs, RigParameter},
};
use eframe::{egui, egui_wgpu::RenderState};
use glam::{Mat4, Quat};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

pub struct LoadJob {
    receiver: mpsc::Receiver<Result<asset::Asset>>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<String>>,
}
impl LoadJob {
    pub fn start(path: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new("Reading VRM".into()));
        let worker_cancel = cancel.clone();
        let worker_progress = progress.clone();
        std::thread::spawn(move || {
            let result = asset::load(&path, &|message| {
                anyhow::ensure!(
                    !worker_cancel.load(Ordering::Relaxed),
                    "VRM import cancelled"
                );
                *worker_progress.lock().unwrap_or_else(|e| e.into_inner()) = message.into();
                Ok(())
            });
            let _ = sender.send(result);
        });
        Self {
            receiver,
            cancel,
            progress,
        }
    }
    pub fn progress(&self) -> String {
        self.progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn poll(&self) -> Option<Result<asset::Asset>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err(anyhow::anyhow!("VRM import worker stopped")))
            }
        }
    }
}
impl Drop for LoadJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub struct Avatar {
    pub asset: asset::Asset,
    pub initial_config: RigConfig,
    pub labels: BTreeMap<String, String>,
    parameters: Vec<RigParameter>,
    renderer: render::Renderer,
    rotations: Vec<Quat>,
    world: Vec<Mat4>,
    springs: spring::Simulation,
    pub spring_groups: Vec<spring::Group>,
    weights: Vec<Vec<f32>>,
    expression_weights: Vec<f32>,
    last_values: Vec<f32>,
    last_settings: Option<aria_core::vrm::Settings>,
    last_frozen: Option<aria_core::vrm::Pose>,
    last_physics: Option<aria_core::physics::PhysicsSettings>,
    last_pose: PoseMode,
    clock: f32,
    pub motion: motion::Player,
    pub detected_bones: BTreeMap<String, usize>,
    glb_jaw: bool,
}
fn glb_default(asset: &asset::Asset, expression: &asset::Expression) -> f32 {
    expression
        .binds
        .first()
        .and_then(|b| {
            asset
                .geometry
                .iter()
                .find(|g| b.mesh == Some(g.mesh))
                .and_then(|g| g.weights.get(b.index))
        })
        .copied()
        .unwrap_or(0.)
}
pub fn expression_id(index: usize) -> String {
    format!("VRMExpression:{index}")
}
impl Avatar {
    pub fn from_asset(state: &RenderState, mut asset: asset::Asset) -> Result<Self> {
        let mut parameters: Vec<_> = aria_core::PARAMETER_SPECS
            .iter()
            .zip(aria_core::Parameters::default().0)
            .map(|((id, min, max), default)| RigParameter {
                id: (*id).into(),
                min: *min,
                max: *max,
                default,
                value: default,
            })
            .collect();
        for (id, min, max, default) in [
            ("ParamBodyAngleY", -10., 10., 0.),
            ("ParamBodyAngleZ", -10., 10., 0.),
            ("VRMArmDrop", 0., 85., 65.),
        ] {
            parameters.push(RigParameter {
                id: id.into(),
                min,
                max,
                default,
                value: default,
            });
        }
        let mut labels = BTreeMap::from([(
            "VRMArmDrop".into(),
            "Relax arms (degrees below T-pose)".into(),
        )]);
        for (id, label) in [
            ("ParamAngleX", "Head turn / yaw"),
            ("ParamAngleY", "Head nod / pitch"),
            ("ParamAngleZ", "Head lean / roll"),
            ("ParamEyeLOpen", "Left eye opening"),
            ("ParamEyeROpen", "Right eye opening"),
            ("ParamMouthOpenY", "Mouth opening / A"),
            ("ParamMouthForm", "Smile / frown"),
            ("ParamBrowLY", "Left brow height"),
            ("ParamBrowRY", "Right brow height"),
            ("ParamEyeBallX", "Horizontal gaze"),
            ("ParamEyeBallY", "Vertical gaze"),
            ("ParamBodyAngleX", "Body turn"),
            ("ParamBodyAngleY", "Body nod"),
            ("ParamBodyAngleZ", "Body lean"),
        ] {
            labels.insert(id.into(), label.into());
        }
        for (i, e) in asset.expressions.iter().enumerate() {
            let id = expression_id(i);
            labels.insert(id.clone(), format!("Expression · {}", e.name));
            let default = if asset.summary.is_glb() {
                glb_default(&asset, e)
            } else {
                0.
            };
            parameters.push(RigParameter {
                id,
                min: default.min(0.),
                max: default.max(1.),
                default,
                value: default,
            });
        }
        let mut initial_config = RigConfig::from_parameters(&parameters);
        if asset.summary.is_glb() {
            initial_config
                .bindings
                .extend(glb::bindings(&asset.expressions));
        }
        // Map additional ARKit shapes only when the file names them explicitly.
        // Mouth-open, eyelids and gaze use the standard VRM presets below.
        for (i, e) in asset.expressions.iter().enumerate() {
            let key = e.name.to_ascii_lowercase();
            if !asset.summary.is_glb()
                && (e.preset.is_empty() || e.preset == "unknown")
                && ARKIT.split_whitespace().any(|name| name == key)
                && !key.starts_with("eyeblink")
                && !key.starts_with("eyelook")
                && key != "jawopen"
            {
                initial_config.bindings.insert(
                    expression_id(i),
                    Binding::direct(&format!("ARKit:{key}"), 0., 1.),
                );
            }
        }
        let renderer = render::Renderer::new(state, &mut asset, &initial_config.vrm)?;
        let rotations = asset.nodes.iter().map(|n| n.rotation).collect();
        let world = asset.nodes.iter().map(|n| n.world).collect();
        let weights = asset.geometry.iter().map(|g| g.weights.clone()).collect();
        let expression_weights = vec![0.; asset.expressions.len()];
        let mut avatar = Self {
            detected_bones: asset.bones.clone(),
            glb_jaw: false,
            asset,
            initial_config,
            labels,
            parameters,
            renderer,
            rotations,
            world,
            springs: Default::default(),
            spring_groups: Vec::new(),
            weights,
            expression_weights,
            last_values: Vec::new(),
            last_settings: None,
            last_frozen: None,
            last_physics: None,
            last_pose: PoseMode::Live,
            clock: 0.,
            motion: Default::default(),
        };
        avatar.update(
            &Inputs::new(),
            &mut avatar.initial_config.clone(),
            &mut Default::default(),
            0.,
        )?;
        Ok(avatar)
    }
    pub fn parameters(&self) -> &[RigParameter] {
        &self.parameters
    }
    pub fn image(&self) -> crate::cubism_render::ModelImage {
        self.renderer.image
    }
    pub fn image_lease(&self) -> Arc<crate::cubism_render::ModelTexture> {
        self.renderer.lease.clone()
    }
    pub fn image_bounds(&self) -> egui::Rect {
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.))
    }
    pub fn atlas_mib(&self) -> f64 {
        (self.renderer.bytes + ((self.image().size.x * self.image().size.y) as u64 * 36)) as f64
            / 1048576.
    }
    pub fn key_palette(&self) -> Result<crate::chroma::Palette> {
        Ok(self.renderer.palette())
    }
    pub fn reset_motion(&mut self) {
        self.springs.reset();
        self.last_settings = None;
    }
    pub fn embedded_expressions(
        &self,
    ) -> Result<
        Vec<(
            aria_model::ExpressionFile,
            aria_core::expressions::Expression,
        )>,
    > {
        self.asset.expressions.iter().enumerate().map(|(i,e)|{
            let id=expression_id(i);let expression=aria_core::expressions::Expression::load(&serde_json::to_vec(&serde_json::json!({"Type":"Live2D Expression","FadeInTime":0.15,"FadeOutTime":0.15,"Parameters":[{"Id":id,"Value":1.0,"Blend":"Overwrite"}]}))?)?;
            Ok((aria_model::ExpressionFile{id,name:e.name.clone(),path:self.asset.path.clone()},expression))
        }).collect()
    }
    pub fn update(
        &mut self,
        inputs: &Inputs,
        config: &mut RigConfig,
        expressions: &mut crate::expressions_panel::ExpressionsPanel,
        dt: f32,
    ) -> Result<bool> {
        let frozen = config.pose.mode == PoseMode::Frozen;
        self.glb_jaw = self.asset.summary.is_glb()
            && !config.bindings.iter().any(|(id, b)| {
                id.starts_with("VRMExpression:")
                    && matches!(
                        b.input.as_str(),
                        "ParamMouthOpenY" | "MouthOpen" | "JawOpen" | "ARKit:jawopen"
                    )
            });
        if config.pose.mode != self.last_pose {
            if !frozen {
                self.springs.reset();
            }
            self.last_pose = config.pose.mode;
        }
        if self.asset.summary.is_glb()
            && self.last_settings.as_ref().is_none_or(|last| {
                last.bone_map != config.vrm.bone_map
                    || last.reverse_forward != config.vrm.reverse_forward
            })
        {
            self.asset.bones.clone_from(&self.detected_bones);
            for (name, node) in &config.vrm.bone_map {
                self.asset.bones.remove(name);
                if let Some(n) = node.filter(|&n| n < self.asset.nodes.len()) {
                    self.asset.bones.insert(name.clone(), n);
                }
            }
            let mut front = glb::front(&self.asset.nodes, &self.asset.bones);
            if config.vrm.reverse_forward {
                front *= Mat4::from_rotation_y(std::f32::consts::PI);
            }
            if front
                .transform_vector3(glam::Vec3::Z)
                .dot(self.asset.front.transform_vector3(glam::Vec3::Z))
                < 0.
            {
                let [min, max] = self.asset.bounds;
                self.asset.bounds = [
                    glam::vec3(-max.x, min.y, -max.z),
                    glam::vec3(-min.x, max.y, -min.z),
                ];
            }
            self.asset.front = front;
        }
        if self.last_settings.as_ref().is_none_or(|last| {
            last.secondary.auto_detect != config.vrm.secondary.auto_detect
                || last.secondary.manual_roots != config.vrm.secondary.manual_roots
                || last.bone_map != config.vrm.bone_map
        }) {
            self.spring_groups = secondary::groups(&self.asset, &config.vrm.secondary);
            self.springs.reset();
        }
        let blink_inputs;
        let inputs = if self.asset.summary.is_glb() && config.vrm.auto_blink && !frozen {
            blink_inputs = {
                let mut values = inputs.clone();
                for id in ["ParamEyeLOpen", "ParamEyeROpen"] {
                    let value = values.entry(id.into()).or_insert(1.);
                    *value = value.min(1. - self.blink());
                }
                values
            };
            &blink_inputs
        } else {
            inputs
        };
        config.evaluate_with_expressions(inputs, &mut self.parameters, dt, None, |p, active| {
            expressions.update(p, active, dt)
        });
        let changed = self.last_settings.as_ref() != Some(&config.vrm)
            || (frozen && self.last_frozen.as_ref() != Some(&config.vrm_pose))
            || self.last_physics.as_ref() != Some(&config.physics)
            || self
                .last_values
                .iter()
                .copied()
                .ne(self.parameters.iter().map(|p| p.value));
        let moving = !frozen
            && (config.physics.enabled && !self.spring_groups.is_empty()
                || config.vrm.auto_blink
                || config.vrm.motion.moving()
                || self.motion.active());
        if !changed && !moving {
            return Ok(false);
        }
        if !frozen {
            self.clock += dt.clamp(0., 0.1);
        }
        self.pose(&config.vrm, frozen.then_some(config.vrm_pose.blink));
        let offsets = if frozen {
            motion::BONES.map(|bone| {
                glam::Vec3::from_array(config.vrm_pose.motion.get(bone).copied().unwrap_or([0.; 3]))
            })
        } else {
            self.motion.update(dt, &config.vrm.motion)
        };
        for (bone, degrees) in motion::BONES.into_iter().zip(offsets) {
            if !frozen {
                config
                    .vrm_pose
                    .motion
                    .insert(bone.into(), degrees.to_array());
            }
            let bone = if bone == "chest" && !self.asset.bones.contains_key("chest") {
                "spine"
            } else {
                bone
            };
            self.add_rotation(
                bone,
                Quat::from_euler(
                    glam::EulerRot::YXZ,
                    degrees.y.to_radians(),
                    degrees.x.to_radians(),
                    degrees.z.to_radians(),
                ),
            );
        }
        spring::world_matrices(
            &self.asset.nodes,
            &self.asset.order,
            &self.rotations,
            &mut self.world,
        );
        if frozen {
            for group in &self.spring_groups {
                for joint in &group.joints {
                    if let Some(q) = config.vrm_pose.rotations.get(&joint.node) {
                        self.rotations[joint.node] = Quat::from_array(*q);
                    }
                }
            }
            spring::world_matrices(
                &self.asset.nodes,
                &self.asset.order,
                &self.rotations,
                &mut self.world,
            );
            self.last_frozen = Some(config.vrm_pose.clone());
        } else {
            self.springs.update(
                &self.asset.nodes,
                &self.asset.order,
                &self.spring_groups,
                &mut self.rotations,
                &mut self.world,
                &config.physics,
                &config.vrm.secondary,
                dt,
                false,
            );
            config.vrm_pose.rotations.clear();
            for group in &self.spring_groups {
                for joint in &group.joints {
                    config
                        .vrm_pose
                        .rotations
                        .insert(joint.node, self.rotations[joint.node].to_array());
                }
            }
            config.vrm_pose.blink = if config.vrm.auto_blink {
                self.blink()
            } else {
                0.
            };
            self.last_frozen = None;
        }
        self.renderer
            .render(&self.asset, &self.world, &self.weights, &config.vrm)?;
        self.last_values.clear();
        self.last_values
            .extend(self.parameters.iter().map(|p| p.value));
        self.last_settings = Some(config.vrm.clone());
        self.last_physics = Some(config.physics.clone());
        Ok(true)
    }
    fn value(&self, id: &str) -> f32 {
        self.parameters
            .iter()
            .find(|p| p.id == id)
            .map_or(0., |p| p.value)
    }
    fn rotate(&mut self, bone: &str, rotation: Quat) {
        if let Some(&node) = self.asset.bones.get(bone) {
            let front = Quat::from_mat4(&self.asset.front);
            let native = front.inverse() * rotation * front;
            let parent = self.asset.nodes[node].parent.map_or(Quat::IDENTITY, |p| {
                self.asset.nodes[p].world.to_scale_rotation_translation().1
            });
            self.rotations[node] =
                (parent.inverse() * native * parent * self.asset.nodes[node].rotation).normalize();
        }
    }
    fn add_rotation(&mut self, bone: &str, rotation: Quat) {
        if let Some(&node) = self.asset.bones.get(bone) {
            let front = Quat::from_mat4(&self.asset.front);
            let native = front.inverse() * rotation * front;
            let parent = self.asset.nodes[node].parent.map_or(Quat::IDENTITY, |p| {
                self.asset.nodes[p].world.to_scale_rotation_translation().1
            });
            self.rotations[node] =
                (parent.inverse() * native * parent * self.rotations[node]).normalize();
        }
    }
    fn blink(&self) -> f32 {
        let phase = self.clock.rem_euclid(4.2);
        if phase < 0.18 {
            (phase / 0.18 * std::f32::consts::PI).sin()
        } else {
            0.
        }
    }
    fn pose(&mut self, settings: &aria_core::vrm::Settings, frozen_blink: Option<f32>) {
        for (q, n) in self.rotations.iter_mut().zip(&self.asset.nodes) {
            *q = n.rotation;
        }
        let head = |scale: f32| {
            Quat::from_euler(
                glam::EulerRot::YXZ,
                self.value("ParamAngleX").to_radians() * scale,
                -self.value("ParamAngleY").to_radians() * scale,
                -self.value("ParamAngleZ").to_radians() * scale,
            )
        };
        let neck = head(0.2);
        let head = head(if self.asset.bones.contains_key("neck") {
            0.8
        } else {
            1.
        });
        self.rotate("neck", neck);
        self.rotate("head", head);
        if self.glb_jaw {
            self.rotate(
                "jaw",
                Quat::from_rotation_x(self.value("ParamMouthOpenY") * 20f32.to_radians()),
            );
        }
        let body = Quat::from_euler(
            glam::EulerRot::YXZ,
            self.value("ParamBodyAngleX").to_radians(),
            -self.value("ParamBodyAngleY").to_radians(),
            -self.value("ParamBodyAngleZ").to_radians(),
        );
        self.rotate("spine", body);
        self.rotate(
            "leftUpperArm",
            Quat::from_rotation_z(-self.value("VRMArmDrop").to_radians()),
        );
        self.rotate(
            "rightUpperArm",
            Quat::from_rotation_z(self.value("VRMArmDrop").to_radians()),
        );
        let blink = frozen_blink.unwrap_or_else(|| {
            if settings.auto_blink {
                self.blink()
            } else {
                0.
            }
        });
        let left = (1. - self.value("ParamEyeLOpen")).max(blink);
        let right = (1. - self.value("ParamEyeROpen")).max(blink);
        let mouth = self.value("ParamMouthOpenY");
        let mut allow = [1f32; 3];
        for (i, e) in self.asset.expressions.iter().enumerate() {
            let value = self.parameters[15 + i].value;
            let weight = if e.binary {
                if value > 0.5 { 1. } else { 0. }
            } else {
                value
            };
            self.expression_weights[i] = weight;
            for (a, mode) in allow.iter_mut().zip(&e.overrides) {
                match mode.as_str() {
                    "block" if weight > 0. => *a = 0.,
                    "blend" => *a = (*a - weight).max(0.),
                    _ => {}
                }
            }
        }
        let has = |name: &str| {
            self.asset
                .expressions
                .iter()
                .any(|e| e.preset == name && !e.binds.is_empty())
        };
        let separate_blink =
            has("blink_l") && has("blink_r") || has("blinkleft") && has("blinkright");
        let gaze_x = self.value("ParamEyeBallX");
        let gaze_y = self.value("ParamEyeBallY");
        let smile = self.value("ParamMouthForm");
        let brow_l = self.value("ParamBrowLY");
        let brow_r = self.value("ParamBrowRY");
        for (i, e) in self.asset.expressions.iter().enumerate() {
            if self.asset.summary.is_glb() {
                continue;
            }
            let auto = match e.preset.as_str() {
                "blink" if !separate_blink => left.max(right) * allow[0],
                "blink_l" | "blinkleft" if separate_blink => left * allow[0],
                "blink_r" | "blinkright" if separate_blink => right * allow[0],
                "a" | "aa" => mouth * allow[2],
                "lookup" if self.asset.look_expression => gaze_y.max(0.) * allow[1],
                "lookdown" if self.asset.look_expression => (-gaze_y).max(0.) * allow[1],
                "lookleft" if self.asset.look_expression => gaze_x.max(0.) * allow[1],
                "lookright" if self.asset.look_expression => (-gaze_x).max(0.) * allow[1],
                _ => match e.name.to_ascii_lowercase().as_str() {
                    "mouthsmileleft" | "mouthsmileright" => smile.max(0.),
                    "mouthfrownleft" | "mouthfrownright" => (-smile).max(0.),
                    "browouterupleft" => brow_l.max(0.),
                    "browouterupright" => brow_r.max(0.),
                    "browdownleft" => (-brow_l).max(0.),
                    "browdownright" => (-brow_r).max(0.),
                    _ => 0.,
                },
            };
            self.expression_weights[i] = self.expression_weights[i].max(if e.binary {
                if auto > 0.5 { 1. } else { 0. }
            } else {
                auto
            });
        }
        if !self.asset.look_expression {
            for (bone, horizontal) in [
                (
                    "leftEye",
                    if gaze_x > 0. {
                        self.asset.eye_ranges[1]
                    } else {
                        self.asset.eye_ranges[0]
                    },
                ),
                (
                    "rightEye",
                    if gaze_x < 0. {
                        self.asset.eye_ranges[1]
                    } else {
                        self.asset.eye_ranges[0]
                    },
                ),
            ] {
                let vertical = if gaze_y > 0. {
                    self.asset.eye_ranges[2]
                } else {
                    self.asset.eye_ranges[3]
                };
                self.rotate(
                    bone,
                    Quat::from_euler(
                        glam::EulerRot::YXZ,
                        gaze_x * horizontal.to_radians() * allow[1],
                        -gaze_y * vertical.to_radians() * allow[1],
                        0.,
                    ),
                );
            }
        }
        for (weights, g) in self.weights.iter_mut().zip(&self.asset.geometry) {
            weights.clone_from(&g.weights);
            for (e, &w) in self.asset.expressions.iter().zip(&self.expression_weights) {
                if w > 0. || self.asset.summary.is_glb() {
                    for b in &e.binds {
                        if b.node.is_none_or(|n| n == g.node) && b.mesh.is_none_or(|m| m == g.mesh)
                        {
                            if self.asset.summary.is_glb() {
                                weights[b.index] = b.weight * w;
                            } else {
                                weights[b.index] += b.weight * w;
                            }
                        }
                    }
                }
            }
            if !self.asset.summary.is_glb() {
                for w in weights {
                    *w = w.clamp(0., 1.);
                }
            }
        }
    }
}

const ARKIT: &str = "browdownleft browdownright browinnerup browouterupleft browouterupright cheekpuff cheeksquintleft cheeksquintright eyeblinkleft eyeblinkright eyelookdownleft eyelookdownright eyelookinleft eyelookinright eyelookoutleft eyelookoutright eyelookupleft eyelookupright eyesquintleft eyesquintright eyewideleft eyewideright jawforward jawleft jawopen jawright mouthclose mouthdimpleleft mouthdimpleright mouthfrownleft mouthfrownright mouthfunnel mouthleft mouthlowerdownleft mouthlowerdownright mouthpressleft mouthpressright mouthpucker mouthright mouthrolllower mouthrollupper mouthshruglower mouthshrugupper mouthsmileleft mouthsmileright mouthstretchleft mouthstretchright mouthupperupleft mouthupperupright nosesneerleft nosesneerright tongueout";

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    #[cfg(windows)]
    #[ignore = "requires ARIA_TEST_VRM and a DX12 GPU; private artwork is read in place"]
    fn local_vrm_tracks_expressions_springs_and_freezes() {
        let path = std::env::var_os("ARIA_TEST_VRM").unwrap();
        exercise_avatar(Path::new(&path));
    }
    #[test]
    #[ignore = "requires ARIA_TEST_GLB and a DX12 GPU; private artwork is read in place"]
    fn local_glb_tracks_expressions_gestures_and_freezes() {
        let path = std::env::var_os("ARIA_TEST_GLB").unwrap();
        exercise_avatar(Path::new(&path));
    }
    fn exercise_avatar(path: &Path) {
        let asset = asset::load(path, &|message| {
            println!("{message}");
            Ok(())
        })
        .unwrap();
        println!(
            "VRM {}: {} bones, {} expressions, {} spring groups, {} unique vertices, {} draw parts",
            asset.summary.version,
            asset.bones.len(),
            asset.expressions.len(),
            asset.springs.len(),
            asset
                .geometry
                .iter()
                .map(|g| g.vertices.len())
                .sum::<usize>(),
            asset.parts.len()
        );
        let state = crate::spout::tests::gpu_state();
        let mut avatar = Avatar::from_asset(&state, asset).unwrap();
        let mut config = avatar.initial_config.clone();
        config.physics.enabled = false;
        config.vrm.motion.enabled = false;
        let mut expressions = crate::expressions_panel::ExpressionsPanel::default();
        let neutral =
            aria_core::rig::tracking_inputs(None, aria_core::Parameters::default(), false, 1.);
        avatar
            .update(&neutral, &mut config, &mut expressions, 0.)
            .unwrap();
        let before = avatar.renderer.read_rgba().unwrap();
        let opaque = before
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count();
        assert!(opaque > 1000 && opaque < before.len() / 4);
        if let Some(path) = std::env::var_os(if avatar.asset.summary.is_glb() {
            "ARIA_TEST_GLB_PNG"
        } else {
            "ARIA_TEST_VRM_PNG"
        }) {
            image::save_buffer(
                Path::new(&path),
                &before,
                avatar.image().size.x as u32,
                avatar.image().size.y as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        assert!(
            !avatar.spring_groups.is_empty(),
            "Test avatar needs secondary groups"
        );
        eprintln!(
            "Secondary motion: {} authored + {} generated groups, {} joints; Medium enabled by default",
            avatar.asset.springs.len(),
            avatar.spring_groups.len() - avatar.asset.springs.len(),
            avatar
                .spring_groups
                .iter()
                .map(|g| g.joints.len())
                .sum::<usize>()
        );
        config.vrm.lighting = aria_core::vrm::Lighting {
            enabled: true,
            directional: false,
            color: [0.5, 0.8, 1.],
            ..Default::default()
        };
        avatar
            .update(&neutral, &mut config, &mut expressions, 0.)
            .unwrap();
        let even_light = avatar.renderer.read_rgba().unwrap();
        assert_ne!(even_light, before, "light color must reach the 3D canvas");
        config.vrm.lighting.directional = true;
        config.vrm.lighting.azimuth = 90.;
        avatar
            .update(&neutral, &mut config, &mut expressions, 0.)
            .unwrap();
        assert_ne!(
            even_light,
            avatar.renderer.read_rgba().unwrap(),
            "directional and even lighting must differ"
        );
        config.vrm.lighting = Default::default();
        let head = avatar.asset.bones["head"];
        let local_forward = avatar.asset.nodes[head].world.inverse().transform_vector3(
            avatar
                .asset
                .front
                .inverse()
                .transform_vector3(glam::Vec3::Z),
        );
        for (id, axis) in [("ParamAngleX", 0), ("ParamAngleY", 1)] {
            let mut input = neutral.clone();
            input.insert(id.into(), 25.);
            avatar
                .update(&input, &mut config, &mut expressions, 1. / 60.)
                .unwrap();
            let forward = avatar
                .asset
                .front
                .transform_vector3(avatar.world[head].transform_vector3(local_forward))
                .normalize();
            assert!(
                forward[axis] > 0.25 && forward[1 - axis].abs() < 0.01,
                "Independent positive yaw/pitch: {forward:?}"
            );
        }
        let mut input = neutral.clone();
        input.insert("ParamAngleX".into(), 25.);
        input.insert("ParamAngleY".into(), 20.);
        input.insert("ParamMouthOpenY".into(), 1.);
        input.insert("ParamEyeLOpen".into(), 0.);
        assert!(
            avatar
                .update(&input, &mut config, &mut expressions, 1. / 60.)
                .unwrap()
        );
        let after = avatar.renderer.read_rgba().unwrap();
        assert!(before != after, "Tracking must change the rendered avatar");
        assert!(avatar.weights.iter().flatten().any(|w| *w > 0.));
        config.physics.enabled = true;
        config.vrm.motion.enabled = true;
        let arm = avatar.asset.bones["rightUpperArm"];
        let rest_arm = avatar.rotations[arm];
        avatar.motion.play(motion::Gesture::Wave);
        for _ in 0..60 {
            avatar
                .update(&input, &mut config, &mut expressions, 1. / 60.)
                .unwrap();
        }
        assert!(avatar.world.iter().all(|m| m.is_finite()));
        assert!(
            !rest_arm.abs_diff_eq(avatar.rotations[arm], 0.1),
            "Wave must move the arm"
        );
        assert!(
            avatar
                .spring_groups
                .iter()
                .flat_map(|g| &g.joints)
                .any(|j| !avatar.rotations[j.node]
                    .abs_diff_eq(avatar.asset.nodes[j.node].rotation, 0.001)),
            "secondary bones must actually move"
        );
        let sprung_world = avatar.world.clone();
        let sprung_rotations = avatar.rotations.clone();
        let sprung_pixels = avatar.renderer.read_rgba().unwrap();
        for group in &avatar.spring_groups {
            for j in &group.joints {
                avatar.rotations[j.node] = avatar.asset.nodes[j.node].rotation;
            }
        }
        spring::world_matrices(
            &avatar.asset.nodes,
            &avatar.asset.order,
            &avatar.rotations,
            &mut avatar.world,
        );
        avatar
            .renderer
            .render(&avatar.asset, &avatar.world, &avatar.weights, &config.vrm)
            .unwrap();
        assert_ne!(
            sprung_pixels,
            avatar.renderer.read_rgba().unwrap(),
            "spring bones must deform visible skinned artwork"
        );
        avatar.world = sprung_world;
        avatar.rotations = sprung_rotations;
        avatar
            .renderer
            .render(&avatar.asset, &avatar.world, &avatar.weights, &config.vrm)
            .unwrap();
        config.capture_pose(avatar.parameters());
        avatar
            .update(&input, &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        let frozen = avatar.renderer.read_rgba().unwrap();
        for _ in 0..30 {
            assert!(
                !avatar
                    .update(&Inputs::new(), &mut config, &mut expressions, 1. / 60.)
                    .unwrap()
            );
        }
        assert!(
            frozen == avatar.renderer.read_rgba().unwrap(),
            "Frozen image changed"
        );
        let original_world = avatar.world.clone();
        let original_weights = avatar.weights.clone();
        config = serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
        avatar.reset_motion();
        avatar
            .update(&neutral, &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        assert!(
            original_world
                .iter()
                .zip(&avatar.world)
                .all(|(a, b)| a.to_cols_array().map(f32::to_bits)
                    == b.to_cols_array().map(f32::to_bits)),
            "Saved pose must restore exact joint transforms"
        );
        assert!(
            original_weights == avatar.weights,
            "Saved pose must restore morph weights"
        );
        let reloaded = avatar.renderer.read_rgba().unwrap();
        let changed = frozen.iter().zip(&reloaded).filter(|(a, b)| a != b).count();
        // Coplanar transparent surfaces can resolve a few edge samples differently
        // on a fresh DX12 draw. Pose data above must still match bit for bit.
        assert!(
            changed < frozen.len() / 1000,
            "Saved pose changed {changed} channels after rerender"
        );
        let frozen_head = avatar.rotations[head];
        config.pose.frozen.insert("ParamAngleY".into(), -20.);
        assert!(
            avatar
                .update(&input, &mut config, &mut expressions, 1. / 60.)
                .unwrap()
        );
        assert!(
            !frozen_head.abs_diff_eq(avatar.rotations[head], 0.1),
            "Frozen motion must still allow parameter edits"
        );
        if let Some((index, _)) = avatar
            .asset
            .expressions
            .iter()
            .enumerate()
            .find(|(_, e)| !e.binds.is_empty())
        {
            let mut monitor = crate::input_monitor::InputMonitor::new(
                avatar.asset.key.clone(),
                avatar.initial_config.clone(),
                None,
                avatar.parameters(),
            );
            monitor.expressions.load_embedded(
                avatar.embedded_expressions().unwrap(),
                &monitor.saved,
                avatar.parameters(),
            );
            let id = expression_id(index);
            let shortcut = aria_core::shortcuts::Shortcut {
                ctrl: true,
                alt: true,
                key: u16::from(b'K'),
                ..Default::default()
            };
            crate::expressions_panel::ExpressionsPanel::assign(&mut monitor.saved, &id, shortcut)
                .unwrap();
            monitor.saved.global_hotkeys = true;
            assert!(monitor.hotkey_keys().iter().any(|r| r.shortcut == shortcut));
            monitor.hotkey_action(
                crate::hotkeys::Action::Expression(id.clone()),
                avatar.parameters(),
                &mut Default::default(),
            );
            for _ in 0..12 {
                avatar
                    .update(
                        &neutral,
                        &mut monitor.saved.config,
                        &mut monitor.expressions,
                        1. / 60.,
                    )
                    .unwrap();
            }
            assert!(avatar.value(&id) > 0.9);
            monitor.hotkey_action(
                crate::hotkeys::Action::Expression(id.clone()),
                avatar.parameters(),
                &mut Default::default(),
            );
            for _ in 0..12 {
                avatar
                    .update(
                        &neutral,
                        &mut monitor.saved.config,
                        &mut monitor.expressions,
                        1. / 60.,
                    )
                    .unwrap();
            }
            assert_eq!(avatar.value(&id), 0.);
            let restored: aria_core::movement::SavedRig =
                serde_json::from_slice(&serde_json::to_vec(&monitor.saved).unwrap()).unwrap();
            restored.validate(avatar.parameters()).unwrap();
            assert_eq!(restored.expression_hotkeys[&id], shortcut);
        }
        if avatar.asset.summary.is_glb() {
            let mut config = avatar.initial_config.clone();
            config.physics.enabled = false;
            config.vrm.motion.enabled = false;
            avatar.motion.stop();
            avatar
                .update(&neutral, &mut config, &mut expressions, 0.5)
                .unwrap();
            let part = &avatar.asset.parts[0];
            let points: Vec<_> = part.indices[..3]
                .iter()
                .map(|&v| avatar.renderer.projected_vertex(part.geometry, v).unwrap())
                .collect();
            let point = (points[0] + points[1] + points[2]) / 3.;
            let pin = avatar
                .pick_pin(egui::vec2(point.x, point.y))
                .expect("Pick GLB surface");
            assert!(
                matches!(avatar.resolve_pin(&pin), crate::items::Anchor::Surface { opacity, .. } if opacity > 0.)
            );
            let mut input = neutral.clone();
            input.insert("ParamAngleX".into(), 25.);
            config.vrm.bone_map.insert("head".into(), None);
            config.vrm.bone_map.insert("neck".into(), None);
            avatar
                .update(&input, &mut config, &mut expressions, 0.016)
                .unwrap();
            assert!(
                avatar.rotations[head].abs_diff_eq(avatar.asset.nodes[head].rotation, 0.001),
                "Disabled head bone must retain its authored rotation"
            );
            config.vrm.bone_map.clear();
            avatar
                .update(&input, &mut config, &mut expressions, 0.016)
                .unwrap();
            assert!(!avatar.rotations[head].abs_diff_eq(avatar.asset.nodes[head].rotation, 0.01));
            assert!(
                matches!(avatar.resolve_pin(&pin), crate::items::Anchor::Surface { scale, .. } if scale.is_finite())
            );
            let front = avatar.asset.front;
            config.vrm.reverse_forward = true;
            avatar
                .update(&input, &mut config, &mut expressions, 0.016)
                .unwrap();
            assert!(
                front
                    .transform_vector3(glam::Vec3::Z)
                    .dot(avatar.asset.front.transform_vector3(glam::Vec3::Z))
                    < -0.99
            );
        }
        println!(
            "Transparent GPU avatar, tracking, morphs, finite springs and frozen screenshots passed; {:.1} MiB GPU assets + canvas",
            avatar.atlas_mib()
        );
    }
}
