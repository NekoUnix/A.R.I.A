use super::{Output, TrackingFrame};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct Runtime {
    pub(super) seconds: f64,
    last_face: Option<TrackingFrame>,
    lost_seconds: f32,
    channels: Vec<Channel>,
    pub faults: usize,
}
#[derive(Clone, Debug, Default)]
struct Channel {
    delay: VecDeque<(f64, f32)>,
    smooth: Option<f32>,
    step: Option<usize>,
    held_until: f64,
}
impl Runtime {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub(super) fn advance(&mut self, frame: Option<&TrackingFrame>, dt: f32, count: usize) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        self.seconds += f64::from(dt);
        self.faults = 0;
        if let Some(frame) = frame.filter(|f| f.face_found && f.is_finite()) {
            self.last_face = Some(frame.clone());
            self.lost_seconds = 0.0;
        } else {
            self.lost_seconds += dt;
        }
        self.channels.resize_with(count, Channel::default);
    }
    pub(super) fn face(&self, milliseconds: f32) -> Option<&TrackingFrame> {
        (self.lost_seconds * 1000.0 <= milliseconds)
            .then_some(self.last_face.as_ref())
            .flatten()
    }
    pub(super) fn modify(
        &mut self,
        index: usize,
        output: &Output,
        value: Option<f32>,
        dt: f32,
    ) -> f32 {
        let channel = &mut self.channels[index];
        let Some(mut value) = value else {
            *channel = Channel::default();
            self.faults += 1;
            return output.default;
        };
        value = value.clamp(output.min, output.max);
        if output.delay_on && output.delay_ms > 0.0 {
            // At most 240 history samples/sec and 1202 samples for a five-second delay.
            if channel
                .delay
                .back()
                .is_none_or(|(at, _)| self.seconds - at >= 1.0 / 240.0)
            {
                channel.delay.push_back((self.seconds, value));
            }
            let due = self.seconds - f64::from(output.delay_ms) / 1000.0;
            while channel.delay.len() > 1 && channel.delay[1].0 <= due {
                channel.delay.pop_front();
            }
            while channel.delay.len() > 1202 {
                channel.delay.pop_front();
            }
            value = channel
                .delay
                .front()
                .filter(|(at, _)| *at <= due)
                .map_or(output.default, |(_, v)| *v);
        } else {
            channel.delay.clear();
        }
        if output.smooth_on {
            let prior = channel.smooth.get_or_insert(output.default);
            let dt = if dt.is_finite() {
                dt.clamp(0.0, 0.25)
            } else {
                0.0
            };
            // Equivalent to lerp(previous,target,1-smooth) at 60 Hz; time-adjusted elsewhere.
            let alpha = 1.0 - output.smoothing.powf(dt * 60.0);
            *prior += (value - *prior) * alpha;
            value = *prior;
        } else {
            channel.smooth = None;
        }
        if output.step_on {
            let steps = &output.steps;
            let candidate = steps.iter().rposition(|s| value >= s.trigger);
            if self.seconds >= channel.held_until {
                let current = channel.step.and_then(|i| steps.get(i));
                let moving_up = candidate > channel.step;
                let moving_down = current.is_some_and(|s| value < s.threshold);
                if moving_up || moving_down {
                    channel.step = candidate;
                    channel.held_until = self.seconds
                        + channel
                            .step
                            .map_or(0.0, |i| f64::from(steps[i].hold_ms) / 1000.0);
                }
            }
            value = channel
                .step
                .and_then(|i| steps.get(i))
                .map_or(output.min, |s| s.target);
        } else {
            channel.step = None;
            channel.held_until = 0.0;
        }
        if value.is_finite() {
            value.clamp(output.min, output.max)
        } else {
            output.default
        }
    }
}
