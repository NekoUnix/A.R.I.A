//! Bounded frame history and read-only OS counters sampled once per second.
use eframe::{egui, egui_wgpu::RenderState};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, Instant},
};

#[derive(Default, serde::Serialize)]
pub struct Usage {
    // Original API fields continue to describe the desktop process alone.
    pub cpu_percent: Option<f64>,
    pub ram_bytes: Option<u64>,
    pub private_bytes: Option<u64>,
    pub vram_bytes: Option<u64>,
    pub vram_budget_bytes: Option<u64>,
    pub shared_gpu_bytes: Option<u64>,
    pub managed_cpu_percent: Option<f64>,
    pub managed_ram_bytes: Option<u64>,
    pub managed_private_bytes: Option<u64>,
    pub managed_processes: usize,
    pub readable_processes: usize,
    pub io_read_bytes_per_second: Option<f64>,
    pub io_write_bytes_per_second: Option<f64>,
    pub handles: Option<u64>,
    pub system_ram_available_bytes: Option<u64>,
    pub system_ram_total_bytes: Option<u64>,
    pub frame_average_ms: Option<f64>,
    pub frame_p95_ms: Option<f64>,
    pub slow_frames: u64,
}
#[derive(Clone, Copy, Default)]
struct ProcessSample {
    identity: Option<u64>,
    ticks: Option<u64>,
    ram: Option<u64>,
    private: Option<u64>,
    read: Option<u64>,
    write: Option<u64>,
    handles: Option<u64>,
}
#[derive(Default)]
pub struct Metrics {
    pub usage: Usage,
    last_sample: Option<Instant>,
    previous: BTreeMap<u32, ProcessSample>,
    frame_intervals: VecDeque<f64>,
}
impl Metrics {
    pub fn record_frame(&mut self, seconds: f64, fps: u32) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        if self.frame_intervals.len() == 120 {
            self.frame_intervals.pop_front();
        }
        self.frame_intervals.push_back(seconds * 1000.0);
        if seconds > 1.5 / f64::from(fps.clamp(15, 120)) {
            self.usage.slow_frames = self.usage.slow_frames.saturating_add(1);
        }
    }
    pub fn update(&mut self, state: Option<&RenderState>, workers: impl Iterator<Item = u32>) {
        let now = Instant::now();
        if self
            .last_sample
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(1))
        {
            return;
        }
        let seconds = self
            .last_sample
            .map(|t| now.duration_since(t).as_secs_f64());
        self.last_sample = Some(now);
        let processors = std::thread::available_parallelism().map_or(1, usize::from);
        let ids: BTreeSet<_> = workers.chain([std::process::id()]).collect();
        let samples: BTreeMap<_, _> = ids.into_iter().map(|id| (id, process_usage(id))).collect();
        let rate = |id: &u32, sample: &ProcessSample, field: fn(&ProcessSample) -> Option<u64>| {
            let previous = self.previous.get(id)?;
            if sample.identity.is_none() || sample.identity != previous.identity {
                return None;
            }
            counter_rate(field(previous)?, field(sample)?, seconds?)
        };
        let main = samples[&std::process::id()];
        let u = &mut self.usage;
        u.cpu_percent =
            rate(&std::process::id(), &main, |s| s.ticks).map(|r| normalized_cpu(r, processors));
        u.ram_bytes = main.ram;
        u.private_bytes = main.private;
        u.managed_processes = samples.len();
        u.readable_processes = samples.values().filter(|s| s.ram.is_some()).count();
        u.managed_ram_bytes = samples.values().map(|s| s.ram).sum();
        u.managed_private_bytes = samples.values().map(|s| s.private).sum();
        u.handles = samples.values().map(|s| s.handles).sum();
        u.managed_cpu_percent = samples
            .iter()
            .map(|(id, s)| rate(id, s, |s| s.ticks))
            .sum::<Option<f64>>()
            .map(|r| normalized_cpu(r, processors));
        u.io_read_bytes_per_second = samples.iter().map(|(id, s)| rate(id, s, |s| s.read)).sum();
        u.io_write_bytes_per_second = samples.iter().map(|(id, s)| rate(id, s, |s| s.write)).sum();
        self.previous = samples;
        (u.system_ram_available_bytes, u.system_ram_total_bytes) = system_memory();
        let gpu = gpu_usage(state);
        u.vram_bytes = gpu.map(|v| v.0);
        u.vram_budget_bytes = gpu.map(|v| v.1);
        u.shared_gpu_bytes = gpu.and_then(|v| v.2);
        let mut frames: Vec<_> = self.frame_intervals.iter().copied().collect();
        if !frames.is_empty() {
            frames.sort_by(f64::total_cmp);
            u.frame_average_ms = Some(frames.iter().sum::<f64>() / frames.len() as f64);
            u.frame_p95_ms = Some(frames[(frames.len() * 95).div_ceil(100) - 1]);
        }
    }
    pub fn footer(
        &self,
        ui: &mut egui::Ui,
        snapshot: &aria_tracking::Snapshot,
        fps: f32,
        ui_seconds: Option<f32>,
        gpu: &str,
    ) {
        let u = &self.usage;
        ui.small(format!("{fps:.0} FPS")).on_hover_text(
            "Model update rate. This can differ from phone tracking rate and display refresh rate.",
        );
        ui.separator();
        ui.small(format!("CPU {}", number(u.managed_cpu_percent, "%"))).on_hover_text("Desktop + owned Cubism/camera workers; normalized across logical processors. First sample or an unreadable worker shows N/A.");
        ui.small(format!("RAM {}", mib(u.managed_ram_bytes))).on_hover_text("Sum of resident working sets for ARIA and owned workers. Shared pages may be counted in more than one process.");
        ui.small(format!("VRAM {}", mib(u.vram_bytes))).on_hover_text("ARIA local GPU allocation, not GPU utilization. Budget and shared GPU memory are in Details.");
        let age = snapshot
            .received_at
            .map(|t| t.elapsed().as_secs_f64() * 1000.0);
        let fresh = age.is_some_and(|ms| ms < 1000.0);
        let tracking = if fresh {
            format!(
                "Tracking {:.0} Hz · {:.0} ms",
                snapshot.packets_per_second,
                age.unwrap()
            )
        } else {
            "Tracking —".into()
        };
        ui.small(tracking).on_hover_text("Accepted tracking packets per second and time since the latest packet arrived locally. This is not camera-to-screen latency. Demo/disconnected sources show a dash.");
        let details = ui.button("Details");
        let popup = egui::Popup::menu(&details);
        #[cfg(feature = "screenshots")]
        let popup = if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("performance-details")
        {
            popup.open(true)
        } else {
            popup
        };
        popup.show(|ui| {
            ui.set_min_width(325.0);
            ui.strong("Performance · local counters");
            crate::help::button(ui, "metrics");
            ui.small(gpu);
            ui.separator();
            egui::Grid::new("resource-details").num_columns(2).show(ui, |ui| {
                let mut row = |name: &str, value: String| { ui.label(name); ui.monospace(value); ui.end_row(); };
                row("Desktop CPU", number(u.cpu_percent, "%"));
                row("Desktop RAM", mib(u.ram_bytes));
                row("Owned processes readable", format!("{} / {}",u.readable_processes,u.managed_processes));
                row("Total private commit", mib(u.managed_private_bytes));
                row("System RAM available / total", format!("{} / {}",mib(u.system_ram_available_bytes),mib(u.system_ram_total_bytes)));
                row("GPU budget", mib(u.vram_budget_bytes));
                row("Shared GPU memory", mib(u.shared_gpu_bytes));
                row("Process I/O read / write", format!("{} / {}", rate_text(u.io_read_bytes_per_second), rate_text(u.io_write_bytes_per_second)));
                row("Open handles", u.handles.map_or_else(||"N/A".into(),|n|n.to_string()));
                row("Frame interval average / p95", format!("{} / {}", number(u.frame_average_ms," ms"),number(u.frame_p95_ms," ms")));
                row("Slow updates this session", u.slow_frames.to_string());
                row("Previous UI work", number(ui_seconds.map(|v|f64::from(v)*1000.0)," ms"));
                row("Accepted / rejected / ignored packets", format!("{} / {} / {}",snapshot.packets,snapshot.rejected,snapshot.ignored));
            });
            ui.separator();
            ui.small("OS counters: 1 Hz. Frame history: last 120 updates.\nSlow: >1.5× frame budget. I/O includes pipes and network.\nOnly directly owned workers are included. N/A = unavailable.\nThese counters stay on your computer.");
        });
    }
}
fn number(value: Option<f64>, unit: &str) -> String {
    value.map_or_else(|| "N/A".into(), |v| format!("{v:.1}{unit}"))
}
fn mib(value: Option<u64>) -> String {
    value.map_or_else(
        || "N/A".into(),
        |v| format!("{:.0} MiB", v as f64 / 1048576.0),
    )
}
fn rate_text(value: Option<f64>) -> String {
    number(value.map(|v| v / 1024.0), " KiB/s")
}
fn counter_rate(before: u64, after: u64, seconds: f64) -> Option<f64> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    Some(after.checked_sub(before)? as f64 / seconds)
}
fn normalized_cpu(ticks_per_second: f64, processors: usize) -> f64 {
    (ticks_per_second / 10_000_000.0 / processors.max(1) as f64 * 100.0).clamp(0.0, 100.0)
}

