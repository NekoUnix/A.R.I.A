//! One exercise per capture, with bounded, independently selectable takes.
use super::{Capture, Proposal, percentile};
use crate::rig::{self, Inputs};
use std::collections::BTreeMap;

pub const MAX_TAKES: usize = 5;
const MAX_SAMPLES: usize = 1800;

#[derive(Clone, Debug)]
pub struct Exercise {
    pub id: String,
    pub title: String,
    pub instruction: String,
    pub inputs: Vec<String>,
}
impl Exercise {
    pub fn neutral(&self) -> bool {
        self.id == "neutral"
    }
}

// Separate directions share only the signals that they intentionally measure.
const STEPS: &[(&str, &str, &str, &str)] = &[
    (
        "yaw-left",
        "Turn your head left",
        "Turn toward your own left. Keep your chin level. Move slowly, hold a comfortable turn, then return to center.",
        "yaw",
    ),
    (
        "yaw-right",
        "Turn your head right",
        "Turn toward your own right. Keep your chin level. Move slowly, hold a comfortable turn, then return to center.",
        "yaw",
    ),
    (
        "pitch-up",
        "Lift your chin",
        "Look upward by lifting your chin. Keep your shoulders still. Hold a comfortable angle, then return to center.",
        "pitch",
    ),
    (
        "pitch-down",
        "Lower your chin",
        "Look downward by lowering your chin. Keep your shoulders still. Hold a comfortable angle, then return to center.",
        "pitch",
    ),
    (
        "roll-left",
        "Tilt toward your left shoulder",
        "Tilt your head toward your own left shoulder without turning your face. Hold comfortably, then return upright.",
        "roll",
    ),
    (
        "roll-right",
        "Tilt toward your right shoulder",
        "Tilt your head toward your own right shoulder without turning your face. Hold comfortably, then return upright.",
        "roll",
    ),
    (
        "eye-left",
        "Close your left eye",
        "Gently close your own left eye, hold briefly, then open it. Repeat slowly. If winking is difficult, close both eyes; this step measures only the left eyelid.",
        "lid-left",
    ),
    (
        "eye-right",
        "Close your right eye",
        "Gently close your own right eye, hold briefly, then open it. Repeat slowly. If winking is difficult, close both eyes; this step measures only the right eyelid.",
        "lid-right",
    ),
    (
        "mouth-open",
        "Open your mouth",
        "Open your mouth to a comfortable talking shape. Hold briefly, then relax. Repeat slowly without stretching your jaw.",
        "mouth-open lip-open",
    ),
    (
        "smile",
        "Smile",
        "Make a natural smile, hold briefly, then relax. Keep your head still and repeat comfortably.",
        "smile mouth-form",
    ),
    (
        "frown",
        "Frown",
        "Turn your mouth corners down, hold briefly, then relax. Keep your head still. Skip if the tracker does not distinguish this expression.",
        "frown mouth-form",
    ),
    (
        "brows-up",
        "Raise your eyebrows",
        "Raise your eyebrows, hold briefly, then relax. Keep your head still and avoid opening your mouth.",
        "brow-up brow-left brow-right",
    ),
    (
        "brows-down",
        "Lower your eyebrows",
        "Gently lower your eyebrows, hold briefly, then relax. Keep your head still.",
        "brow-down brow-left brow-right",
    ),
    (
        "mouth-left",
        "Move your mouth left",
        "Move your lips toward your own left without turning your head. Hold briefly, then relax.",
        "mouth-x mouth-left",
    ),
    (
        "mouth-right",
        "Move your mouth right",
        "Move your lips toward your own right without turning your head. Hold briefly, then relax.",
        "mouth-x mouth-right",
    ),
    (
        "pucker",
        "Pucker your lips",
        "Bring your lips together into a gentle kiss shape. Hold briefly, then relax. Repeat slowly.",
        "pucker",
    ),
    (
        "funnel",
        "Make an O shape",
        "Round your open lips into an O shape. Hold briefly, then relax. Keep your head still.",
        "funnel",
    ),
    (
        "shrug",
        "Raise your lips",
        "Raise your upper lip toward your nose, hold briefly, then relax. Skip if this input is not useful for your avatar.",
        "shrug",
    ),
    (
        "press",
        "Press your lips together",
        "Gently press your lips together, hold briefly, then relax. Keep your jaw comfortable.",
        "press lip-open",
    ),
    (
        "cheek-puff",
        "Puff your cheeks",
        "Gently puff your cheeks, hold briefly, then relax. Repeat comfortably. Skip if the tracker does not provide this signal.",
        "puff",
    ),
    (
        "tongue",
        "Show your tongue",
        "Briefly stick out your tongue, then relax. Keep your head still. Skip if your tracker cannot detect it.",
        "tongue",
    ),
    (
        "gaze-left",
        "Look left with your eyes",
        "Keep your head facing forward. Look toward your own left with only your eyes, hold briefly, then look back at the camera.",
        "gaze-x",
    ),
    (
        "gaze-right",
        "Look right with your eyes",
        "Keep your head facing forward. Look toward your own right with only your eyes, hold briefly, then look back at the camera.",
        "gaze-x",
    ),
    (
        "gaze-up",
        "Look up with your eyes",
        "Keep your head facing forward. Look upward with only your eyes, hold briefly, then look back at the camera.",
        "gaze-y",
    ),
    (
        "gaze-down",
        "Look down with your eyes",
        "Keep your head facing forward. Look downward with only your eyes, hold briefly, then look back at the camera.",
        "gaze-y",
    ),
];

