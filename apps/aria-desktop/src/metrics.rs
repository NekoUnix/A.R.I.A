//! Read-only, once-per-second counters for this process and its rendering adapter.
use eframe::{egui, egui_wgpu::RenderState};
use std::time::{Duration, Instant};

#[derive(Default, serde::Serialize)]
pub struct Usage {
    pub cpu_percent: Option<f64>,
    pub ram_bytes: Option<u64>,
    pub private_bytes: Option<u64>,
    pub vram_bytes: Option<u64>,
    pub vram_budget_bytes: Option<u64>,
    pub shared_gpu_bytes: Option<u64>,
}
pub struct Metrics {
    pub usage: Usage,
    last_sample: Option<Instant>,
    previous_cpu: Option<(Instant, u64)>,
    processors: usize,
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            usage: Usage::default(),
            last_sample: None,
            previous_cpu: None,
            processors: std::thread::available_parallelism().map_or(1, usize::from),
        }
    }
}
impl Metrics {
    pub fn update(&mut self, state: Option<&RenderState>) {
        let now = Instant::now();
        if self
            .last_sample
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(1))
        {
            return;
        }
        self.last_sample = Some(now);
        let (ticks, ram, private) = process_usage();
        self.usage.cpu_percent = ticks.and_then(|ticks| {
            self.previous_cpu.and_then(|(last, prev)| {
                cpu_percent(
                    prev,
                    ticks,
                    now.duration_since(last).as_secs_f64(),
                    self.processors,
                )
            })
        });
        self.previous_cpu = ticks.map(|t| (now, t));
        self.usage.ram_bytes = ram;
        self.usage.private_bytes = private;
        let gpu = gpu_usage(state);
        self.usage.vram_bytes = gpu.map(|v| v.0);
        self.usage.vram_budget_bytes = gpu.map(|v| v.1);
        self.usage.shared_gpu_bytes = gpu.and_then(|v| v.2);
    }
    pub fn footer(&self, ui: &mut egui::Ui) {
        crate::help::button(ui, "metrics");
        let u = &self.usage;
        ui.label(egui::RichText::new(format!("CPU {}",u.cpu_percent.map_or_else(||"N/A".into(),|v|format!("{v:.1}%")))).small())
            .on_hover_text(format!("ARIA process CPU time, normalized across {} available logical processors. Sampled every second; 100% means all processors busy.",self.processors));
        ui.label(egui::RichText::new(format!("RAM {}", mib(u.ram_bytes))).small())
            .on_hover_text(format!(
                "ARIA resident working set. Private committed memory: {}. Sampled every second.",
                mib(u.private_bytes)
            ));
        ui.label(egui::RichText::new(format!("VRAM {}",mib(u.vram_bytes))).small())
            .on_hover_text(format!("ARIA's local GPU memory usage on the rendering adapter (DXGI). Budget: {}. Shared/non-local GPU memory: {}. Includes driver allocations; integrated GPUs use system memory. N/A means the backend or driver cannot report this counter.",mib(u.vram_budget_bytes),mib(u.shared_gpu_bytes)));
    }
}
fn mib(bytes: Option<u64>) -> String {
    bytes.map_or_else(
        || "N/A".into(),
        |b| format!("{:.0} MiB", b as f64 / 1048576.0),
    )
}
fn cpu_percent(before: u64, after: u64, seconds: f64, processors: usize) -> Option<f64> {
    if !seconds.is_finite() || seconds <= 0.0 || processors == 0 {
        return None;
    }
    let delta = after.checked_sub(before)?;
    Some((delta as f64 / 10_000_000.0 / seconds / processors as f64 * 100.0).clamp(0.0, 100.0))
}

#[cfg(windows)]
fn process_usage() -> (Option<u64>, Option<u64>, Option<u64>) {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    };
    // Current-process pseudo handle needs no CloseHandle. Both APIs write only
    // to stack-owned structures of the documented size; no foreign memory read.
    unsafe {
        let process = GetCurrentProcess();
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let ticks = GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user)
            .ok()
            .map(|()| {
                let value =
                    |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
                value(kernel).saturating_add(value(user))
            });
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
            (
                ticks,
                Some(memory.WorkingSetSize as u64),
                Some(memory.PrivateUsage as u64),
            )
        } else {
            (ticks, None, None)
        }
    }
}
#[cfg(not(windows))]
fn process_usage() -> (Option<u64>, Option<u64>, Option<u64>) {
    (None, None, None)
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
    fn cpu_counts_all_threads_and_normalizes_to_machine_capacity() {
        assert_eq!(cpu_percent(10, 20_000_010, 1.0, 8), Some(25.0));
        assert_eq!(cpu_percent(10, 10, 1.0, 8), Some(0.0));
        assert_eq!(cpu_percent(10, 9, 1.0, 8), None);
        assert_eq!(cpu_percent(0, 10, 0.0, 8), None);
        assert_eq!(mib(None), "N/A");
        assert_eq!(mib(Some(0)), "0 MiB");
    }
    #[test]
    #[cfg(windows)]
    fn windows_reports_the_current_process() {
        let (cpu, ram, private) = process_usage();
        assert!(cpu.is_some());
        assert!(ram.unwrap() > 0);
        assert!(private.unwrap() > 0);
    }
}
