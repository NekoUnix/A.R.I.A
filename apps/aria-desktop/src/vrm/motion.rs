//! Bounded additive humanoid gestures in the normalized avatar-facing axes.
use aria_core::vrm::Motion;
use glam::Vec3;
use std::f32::consts::TAU;

pub const BONES: [&str; 11] = [
    "spine",
    "chest",
    "head",
    "leftUpperArm",
    "rightUpperArm",
    "leftLowerArm",
    "rightLowerArm",
    "leftHand",
    "rightHand",
    "neck",
    "upperChest",
];
type Pose = [Vec3; 11];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Wave,
    Nod,
    Shake,
    Bow,
    Cheer,
    Stretch,
}
impl Gesture {
    pub const ALL: [Self; 6] = [
        Self::Wave,
        Self::Nod,
        Self::Shake,
        Self::Bow,
        Self::Cheer,
        Self::Stretch,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Wave => "Wave",
            Self::Nod => "Nod",
            Self::Shake => "Shake head",
            Self::Bow => "Bow",
            Self::Cheer => "Cheer",
            Self::Stretch => "Stretch",
        }
    }
    fn duration(self) -> f32 {
        match self {
            Self::Wave => 3.6,
            Self::Nod | Self::Shake => 2.4,
            Self::Bow => 3.,
            Self::Cheer | Self::Stretch => 4.,
        }
    }
}
#[derive(Default)]
pub struct Player {
    pub current: Option<Gesture>,
    elapsed: f32,
    idle_phase: f32,
    blend: f32,
    from: Pose,
    last: Pose,
    settling: bool,
}
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0., 1.);
    x * x * (3. - 2. * x)
}
impl Player {
    pub fn play(&mut self, gesture: Gesture) {
        self.from = self.last;
        self.current = Some(gesture);
        self.elapsed = 0.;
        self.blend = 0.;
        self.settling = true;
    }
    pub fn stop(&mut self) {
        self.from = self.last;
        self.current = None;
        self.blend = 0.;
        self.settling = true;
    }
    pub fn active(&self) -> bool {
        self.current.is_some() || self.settling
    }
    pub fn update(&mut self, dt: f32, settings: &Motion) -> Pose {
        let dt = if dt.is_finite() {
            dt.clamp(0., 0.1)
        } else {
            0.
        };
        self.idle_phase = (self.idle_phase + dt * settings.speed).rem_euclid(1000. * TAU);
        self.elapsed += dt * settings.gesture_speed;
        self.blend = (self.blend + dt / 0.3).min(1.);
        let mut target = [Vec3::ZERO; 11];
        if let Some(gesture) = self.current {
            let duration = gesture.duration();
            if self.elapsed >= duration && !settings.gesture_loop {
                self.current = None;
            } else {
                let t = self.elapsed.rem_euclid(duration);
                let weight =
                    smooth(t / 0.45) * smooth((duration - t) / 0.45) * settings.gesture_strength;
                let phase = t / duration * TAU;
                match gesture {
                    Gesture::Nod => target[2].x = 16. * (phase * 2.).sin(),
                    Gesture::Shake => target[2].y = 22. * (phase * 2.).sin(),
                    Gesture::Bow => {
                        target[0].x = 28. * (phase * 0.5).sin();
                        target[2].x = 10. * (phase * 0.5).sin();
                    }
                    Gesture::Wave => {
                        target[4].z = -115.;
                        target[6].y = 65.;
                        target[8].y = 22. * (phase * 3.).sin();
                        target[4].x = 8. * (phase * 3.).sin();
                    }
                    Gesture::Cheer => {
                        target[3].z = 140.;
                        target[4].z = -140.;
                        target[5].y = -20.;
                        target[6].y = 20.;
                        target[0].z = 4. * (phase * 2.).sin();
                    }
                    Gesture::Stretch => {
                        target[3].z = 75.;
                        target[4].z = -75.;
                        target[1].x = -5.;
                        target[2].z = 8. * phase.sin();
                    }
                }
                for v in &mut target {
                    *v *= weight;
                }
            }
        }
        for (out, (from, target)) in self.last.iter_mut().zip(self.from.iter().zip(target)) {
            *out = from.lerp(target, smooth(self.blend));
        }
        if self.blend == 1. {
            self.settling = false;
        }
        let mut pose = self.last;
        if settings.enabled {
            let t = self.idle_phase;
            pose[0] += Vec3::new(
                0.3 * (t * 0.7).sin(),
                (t * 0.6).sin(),
                1.8 * (t * 0.9).sin(),
            ) * settings.sway;
            pose[1].x += 0.8 * (t * 1.5).sin() * settings.breathing;
            for (upper, lower, sign) in [(3, 5, -1.), (4, 6, 1.)] {
                pose[upper] += Vec3::new(
                    2. * (t * 0.9 + sign * 0.4).sin(),
                    0.,
                    sign * (1. + 1.5 * (t * 1.5).sin()),
                ) * settings.arms;
                pose[lower].y += sign * (5. + 2. * (t * 0.9 + 0.6).sin()) * settings.arms;
            }
        }
        pose
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gestures_finish_loop_and_stop_without_jumps_or_drift() {
        let mut settings = Motion {
            enabled: false,
            ..Default::default()
        };
        for gesture in Gesture::ALL {
            let mut player = Player::default();
            player.play(gesture);
            let mut moved = false;
            for _ in 0..320 {
                let pose = player.update(1. / 60., &settings);
                assert!(
                    pose.iter()
                        .all(|p| p.is_finite() && p.abs().max_element() <= 160.)
                );
                moved |= pose.iter().any(|p| p.length() > 1.);
            }
            assert!(moved);
            assert!(!player.active());
            assert_eq!(player.last, [Vec3::ZERO; 11]);
        }
        settings.gesture_loop = true;
        let mut player = Player::default();
        player.play(Gesture::Wave);
        for _ in 0..300 {
            player.update(1. / 60., &settings);
        }
        assert!(player.active());
        let before = player.last;
        player.stop();
        assert_eq!(player.update(0., &settings), before);
        for _ in 0..30 {
            player.update(1. / 60., &settings);
        }
        assert!(!player.active());
        assert_eq!(player.last, [Vec3::ZERO; 11]);
    }
    #[test]
    fn zero_idle_is_exactly_neutral_and_motion_is_frame_rate_independent() {
        let settings = Motion {
            sway: 0.,
            breathing: 0.,
            arms: 0.,
            ..Default::default()
        };
        assert!(!settings.moving());
        assert_eq!(Player::default().update(1., &settings), [Vec3::ZERO; 11]);
        let mut a = Player::default();
        let mut b = Player::default();
        let settings = Motion::default();
        for _ in 0..60 {
            a.update(1. / 60., &settings);
        }
        for _ in 0..120 {
            b.update(1. / 120., &settings);
        }
        for (a, b) in a.update(0., &settings).iter().zip(b.update(0., &settings)) {
            assert!((*a - b).length() < 0.001);
        }
    }
}