fn channel(name: &str) -> Option<&'static str> {
    Some(match name {
        "FaceAngleX" | "ParamAngleX" | "ParamBodyAngleX" => "yaw",
        "FaceAngleY" | "ParamAngleY" => "pitch",
        "FaceAngleZ" | "ParamAngleZ" => "roll",
        "EyeOpenLeft" | "ParamEyeLOpen" | "ARKit:eyeblinkleft" => "lid-left",
        "EyeOpenRight" | "ParamEyeROpen" | "ARKit:eyeblinkright" => "lid-right",
        "MouthOpen" | "JawOpen" | "ParamMouthOpenY" | "ARKit:jawopen" => "mouth-open",
        "MouthSmile" | "ARKit:mouthsmileleft" | "ARKit:mouthsmileright" => "smile",
        "ParamMouthForm" => "mouth-form",
        "ARKit:mouthfrownleft" | "ARKit:mouthfrownright" => "frown",
        "Brows"
        | "BrowInnerUp"
        | "ARKit:browinnerup"
        | "ARKit:browouterupleft"
        | "ARKit:browouterupright" => "brow-up",
        "BrowLeftY" | "ParamBrowLY" => "brow-left",
        "BrowRightY" | "ParamBrowRY" => "brow-right",
        "ARKit:browdownleft" | "ARKit:browdownright" => "brow-down",
        "MouthX" => "mouth-x",
        "ARKit:mouthleft" => "mouth-left",
        "ARKit:mouthright" => "mouth-right",
        "MouthPucker" | "ARKit:mouthpucker" => "pucker",
        "MouthFunnel" | "ARKit:mouthfunnel" => "funnel",
        "MouthShrug" | "ARKit:mouthshrugupper" | "ARKit:mouthshruglower" => "shrug",
        "MouthPressLipOpen" => "lip-open",
        "ARKit:mouthpressleft" | "ARKit:mouthpressright" => "press",
        "CheekPuff" | "ARKit:cheekpuff" => "puff",
        "TongueOut" | "ARKit:tongueout" => "tongue",
        "EyeLeftX"
        | "EyeRightX"
        | "ParamEyeBallX"
        | "ARKit:eyelookinleft"
        | "ARKit:eyelookinright"
        | "ARKit:eyelookoutleft"
        | "ARKit:eyelookoutright" => "gaze-x",
        "EyeLeftY"
        | "EyeRightY"
        | "ParamEyeBallY"
        | "ARKit:eyelookupleft"
        | "ARKit:eyelookupright"
        | "ARKit:eyelookdownleft"
        | "ARKit:eyelookdownright" => "gaze-y",
        _ => return None,
    })
}

#[derive(Default, Clone)]
pub struct Take {
    pub id: u64,
    pub elapsed: f64,
    pub samples: usize,
    values: BTreeMap<String, Vec<f32>>,
    quality: Quality,
    last_sequence: Option<u64>,
    last_time: Option<f64>,
}
#[derive(Clone, Copy, Default)]
pub struct Quality {
    pub usable: usize,
    pub measured: usize,
    score: f32,
}
impl Quality {
    pub fn label(self) -> &'static str {
        if self.usable == 0 {
            "Needs another take"
        } else if self.usable == self.measured {
            "Clear signal"
        } else {
            "Some inputs need review"
        }
    }
}