#[cfg(windows)]
fn process_usage(id: u32) -> ProcessSample {
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
            Threading::*,
        },
    };
    struct OwnedHandle(HANDLE);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    // Only explicit child IDs supplied by ARIA, never process discovery. Handles
    // are read-only and closed after each sample. The self pseudo handle is borrowed.
    unsafe {
        let owned = if id == std::process::id() {
            None
        } else {
            match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, id) {
                Ok(h) => Some(OwnedHandle(h)),
                Err(_) => return ProcessSample::default(),
            }
        };
        let process = owned.as_ref().map_or_else(|| GetCurrentProcess(), |h| h.0);
        let mut sample = ProcessSample::default();
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        if GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user).is_ok() {
            let value =
                |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
            sample.identity = Some(value(created));
            sample.ticks = Some(value(kernel).saturating_add(value(user)));
        }
        let mut memory = PROCESS_MEMORY_COUNTERS_EX {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        if GetProcessMemoryInfo(
            process,
            (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            memory.cb,
        )
        .is_ok()
        {
            sample.ram = Some(memory.WorkingSetSize as u64);
            sample.private = Some(memory.PrivateUsage as u64);
        }
        let mut io = IO_COUNTERS::default();
        if GetProcessIoCounters(process, &mut io).is_ok() {
            sample.read = Some(io.ReadTransferCount);
            sample.write = Some(io.WriteTransferCount);
        }
        let mut handles = 0;
        if GetProcessHandleCount(process, &mut handles).is_ok() {
            sample.handles = Some(u64::from(handles));
        }
        sample
    }
}
#[cfg(not(windows))]
fn process_usage(_id: u32) -> ProcessSample {
    ProcessSample::default()
}

