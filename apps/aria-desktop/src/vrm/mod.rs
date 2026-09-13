//! Primary VRM avatars, independent of Cubism Core and of static 3D throw props.
pub mod asset;
#[cfg(test)]
mod fixtures;
pub mod motion;
pub mod panel;
mod pins;
mod render;
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
    weights: Vec<Vec<f32>>,
    expression_weights: Vec<f32>,
    last_values: Vec<f32>,
    last_settings: Option<aria_core::vrm::Settings>,
    last_frozen: Option<aria_core::vrm::Pose>,
    last_physics: Option<aria_core::physics::PhysicsSettings>,
    last_pose: PoseMode,
    clock: f32,
    pub motion: motion::Player,
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
            parameters.push(RigParameter {
                id,
                min: 0.,
                max: 1.,
                default: 0.,
                value: 0.,
            });
        }
        let mut initial_config = RigConfig::from_parameters(&parameters);
        // Map additional ARKit shapes only when the file names them explicitly.
        // Mouth-open, eyelids and gaze use the standard VRM presets below.
        for (i, e) in asset.expressions.iter().enumerate() {
            let key = e.name.to_ascii_lowercase();
            if (e.preset.is_empty() || e.preset == "unknown")
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
            asset,
            initial_config,
            labels,
            parameters,
            renderer,
            rotations,
            world,
            springs: Default::default(),
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
        if config.pose.mode != self.last_pose {
            if !frozen {
                self.springs.reset();
            }
            self.last_pose = config.pose.mode;
        }
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
            && (config.physics.enabled && !self.asset.springs.is_empty()
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
            for group in &self.asset.springs {
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
                &self.asset.springs,
                &mut self.rotations,
                &mut self.world,
                &config.physics,
                dt,
                false,
            );
            for group in &self.asset.springs {
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
                if w > 0. {
                    for b in &e.binds {
                        if b.node.is_none_or(|n| n == g.node) && b.mesh.is_none_or(|m| m == g.mesh)
                        {
                            weights[b.index] += b.weight * w;
                        }
                    }
                }
            }
            for w in weights {
                *w = w.clamp(0., 1.);
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
        let asset = asset::load(Path::new(&path), &|message| {
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
        if let Some(path) = std::env::var_os("ARIA_TEST_VRM_PNG") {
            image::save_buffer(
                Path::new(&path),
                &before,
                avatar.image().size.x as u32,
                avatar.image().size.y as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
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
        println!(
            "Transparent GPU avatar, tracking, morphs, finite springs and frozen screenshots passed; {:.1} MiB GPU assets + canvas",
            avatar.atlas_mib()
        );
    }
}
