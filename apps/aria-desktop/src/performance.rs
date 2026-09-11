//! App-local Windows scheduling and a frame gate independent of UI repaints.
use std::time::{Duration, Instant};

pub struct FrameClock {
    last: Instant,
}
impl FrameClock {
    pub fn new(now: Instant) -> Self {
        Self { last: now }
    }
    pub fn tick(&mut self, now: Instant, fps: u32, first_pass: bool) -> f32 {
        let elapsed = now.saturating_duration_since(self.last);
        if !first_pass || elapsed < interval(fps) {
            return 0.0;
        }
        self.last = now;
        elapsed.as_secs_f32().min(0.25)
    }
    pub fn remaining(&self, now: Instant, fps: u32) -> Duration {
        (self.last + interval(fps)).saturating_duration_since(now)
    }
}
fn interval(fps: u32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(fps.clamp(15, 120)))
}

#[cfg(windows)]
pub fn set_high_priority(high: bool) -> anyhow::Result<()> {
    use windows::Win32::System::Threading::*;
    let class = if high {
        HIGH_PRIORITY_CLASS
    } else {
        NORMAL_PRIORITY_CLASS
    };
    // Only this process's pseudo handle; never modify OBS or other applications.
    unsafe {
        SetPriorityClass(GetCurrentProcess(), class)?;
        anyhow::ensure!(
            GetPriorityClass(GetCurrentProcess()) == class.0,
            "Windows did not apply the requested priority"
        );
    }
    Ok(())
}
#[cfg(not(windows))]
pub fn set_high_priority(_high: bool) -> anyhow::Result<()> {
    anyhow::bail!("Process priority is available on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouse_repaints_and_layout_passes_do_not_multiply_simulation_rate() {
        let start = Instant::now();
        let mut clock = FrameClock::new(start);
        let mut ticks = 0;
        for ms in 1..=1000 {
            let now = start + Duration::from_millis(ms);
            ticks += usize::from(clock.tick(now, 60, true) > 0.0);
            assert_eq!(clock.tick(now, 60, false), 0.0);
        }
        assert_eq!(ticks, 58); // 1 ms sampled clock, 17 ms per tick.
        assert_eq!(clock.tick(start + Duration::from_secs(10), 60, true), 0.25);
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "changes only the test process priority; run explicitly"]
    fn windows_high_priority_round_trip() {
        use windows::Win32::System::Threading::*;
        let original = unsafe { GetPriorityClass(GetCurrentProcess()) };
        struct Restore(u32);
        impl Drop for Restore {
            fn drop(&mut self) {
                unsafe {
                    let _ = SetPriorityClass(GetCurrentProcess(), PROCESS_CREATION_FLAGS(self.0));
                }
            }
        }
        let _restore = Restore(original);
        set_high_priority(true).unwrap();
        set_high_priority(false).unwrap();
    }
}