#[derive(Default)]
pub struct Session {
    pub exercises: Vec<Exercise>,
    takes: Vec<Vec<Take>>,
    selected: Vec<Option<u64>>,
    pub active: Option<(usize, Take)>,
    names: Vec<String>,
    next_id: u64,
}
impl Session {
    pub fn new(names: impl IntoIterator<Item = String>, used: Option<&[String]>) -> Self {
        let capture = Capture::new(names);
        let names = capture.names;
        let mut exercises = vec![Exercise {
            id: "neutral".into(),
            title: "Find your relaxed resting pose".into(),
            instruction: "Face the camera at your usual streaming distance. Rest your face naturally with eyes open and lips gently closed. Hold still during this take.".into(),
            inputs: names.clone(),
        }];
        for &(id, title, instruction, channels) in STEPS {
            let matches = |name: &str| {
                channel(name).is_some_and(|c| channels.split_whitespace().any(|v| v == c))
            };
            if used.unwrap_or(&names).iter().any(|n| matches(n)) {
                exercises.push(Exercise {
                    id: id.into(),
                    title: title.into(),
                    instruction: instruction.into(),
                    inputs: names.iter().filter(|n| matches(n)).cloned().collect(),
                });
            }
        }
        for name in &names {
            if name.starts_with("ARKit:")
                && channel(name).is_none()
                && used.is_none_or(|used| used.contains(name))
            {
                exercises.push(Exercise {
                    id: name.clone(), title: format!("Extra expression: {}", &name[6..]),
                    instruction: format!("Activate only the expression assigned to {name}, hold briefly, then relax. Watch its live value below. Skip if this control is unavailable or its meaning is unclear; it stays adjustable in Inputs."),
                    inputs: vec![name.clone()],
                });
            }
        }
        let count = exercises.len();
        Self {
            exercises,
            takes: vec![vec![]; count],
            selected: vec![None; count],
            names,
            ..Self::default()
        }
    }
    pub fn takes(&self, index: usize) -> &[Take] {
        &self.takes[index]
    }
    pub fn selected(&self, index: usize) -> Option<u64> {
        self.selected[index]
    }
    fn chosen(&self, index: usize) -> Option<&Take> {
        self.takes[index]
            .iter()
            .find(|t| Some(t.id) == self.selected[index])
    }
    pub fn begin(&mut self, index: usize) -> bool {
        if index >= self.exercises.len() || self.takes[index].len() >= MAX_TAKES {
            return false;
        }
        self.next_id += 1;
        self.active = Some((
            index,
            Take {
                id: self.next_id,
                ..Take::default()
            },
        ));
        true
    }
    pub fn cancel_take(&mut self) {
        self.active = None;
    }
    /// Completed captures stay on the same page. Only an explicit UI action advances.
    pub fn sample(
        &mut self,
        sequence: u64,
        now: f64,
        live: bool,
        inputs: &Inputs,
        seconds: f64,
    ) -> bool {
        let Some((index, take)) = &mut self.active else {
            return false;
        };
        if !live || !now.is_finite() {
            take.last_time = None;
            return false;
        }
        if take.last_sequence == Some(sequence) {
            return false;
        }
        take.last_sequence = Some(sequence);
        if let Some(last) = take.last_time {
            let dt = now - last;
            if (0.0..=0.25).contains(&dt) {
                take.elapsed += dt;
            }
        }
        take.last_time = Some(now);
        take.samples += 1;
        for name in &self.exercises[*index].inputs {
            if let Some(&v) = inputs
                .get(name)
                .filter(|v| v.is_finite() && v.abs() <= 10000.0)
            {
                let samples = take.values.entry(name.clone()).or_default();
                if samples.len() < MAX_SAMPLES {
                    samples.push(v);
                }
            }
        }
        if take.elapsed < seconds.clamp(3.0, 30.0) || take.samples < 30 {
            return false;
        }
        let (index, mut take) = self.active.take().unwrap();
        take.quality = self.calculate_quality(index, &take);
        let id = take.id;
        self.takes[index].push(take);
        if self.selected[index].is_none() {
            self.select(index, Some(id));
        }
        true
    }
    pub fn select(&mut self, index: usize, id: Option<u64>) {
        if id.is_some_and(|id| !self.takes[index].iter().any(|t| t.id == id)) {
            return;
        }
        if index == 0 && self.selected[0] != id {
            // A different baseline invalidates only this draft's movement takes.
            for i in 1..self.exercises.len() {
                self.takes[i].clear();
                self.selected[i] = None;
            }
            self.active = None;
        }
        self.selected[index] = id;
    }
    pub fn remove(&mut self, index: usize, id: u64) {
        if self.selected[index] == Some(id) {
            self.select(index, None);
        }
        self.takes[index].retain(|t| t.id != id);
    }
    pub fn quality(&self, _index: usize, take: &Take) -> Quality {
        take.quality
    }
    fn calculate_quality(&self, index: usize, take: &Take) -> Quality {
        let mut quality = Quality::default();
        for name in &self.exercises[index].inputs {
            let Some(values) = take.values.get(name).filter(|v| v.len() >= 30) else {
                continue;
            };
            quality.measured += 1;
            let (lo, hi) = rig::input_range(name);
            let span = (hi - lo).max(1e-5);
            if index == 0 {
                let noise = (percentile(values, 0.95) - percentile(values, 0.05)) / span;
                if noise <= 0.12 {
                    quality.usable += 1;
                    quality.score += 1.0 - noise;
                }
            } else if let Some(neutral) = self
                .chosen(0)
                .and_then(|t| t.values.get(name))
                .filter(|v| v.len() >= 30)
            {
                let center = percentile(neutral, 0.5);
                let noise = percentile(neutral, 0.95) - percentile(neutral, 0.05);
                let distance = (percentile(values, 0.98) - center)
                    .abs()
                    .max((percentile(values, 0.02) - center).abs());
                if noise <= span * 0.12 && distance >= (span * 0.04).max(noise * 3.0) {
                    quality.usable += 1;
                    // Cap the range reward: a larger extreme is not inherently a better take.
                    quality.score += 1.0 + (distance / span).min(0.25);
                }
            }
        }
        quality.score /= self.exercises[index].inputs.len().max(1) as f32;
        quality
    }
    pub fn recommended(&self, index: usize) -> Option<u64> {
        self.takes[index]
            .iter()
            .filter(|t| self.quality(index, t).usable > 0)
            .max_by(|a, b| {
                self.quality(index, a)
                    .score
                    .total_cmp(&self.quality(index, b).score)
            })
            .map(|t| t.id)
    }
    pub fn can_continue(&self, index: usize) -> bool {
        self.chosen(index)
            .is_some_and(|t| self.quality(index, t).usable > 0)
    }
    pub fn proposals(&self) -> Vec<Proposal> {
        let mut combined = Capture::new(self.names.clone());
        if let Some(take) = self.chosen(0) {
            combined.neutral = take.values.clone();
        }
        for index in 1..self.exercises.len() {
            if let Some(take) = self.chosen(index) {
                for (name, values) in &take.values {
                    combined
                        .movement
                        .entry(name.clone())
                        .or_default()
                        .extend_from_slice(values);
                }
            }
        }
        combined.proposals()
    }
    pub fn captured(&self) -> usize {
        self.selected.iter().filter(|t| t.is_some()).count()
    }
    pub fn known_names(&self) -> &[String] {
        &self.names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(s: &mut Session, index: usize, low: f32, high: f32, noisy: bool) -> u64 {
        assert!(s.begin(index));
        for n in 0..=190 {
            let value = if noisy || n > 45 && n < 145 {
                high
            } else {
                low
            };
            let values = Inputs::from([
                (
                    "FaceAngleX".into(),
                    if noisy && n % 2 == 0 { low } else { value },
                ),
                ("FaceAngleY".into(), 900.0),
            ]);
            if s.sample(n, n as f64 / 60.0, true, &values, 3.0) {
                break;
            }
        }
        s.takes(index).last().unwrap().id
    }
    fn yaw_session() -> Session {
        Session::new(
            ["FaceAngleX".into(), "FaceAngleY".into()],
            Some(&["FaceAngleX".into()]),
        )
    }
    #[test]
    fn exercises_capture_only_their_signals_and_keep_asymmetric_sides() {
        let mut s = yaw_session();
        assert_eq!(
            s.exercises
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>(),
            ["neutral", "yaw-left", "yaw-right"]
        );
        record(&mut s, 0, 4.0, 4.0, false);
        record(&mut s, 1, 4.0, -18.0, false);
        record(&mut s, 2, 4.0, 46.0, false);
        assert!(!s.takes(1)[0].values.contains_key("FaceAngleY"));
        let rows = s.proposals();
        let range = rows
            .iter()
            .find(|p| p.input == "FaceAngleX")
            .unwrap()
            .range
            .as_ref()
            .unwrap();
        assert_eq!((range.low, range.neutral, range.high), (-18.0, 4.0, 46.0));
        assert!(
            rows.iter()
                .find(|p| p.input == "FaceAngleY")
                .unwrap()
                .range
                .is_none()
        );
    }
    #[test]
    fn retries_remain_selectable_and_only_selected_takes_affect_ranges() {
        let mut s = yaw_session();
        record(&mut s, 0, 0.0, 0.0, false);
        let weak = record(&mut s, 1, 0.0, -0.1, false);
        let clear = record(&mut s, 1, 0.0, -18.0, false);
        assert_eq!(s.selected(1), Some(weak));
        assert_eq!(s.recommended(1), Some(clear));
        record(&mut s, 2, 0.0, 42.0, false);
        assert!(s.proposals()[0].range.is_none());
        s.select(1, Some(clear));
        assert_eq!(s.proposals()[0].range.as_ref().unwrap().low, -18.0);
        s.select(1, None);
        assert!(s.proposals()[0].range.is_none());
        assert!(s.selected(2).is_some());
        s.select(1, Some(clear));
        for _ in 0..3 {
            record(&mut s, 1, 0.0, -20.0, false);
        }
        assert!(!s.begin(1));
        s.remove(1, weak);
        assert!(s.begin(1));
    }
    #[test]
    fn neutral_replacement_invalidates_movement_but_retry_does_not() {
        let mut s = yaw_session();
        let baseline = record(&mut s, 0, 0.0, 0.0, false);
        record(&mut s, 1, 0.0, -25.0, false);
        let replacement = record(&mut s, 0, 3.0, 3.0, false);
        assert_eq!(s.selected(0), Some(baseline));
        assert_eq!(s.takes(1).len(), 1);
        s.select(0, Some(replacement));
        assert!(s.takes(1).is_empty());
        assert!(s.selected(1).is_none());
    }
    #[test]
    fn duplicate_packets_face_loss_and_clock_gaps_do_not_finish_a_take() {
        let mut s = yaw_session();
        s.begin(0);
        let values = Inputs::from([("FaceAngleX".into(), 0.0)]);
        s.sample(1, 0.0, true, &values, 12.0);
        s.sample(1, 10.0, true, &values, 12.0);
        s.sample(2, 11.0, false, &values, 12.0);
        s.sample(3, 30.0, true, &values, 12.0);
        s.sample(4, 30.1, true, &values, 12.0);
        let t = &s.active.as_ref().unwrap().1;
        assert_eq!(t.samples, 3);
        assert!((t.elapsed - 0.1).abs() < 1e-6);
        assert!(s.takes(0).is_empty());
    }
    #[test]
    fn noisy_neutral_is_not_recommended_and_frames_are_bounded() {
        let mut s = Session::new(["FaceAngleX".into()], None);
        record(&mut s, 0, -20.0, 20.0, true);
        assert!(s.recommended(0).is_none());
        assert!(!s.can_continue(0));
        s.begin(0);
        for n in 0..5000 {
            s.sample(
                n,
                n as f64 / 1000.0,
                true,
                &Inputs::from([("FaceAngleX".into(), 0.0)]),
                30.0,
            );
        }
        assert_eq!(
            s.active.as_ref().unwrap().1.values["FaceAngleX"].len(),
            MAX_SAMPLES
        );
    }
    #[test]
    fn extra_arkit_inputs_get_individual_exercises_and_controllers_do_not() {
        let s = Session::new(["ARKit:nosesneerleft".into(), "NP_LStickX".into()], None);
        assert_eq!(s.exercises.len(), 2);
        assert_eq!(s.exercises[1].inputs, ["ARKit:nosesneerleft"]);
        assert!(
            s.known_names()
                .iter()
                .all(|n| super::super::group(n).is_some())
        );
    }
}
