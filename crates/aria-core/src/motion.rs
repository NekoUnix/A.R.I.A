//! Bounded Cubism motion3 parameter curves, independently implemented from the
//! public format: https://github.com/Live2D/CubismSpecs/blob/master/FileFormats/motion3.json.md
use crate::rig::RigParameter;
use anyhow::{Result, ensure};
use serde::Deserialize;

#[derive(Clone, Debug)]
enum Segment {
    Linear([f32; 2], [f32; 2]),
    Bezier([[f32; 2]; 4]),
    Step([f32; 2], [f32; 2], bool),
}
impl Segment {
    fn end(&self) -> f32 {
        match self {
            Self::Linear(_, b) | Self::Step(_, b, _) => b[0],
            Self::Bezier(p) => p[3][0],
        }
    }
    fn value(&self, time: f32) -> f32 {
        match self {
            Self::Linear(a, b) => {
                a[1] + (b[1] - a[1]) * ((time - a[0]) / (b[0] - a[0])).clamp(0., 1.)
            }
            Self::Step(a, b, inverse) => {
                if *inverse {
                    b[1]
                } else {
                    a[1]
                }
            }
            Self::Bezier(p) => {
                let cubic = |t: f32, axis: usize| {
                    let u = 1. - t;
                    u * u * u * p[0][axis]
                        + 3. * u * u * t * p[1][axis]
                        + 3. * u * t * t * p[2][axis]
                        + t * t * t * p[3][axis]
                };
                let (mut low, mut high) = (0., 1.);
                for _ in 0..22 {
                    let middle = (low + high) * 0.5;
                    if cubic(middle, 0) < time {
                        low = middle
                    } else {
                        high = middle
                    }
                }
                cubic((low + high) * 0.5, 1)
            }
        }
    }
}
#[derive(Clone, Debug)]
struct Curve {
    target: String,
    id: String,
    first: [f32; 2],
    segments: Vec<Segment>,
    fade_in: f32,
    fade_out: f32,
}
#[derive(Clone, Debug)]
pub struct Motion {
    pub duration: f32,
    pub looping: bool,
    curves: Vec<Curve>,
    pub notes: Vec<String>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Event {
    pub time: f32,
    pub value: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Document {
    version: u32,
    meta: Meta,
    curves: Vec<Row>,
    #[serde(default)]
    user_data: Vec<Event>,
    #[serde(default = "default_fade")]
    fade_in_time: f32,
    #[serde(default = "default_fade")]
    fade_out_time: f32,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Meta {
    duration: f32,
    #[serde(default, rename = "Loop")]
    looping: bool,
    #[serde(default)]
    user_data_count: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Row {
    target: String,
    id: String,
    segments: Vec<f32>,
    #[serde(default = "default_fade")]
    fade_in_time: f32,
    #[serde(default = "default_fade")]
    fade_out_time: f32,
}
fn default_fade() -> f32 {
    0.5
}
impl Motion {
    pub fn load(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= crate::asset_limits::MODEL_JSON,
            "Motion exceeds 20 MiB"
        );
        let d: Document = serde_json::from_slice(bytes)?;
        ensure!(
            d.version == 3
                && d.meta.duration.is_finite()
                && (0.001..=3600.).contains(&d.meta.duration)
                && d.curves.len() <= 4096,
            "Invalid motion version/duration/curve count"
        );
        let mut motion = Self {
            duration: d.meta.duration,
            looping: d.meta.looping,
            curves: Vec::new(),
            notes: Vec::new(),
            events: d.user_data,
        };
        ensure!(
            motion.events.len() <= 4096
                && motion.events.iter().all(|e| e.time.is_finite()
                    && (0.0..=motion.duration).contains(&e.time)
                    && e.value.len() <= 4096),
            "Invalid motion events"
        );
        ensure!(
            [d.fade_in_time, d.fade_out_time]
                .iter()
                .all(|v| v.is_finite() && (0.0..=60.).contains(v)),
            "Invalid motion fade"
        );
        let mut total = 0;
        for r in d.curves {
            ensure!(
                ["Parameter", "PartOpacity", "Model"].contains(&r.target.as_str()),
                "Unknown motion target {}",
                r.target
            );
            if r.target == "Model" {
                ensure!(
                    ["EyeBlink", "LipSync", "Opacity"].contains(&r.id.as_str()),
                    "Unknown model motion track {}",
                    r.id
                );
            }
            total += r.segments.len();
            ensure!(
                total <= 1_000_000
                    && r.segments.len() >= 2
                    && r.segments.iter().all(|v| v.is_finite() && v.abs() <= 1e6),
                "Invalid or oversized motion curve"
            );
            ensure!(
                !r.id.is_empty()
                    && r.id.len() <= 256
                    && [r.fade_in_time, r.fade_out_time]
                        .iter()
                        .all(|v| v.is_finite() && (-1.0..=60.).contains(v)),
                "Invalid motion identity/fade"
            );
            let first = [r.segments[0], r.segments[1]];
            ensure!(first[0] >= 0., "Negative motion time");
            let mut last = first;
            let mut segments = Vec::new();
            let mut at = 2;
            while at < r.segments.len() {
                let kind = r.segments[at];
                at += 1;
                ensure!(
                    [0., 1., 2., 3.].contains(&kind),
                    "Unknown motion segment type"
                );
                let count = if kind == 1. { 6 } else { 2 };
                ensure!(at + count <= r.segments.len(), "Incomplete motion segment");
                let end = [r.segments[at + count - 2], r.segments[at + count - 1]];
                ensure!(
                    end[0] > last[0] && end[0] <= motion.duration + 0.1,
                    "Non-increasing/out-of-duration motion time"
                );
                let segment = match kind as u8 {
                    0 => Segment::Linear(last, end),
                    1 => {
                        let a = [r.segments[at], r.segments[at + 1]];
                        let b = [r.segments[at + 2], r.segments[at + 3]];
                        ensure!(
                            last[0] <= a[0] && a[0] <= b[0] && b[0] <= end[0],
                            "Motion Bezier time controls must be monotonic"
                        );
                        Segment::Bezier([last, a, b, end])
                    }
                    _ => Segment::Step(last, end, kind == 3.),
                };
                at += count;
                segments.push(segment);
                last = end;
            }
            motion.curves.push(Curve {
                target: r.target,
                id: r.id,
                first,
                segments,
                fade_in: if r.fade_in_time < 0. {
                    d.fade_in_time
                } else {
                    r.fade_in_time
                },
                fade_out: if r.fade_out_time < 0. {
                    d.fade_out_time
                } else {
                    r.fade_out_time
                },
            });
        }
        ensure!(
            d.meta.user_data_count == 0 || d.meta.user_data_count == motion.events.len(),
            "Motion event count mismatch"
        );
        Ok(motion)
    }
    pub fn apply(&self, parameters: &mut [RigParameter], elapsed: f32, looping: bool, hold: bool) {
        self.apply_all(
            parameters,
            &mut [],
            &mut std::collections::BTreeMap::new(),
            elapsed,
            looping,
            hold,
        );
    }
    pub fn apply_all(
        &self,
        parameters: &mut [RigParameter],
        parts: &mut [RigParameter],
        model: &mut std::collections::BTreeMap<String, f32>,
        elapsed: f32,
        looping: bool,
        hold: bool,
    ) {
        if !elapsed.is_finite() {
            return;
        }
        let elapsed = elapsed.max(0.);
        let time = if looping {
            elapsed.rem_euclid(self.duration)
        } else {
            elapsed.min(self.duration)
        };
        let ease = |t: f32| 0.5 - 0.5 * (std::f32::consts::PI * t.clamp(0., 1.)).cos();
        for c in &self.curves {
            let value = if time <= c.first[0] {
                c.first[1]
            } else if let Some(s) = c.segments.iter().find(|s| time < s.end()) {
                s.value(time)
            } else {
                c.segments.last().map_or(c.first[1], |s| match s {
                    Segment::Linear(_, b) | Segment::Step(_, b, _) => b[1],
                    Segment::Bezier(p) => p[3][1],
                })
            };
            let fade_in = if c.fade_in == 0. {
                1.
            } else {
                ease(elapsed / c.fade_in)
            };
            let fade_out = if looping || hold || c.fade_out == 0. {
                1.
            } else {
                ease((self.duration - elapsed) / c.fade_out)
            };
            let weight = fade_in * fade_out;
            if c.target == "Model" {
                let base = if c.id == "LipSync" { 0. } else { 1. };
                model.insert(c.id.clone(), base + (value - base) * weight);
            } else {
                let list = if c.target == "PartOpacity" {
                    &mut *parts
                } else {
                    &mut *parameters
                };
                if let Some(p) = list.iter_mut().find(|p| p.id == c.id) {
                    p.value = (p.value + (value - p.value) * weight).clamp(p.min, p.max);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn part_model_tracks_and_events() {
        let m=Motion::load(br#"{"Version":3,"Meta":{"Duration":1,"UserDataCount":1},"UserData":[{"Time":0.5,"Value":"impact"}],"Curves":[{"Target":"PartOpacity","Id":"Arm","FadeInTime":0,"FadeOutTime":0,"Segments":[0,1,0,1,0]},{"Target":"Model","Id":"Opacity","FadeInTime":0,"FadeOutTime":0,"Segments":[0,1,0,0,1,0]}]}"#);
        assert!(m.is_err(), "Malformed non-increasing curve should fail");
        let m=Motion::load(br#"{"Version":3,"Meta":{"Duration":1},"Curves":[{"Target":"PartOpacity","Id":"Arm","FadeInTime":0,"FadeOutTime":0,"Segments":[0,1,0,1,0]},{"Target":"Model","Id":"Opacity","FadeInTime":0,"FadeOutTime":0,"Segments":[0,1,0,1,0]}]}"#).unwrap();
        let mut parts = [RigParameter {
            id: "Arm".into(),
            min: 0.,
            max: 1.,
            default: 1.,
            value: 1.,
        }];
        let mut model = std::collections::BTreeMap::new();
        m.apply_all(&mut [], &mut parts, &mut model, 0.5, false, true);
        assert_eq!(parts[0].value, 0.5);
        assert_eq!(model["Opacity"], 0.5);
        assert!(m.notes.is_empty());
    }
    #[test]
    fn linear_bezier_steps_and_frozen_endpoints() {
        let motion=Motion::load(br#"{"Version":3,"Meta":{"Duration":4},"Curves":[{"Target":"Parameter","Id":"P","FadeInTime":0,"FadeOutTime":0,"Segments":[0,0,0,1,1,1,1.3,1,1.7,0,2,0,2,3,1,3,4,0]}]}"#).unwrap();
        let mut p = [RigParameter {
            id: "P".into(),
            min: 0.,
            max: 1.,
            default: 0.,
            value: 0.,
        }];
        for (t, expected) in [(0.5, 0.5), (1.5, 0.5), (2.5, 0.), (3.5, 0.), (4., 0.)] {
            motion.apply(&mut p, t, false, true);
            assert!((p[0].value - expected).abs() < 0.001);
        }
        assert!(Motion::load(br#"{"Version":3,"Meta":{"Duration":1},"Curves":[{"Target":"Parameter","Id":"P","Segments":[0,0,1,1]}]}"#).is_err());
    }
}