#[cfg(windows)]
fn system_memory() -> (Option<u64>, Option<u64>) {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut memory = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    if unsafe { GlobalMemoryStatusEx(&mut memory) }.is_ok() {
        (Some(memory.ullAvailPhys), Some(memory.ullTotalPhys))
    } else {
        (None, None)
    }
}
#[cfg(not(windows))]
fn system_memory() -> (Option<u64>, Option<u64>) {
    (None, None)
}
#[cfg(windows)]
fn gpu_usage(state: Option<&RenderState>) -> Option<(u64, u64, Option<u64>)> {
    use windows::Win32::Graphics::Dxgi::{
        DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
    };
    let state = state?;
    // Borrow the exact adapter held by wgpu. Keep the HAL guard alive throughout
    // both read-only queries; never release, mutate or retain its raw objects.
    let adapter = unsafe { state.adapter.as_hal::<wgpu::hal::api::Dx12>() }?;
    let local = adapter
        .raw_adapter()
        .query_video_memory_info(DXGI_MEMORY_SEGMENT_GROUP_LOCAL)
        .ok()?;
    let shared = adapter
        .raw_adapter()
        .query_video_memory_info(DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL)
        .ok()
        .map(|v| v.CurrentUsage);
    Some((local.CurrentUsage, local.Budget, shared))
}
#[cfg(not(windows))]
fn gpu_usage(_state: Option<&RenderState>) -> Option<(u64, u64, Option<u64>)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    #[ignore = "subprocess fixture for owned_worker_rates"]
    fn owned_worker_fixture() {
        use std::io::Read;
        let _ = std::io::stdin().read_to_end(&mut Vec::new());
    }
    #[test]
    #[cfg(windows)]
    fn owned_worker_rates_include_child_and_recover_after_exit() {
        use std::{
            os::windows::process::CommandExt,
            process::{Command, Stdio},
        };
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let child = Child(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "metrics::tests::owned_worker_fixture",
                    "--ignored",
                ])
                .creation_flags(0x08000000)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut m = Metrics::default();
        m.update(None, [child.0.id()].into_iter());
        assert_eq!(m.usage.readable_processes, 2);
        assert!(m.usage.managed_ram_bytes.unwrap() >= m.usage.ram_bytes.unwrap());
        assert!(m.usage.managed_cpu_percent.is_none());
        m.last_sample = Some(Instant::now() - Duration::from_secs(1));
        m.update(None, [child.0.id()].into_iter());
        assert!(m.usage.managed_cpu_percent.is_some());
        assert!(m.usage.io_read_bytes_per_second.is_some());
        drop(child);
        m.last_sample = Some(Instant::now() - Duration::from_secs(1));
        m.update(None, std::iter::empty());
        assert_eq!(m.usage.managed_processes, 1);
        assert_eq!(m.previous.len(), 1);
        assert!(m.usage.managed_cpu_percent.is_some());
    }
    #[test]
    fn rates_reject_resets_and_cpu_counts_all_threads() {
        assert_eq!(
            normalized_cpu(counter_rate(10, 20_000_010, 1.0).unwrap(), 8),
            25.0
        );
        assert_eq!(counter_rate(10, 9, 1.0), None);
        assert_eq!(counter_rate(0, 1, 0.0), None);
        assert_eq!(counter_rate(0, 1, f64::NAN), None);
        assert_eq!(mib(None), "N/A");
        assert_eq!(mib(Some(0)), "0 MiB");
    }
    #[test]
    fn frame_history_is_bounded_and_counts_stalls() {
        let mut m = Metrics::default();
        for _ in 0..240 {
            m.record_frame(1.0 / 60.0, 60);
        }
        m.record_frame(f64::NAN, 60);
        m.record_frame(0.2, 60);
        m.update(None, std::iter::empty());
        assert_eq!(m.frame_intervals.len(), 120);
        assert_eq!(m.usage.slow_frames, 1);
        assert!((m.usage.frame_p95_ms.unwrap() - 1000.0 / 60.0).abs() < 0.01);
    }
    #[test]
    #[cfg(windows)]
    fn windows_reports_owned_process_and_missing_is_not_zero() {
        let sample = process_usage(std::process::id());
        assert!(sample.ticks.is_some());
        assert!(sample.ram.unwrap() > 0);
        assert!(sample.private.unwrap() > 0);
        assert!(sample.handles.unwrap() > 0);
        assert!(system_memory().1.unwrap() > 0);
        assert!(process_usage(u32::MAX).ram.is_none());
        let mut m = Metrics::default();
        m.update(None, [std::process::id(), u32::MAX].into_iter());
        assert_eq!(m.usage.managed_processes, 2);
        assert_eq!(m.usage.readable_processes, 1);
        assert!(m.usage.managed_ram_bytes.is_none());
        assert!(m.usage.managed_cpu_percent.is_none());
    }
}
