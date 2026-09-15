//! One sample per second, a two-minute plot, and lifetime extrema kept separately.
use super::Usage;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::{collections::VecDeque, time::Instant};

const WINDOW_SECONDS: f64 = 120.0;
const MAX_SAMPLES: usize = 121;
const MAX_JOIN_SECONDS: f64 = 2.5;

#[derive(Clone, Copy, Debug)]
struct Sample {
    seconds: f64,
    value: Option<f64>,
}

#[derive(Default)]
struct Series {
    samples: VecDeque<Sample>,
    extrema: Option<(f64, f64)>,
}
impl Series {
    fn record(&mut self, seconds: f64, value: Option<f64>) {
        if !seconds.is_finite()
            || seconds < 0.0
            || self
                .samples
                .back()
                .is_some_and(|last| seconds <= last.seconds)
        {
            return;
        }
        let value = value.filter(|v| v.is_finite() && *v >= 0.0);
        if let Some(v) = value {
            self.extrema = Some(
                self.extrema
                    .map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))),
            );
        }
        while self.samples.len() >= MAX_SAMPLES
            || self
                .samples
                .front()
                .is_some_and(|s| s.seconds < seconds - WINDOW_SECONDS)
        {
            self.samples.pop_front();
        }
        self.samples.push_back(Sample { seconds, value });
    }
    fn current(&self) -> Option<f64> {
        self.samples.back().and_then(|s| s.value)
    }
    fn nearest(&self, seconds: f64) -> Option<&Sample> {
        self.samples
            .iter()
            .min_by(|a, b| {
                (a.seconds - seconds)
                    .abs()
                    .total_cmp(&(b.seconds - seconds).abs())
            })
            .filter(|s| (s.seconds - seconds).abs() <= MAX_JOIN_SECONDS / 2.0)
    }
    fn ceiling(&self, unit: Unit) -> f64 {
        // Each graph uses its visible samples, so an old session peak cannot
        // flatten today's line. The actual plot scale is shown in its tooltip.
        self.samples
            .iter()
            .filter_map(|s| s.value)
            .fold(unit.minimum_scale(), f64::max)
            * 1.15
    }
}

#[derive(Clone, Copy)]
enum Unit {
    Percent,
    Bytes,
    BytesPerSecond,
    Milliseconds,
    Fps,
    Hz,
    Count,
}
impl Unit {
    fn minimum_scale(self) -> f64 {
        match self {
            Self::Bytes => 1048576.0,
            Self::BytesPerSecond => 1024.0,
            _ => 1.0,
        }
    }
    fn format(self, value: Option<f64>) -> String {
        let Some(v) = value.filter(|v| v.is_finite()) else {
            return "N/A".into();
        };
        match self {
            Self::Percent => format!("{v:.2}%"),
            Self::Milliseconds => format!("{v:.2} ms"),
            Self::Fps => format!("{v:.1} FPS"),
            Self::Hz => format!("{v:.1} Hz"),
            Self::Count => format!("{v:.0}"),
            Self::Bytes | Self::BytesPerSecond => {
                let (divisor, name) = if v >= 1073741824.0 {
                    (1073741824.0, "GiB")
                } else if v >= 1048576.0 {
                    (1048576.0, "MiB")
                } else if v >= 1024.0 {
                    (1024.0, "KiB")
                } else {
                    (1.0, "B")
                };
                let suffix = if matches!(self, Self::BytesPerSecond) {
                    "/s"
                } else {
                    ""
                };
                format!("{:.2} {name}{suffix}", v / divisor)
            }
        }
    }
}

#[derive(Clone, Copy)]
#[repr(usize)]
enum Metric {
    Fps,
    Cpu,
    Ram,
    Vram,
    TrackingRate,
    TrackingAge,
    DesktopCpu,
    DesktopRam,
    PrivateCommit,
    GpuBudget,
    SharedGpu,
    SystemAvailable,
    SystemTotal,
    IoRead,
    IoWrite,
    Handles,
    FrameAverage,
    FrameP95,
    UiWork,
    SlowUpdates,
    Processes,
    ReadableProcesses,
    Packets,
    Rejected,
    Ignored,
}
const ALL: [Metric; 25] = [
    Metric::Fps,
    Metric::Cpu,
    Metric::Ram,
    Metric::Vram,
    Metric::TrackingRate,
    Metric::TrackingAge,
    Metric::DesktopCpu,
    Metric::DesktopRam,
    Metric::PrivateCommit,
    Metric::GpuBudget,
    Metric::SharedGpu,
    Metric::SystemAvailable,
    Metric::SystemTotal,
    Metric::IoRead,
    Metric::IoWrite,
    Metric::Handles,
    Metric::FrameAverage,
    Metric::FrameP95,
    Metric::UiWork,
    Metric::SlowUpdates,
    Metric::Processes,
    Metric::ReadableProcesses,
    Metric::Packets,
    Metric::Rejected,
    Metric::Ignored,
];
impl Metric {
    fn label(self) -> &'static str {
        match self {
            Self::Fps => "Model FPS",
            Self::Cpu => "CPU",
            Self::Ram => "RAM",
            Self::Vram => "VRAM",
            Self::TrackingRate => "Tracking Hz",
            Self::TrackingAge => "Packet age",
            Self::DesktopCpu => "Desktop CPU",
            Self::DesktopRam => "Desktop RAM",
            Self::PrivateCommit => "Private commit",
            Self::GpuBudget => "GPU budget",
            Self::SharedGpu => "Shared GPU memory",
            Self::SystemAvailable => "Available system RAM",
            Self::SystemTotal => "Total system RAM",
            Self::IoRead => "Process I/O · read",
            Self::IoWrite => "Process I/O · write",
            Self::Handles => "Open handles",
            Self::FrameAverage => "Frame interval · average",
            Self::FrameP95 => "Frame interval · p95",
            Self::UiWork => "UI work",
            Self::SlowUpdates => "Slow updates",
            Self::Processes => "Owned processes",
            Self::ReadableProcesses => "Readable processes",
            Self::Packets => "Accepted packets",
            Self::Rejected => "Rejected packets",
            Self::Ignored => "Ignored packets",
        }
    }
    fn unit(self) -> Unit {
        match self {
            Self::Fps => Unit::Fps,
            Self::Cpu | Self::DesktopCpu => Unit::Percent,
            Self::Ram
            | Self::Vram
            | Self::DesktopRam
            | Self::PrivateCommit
            | Self::GpuBudget
            | Self::SharedGpu
            | Self::SystemAvailable
            | Self::SystemTotal => Unit::Bytes,
            Self::TrackingRate => Unit::Hz,
            Self::TrackingAge | Self::FrameAverage | Self::FrameP95 | Self::UiWork => {
                Unit::Milliseconds
            }
            Self::IoRead | Self::IoWrite => Unit::BytesPerSecond,
            _ => Unit::Count,
        }
    }
    fn color(self) -> Color32 {
        match self {
            Self::Fps | Self::IoRead | Self::Packets => Color32::from_rgb(53, 199, 166),
            Self::Cpu | Self::DesktopCpu | Self::Handles => Color32::from_rgb(246, 170, 72),
            Self::Ram | Self::DesktopRam | Self::SystemTotal => Color32::from_rgb(84, 167, 255),
            Self::Vram | Self::GpuBudget | Self::SharedGpu => Color32::from_rgb(177, 135, 255),
            Self::TrackingRate | Self::PrivateCommit | Self::ReadableProcesses => {
                Color32::from_rgb(55, 192, 216)
            }
            Self::TrackingAge | Self::FrameP95 | Self::Rejected => Color32::from_rgb(241, 118, 167),
            Self::FrameAverage | Self::SystemAvailable | Self::Processes => {
                Color32::from_rgb(150, 196, 73)
            }
            Self::UiWork | Self::IoWrite | Self::SlowUpdates | Self::Ignored => {
                Color32::from_rgb(244, 132, 98)
            }
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Fps => {
                "Actual model update rate, not the target, display refresh rate or camera rate."
            }
            Self::Cpu => {
                "Desktop + directly owned workers; normalized across logical processors. N/A while awaiting two readable samples."
            }
            Self::DesktopCpu => "Desktop process CPU only; normalized across logical processors.",
            Self::Ram => {
                "Resident RAM for desktop + directly owned workers. Shared pages can be counted more than once."
            }
            Self::DesktopRam => "Resident working set of the desktop process only.",
            Self::PrivateCommit => {
                "Committed private memory for desktop + directly owned workers; not necessarily resident RAM."
            }
            Self::Vram => {
                "ARIA's local GPU allocation on its rendering adapter; not GPU utilization."
            }
            Self::GpuBudget => "The local GPU memory budget supplied by the driver for ARIA.",
            Self::SharedGpu => {
                "ARIA's shared/non-local GPU allocation; integrated GPUs can use system memory."
            }
            Self::SystemAvailable => "Physical RAM currently available across the whole computer.",
            Self::SystemTotal => "Total physical RAM installed in the computer.",
            Self::IoRead | Self::IoWrite => {
                "Desktop + directly owned worker traffic, including files, network and IPC pipes. This is not disk throughput."
            }
            Self::Handles => {
                "Open operating-system handles across the desktop and directly owned workers."
            }
            Self::TrackingRate => {
                "Accepted tracking packets per second. A disconnected or stale source produces a gap, not a zero sample."
            }
            Self::TrackingAge => {
                "Time since the latest accepted packet arrived locally, sampled at 1 Hz. Not camera-to-screen latency; stale sources produce a gap."
            }
            Self::FrameAverage => {
                "Average interval over the last 120 model updates, including stalls."
            }
            Self::FrameP95 => {
                "95% of the last 120 model-update intervals are at or below this value."
            }
            Self::UiWork => {
                "Previous application UI update's CPU duration; not full GPU latency or OBS encoding."
            }
            Self::SlowUpdates => {
                "Cumulative model intervals above 1.5 times the selected frame budget since ARIA opened."
            }
            Self::Processes => {
                "Desktop plus directly owned Cubism and camera/setup workers. Other applications and worker descendants are not discovered."
            }
            Self::ReadableProcesses => {
                "Owned processes whose resident memory could be read at this sample."
            }
            Self::Packets => {
                "Cumulative accepted packets for the current receiver. Reconnecting can reset the counter; session low/high are retained."
            }
            Self::Rejected => {
                "Packets rejected by validation for the current receiver. Reconnecting can reset the counter; session low/high are retained."
            }
            Self::Ignored => {
                "Wrong-sender or stale packets ignored by the current receiver. Reconnecting can reset the counter; session low/high are retained."
            }
        }
    }
}

pub(super) struct History {
    started: Instant,
    seconds: f64,
    series: [Series; ALL.len()],
}
impl Default for History {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            seconds: 0.0,
            series: std::array::from_fn(|_| Series::default()),
        }
    }
}
impl History {
    pub(super) fn record(
        &mut self,
        usage: &Usage,
        snapshot: &aria_tracking::Snapshot,
        fps: f32,
        ui_seconds: Option<f32>,
    ) {
        self.record_at(
            self.started.elapsed().as_secs_f64(),
            usage,
            snapshot,
            fps,
            ui_seconds,
        );
    }
    fn record_at(
        &mut self,
        seconds: f64,
        u: &Usage,
        snapshot: &aria_tracking::Snapshot,
        fps: f32,
        ui_seconds: Option<f32>,
    ) {
        if !seconds.is_finite() || seconds < self.seconds {
            return;
        }
        self.seconds = seconds;
        let age = snapshot
            .received_at
            .map(|t| t.elapsed().as_secs_f64() * 1000.0)
            .filter(|age| *age < 1000.0);
        let bytes = |value: Option<u64>| value.map(|n| n as f64);
        for metric in ALL {
            let value = match metric {
                Metric::Fps => {
                    (fps > 0.0 && u.frame_average_ms.is_some()).then_some(f64::from(fps))
                }
                Metric::Cpu => u.managed_cpu_percent,
                Metric::DesktopCpu => u.cpu_percent,
                Metric::Ram => bytes(u.managed_ram_bytes),
                Metric::DesktopRam => bytes(u.ram_bytes),
                Metric::Vram => bytes(u.vram_bytes),
                Metric::GpuBudget => bytes(u.vram_budget_bytes),
                Metric::SharedGpu => bytes(u.shared_gpu_bytes),
                Metric::PrivateCommit => bytes(u.managed_private_bytes),
                Metric::SystemAvailable => bytes(u.system_ram_available_bytes),
                Metric::SystemTotal => bytes(u.system_ram_total_bytes),
                Metric::IoRead => u.io_read_bytes_per_second,
                Metric::IoWrite => u.io_write_bytes_per_second,
                Metric::Handles => bytes(u.handles),
                Metric::TrackingRate => age.map(|_| f64::from(snapshot.packets_per_second)),
                Metric::TrackingAge => age,
                Metric::FrameAverage => u.frame_average_ms,
                Metric::FrameP95 => u.frame_p95_ms,
                Metric::UiWork => ui_seconds.map(|v| f64::from(v) * 1000.0),
                Metric::SlowUpdates => Some(u.slow_frames as f64),
                Metric::Processes => Some(u.managed_processes as f64),
                Metric::ReadableProcesses => Some(u.readable_processes as f64),
                Metric::Packets => Some(snapshot.packets as f64),
                Metric::Rejected => Some(snapshot.rejected as f64),
                Metric::Ignored => Some(snapshot.ignored as f64),
            };
            self.series[metric as usize].record(seconds, value);
        }
    }
    pub(super) fn footer(&self, ui: &mut egui::Ui, gpu: &str) {
        let width =
            ((ui.available_width() - 88.0) / 6.0 - ui.spacing().item_spacing.x).clamp(75.0, 145.0);
        for metric in &ALL[..6] {
            self.chart(ui, *metric, Vec2::new(width, 57.0), false);
        }
        let button = ui.button("Graphs");
        let popup = egui::Popup::menu(&button)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        #[cfg(feature = "screenshots")]
        let popup = if crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("performance-details")
        {
            popup.open(true)
        } else {
            popup
        };
        popup.show(|ui| {
            ui.set_width(590.0_f32.min(ui.ctx().content_rect().width() - 40.0));
            ui.set_height((ui.ctx().content_rect().height() - 125.0).clamp(300.0, 610.0));
            ui.horizontal(|ui| {
                ui.strong("Performance · this session");
                crate::help::button(ui, "metrics");
            });
            ui.small(format!("Open for {} · last 2 minutes · sampled once per second", elapsed(self.seconds)));
            ui.small("Hover a graph for values and session low/high. Each graph scales independently.");
            ui.small(gpu);
            ui.separator();
            egui::ScrollArea::vertical().max_height(ui.available_height()).auto_shrink([false, false]).show(ui, |ui| {
                self.group(ui, "Overview", &ALL[..6], true);
                self.group(ui, "Frame timing", &[Metric::Fps, Metric::FrameAverage, Metric::FrameP95, Metric::UiWork, Metric::SlowUpdates], false);
                self.group(ui, "CPU, memory & processes", &[Metric::Cpu, Metric::Ram, Metric::DesktopCpu, Metric::DesktopRam, Metric::PrivateCommit, Metric::Handles, Metric::Processes, Metric::ReadableProcesses], false);
                self.group(ui, "GPU & system memory", &[Metric::Vram, Metric::GpuBudget, Metric::SharedGpu, Metric::SystemAvailable, Metric::SystemTotal], false);
                self.group(ui, "Tracking & I/O", &[Metric::TrackingRate, Metric::TrackingAge, Metric::IoRead, Metric::IoWrite, Metric::Packets, Metric::Rejected, Metric::Ignored], false);
                ui.small("Session extrema include older samples no longer in the graph. They reset only when ARIA closes. Gaps / N/A mean unavailable, not zero. Counters stay on this computer.");
            });
        });
    }
    fn group(&self, ui: &mut egui::Ui, label: &str, metrics: &[Metric], open: bool) {
        egui::CollapsingHeader::new(label)
            .id_salt(("performance-graphs", label))
            .default_open(open)
            .show(ui, |ui| {
                let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                for row in metrics.chunks(2) {
                    ui.horizontal(|ui| {
                        for metric in row {
                            self.chart(ui, *metric, Vec2::new(width, 110.0), true);
                        }
                    });
                }
            });
    }
    fn chart(
        &self,
        ui: &mut egui::Ui,
        metric: Metric,
        size: Vec2,
        expanded: bool,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let series = &self.series[metric as usize];
        let color = metric.color();
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
        painter.circle_filled(rect.min + egui::vec2(8.0, 10.0), 2.5, color);
        painter.text(
            rect.min + egui::vec2(15.0, 10.0),
            Align2::LEFT_CENTER,
            metric.label(),
            FontId::proportional(if expanded { 11.0 } else { 10.0 }),
            ui.visuals().text_color(),
        );
        let plot = Rect::from_min_max(
            rect.min + egui::vec2(5.0, 21.0),
            rect.max - egui::vec2(5.0, 21.0),
        );
        let start = (self.seconds - WINDOW_SECONDS).max(0.0);
        let duration = (self.seconds - start).max(1.0);
        let ceiling = series.ceiling(metric.unit());
        let position = |s: &Sample, value: f64| {
            Pos2::new(
                plot.left()
                    + ((s.seconds - start) / duration).clamp(0.0, 1.0) as f32 * plot.width(),
                plot.bottom() - (value / ceiling).clamp(0.0, 1.0) as f32 * plot.height(),
            )
        };
        for fraction in if expanded {
            &[0.0, 0.5, 1.0][..]
        } else {
            &[0.0][..]
        } {
            let y = plot.bottom() - plot.height() * fraction;
            painter.line_segment(
                [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                Stroke::new(0.5, ui.visuals().widgets.noninteractive.bg_stroke.color),
            );
        }
        let mut fill = egui::Mesh::default();
        let mut line = Vec::with_capacity(series.samples.len());
        let mut strokes = Vec::new();
        let mut previous: Option<&Sample> = None;
        let mut latest_point = None;
        for sample in &series.samples {
            if sample.value.is_none()
                || previous.is_some_and(|p| sample.seconds - p.seconds > MAX_JOIN_SECONDS)
            {
                if line.len() > 1 {
                    strokes.push(egui::Shape::line(
                        std::mem::take(&mut line),
                        Stroke::new(1.7, color),
                    ));
                }
                line.clear();
                previous = None;
            }
            if let Some(value) = sample.value {
                let point = position(sample, value);
                if let Some(prev) = previous.and_then(|p| p.value.map(|v| position(p, v))) {
                    let index = fill.vertices.len() as u32;
                    for (pos, tint) in [
                        (prev, color.gamma_multiply(0.30)),
                        (point, color.gamma_multiply(0.30)),
                        (
                            egui::pos2(point.x, plot.bottom()),
                            color.gamma_multiply(0.03),
                        ),
                        (
                            egui::pos2(prev.x, plot.bottom()),
                            color.gamma_multiply(0.03),
                        ),
                    ] {
                        fill.vertices.push(egui::epaint::Vertex {
                            pos,
                            uv: egui::epaint::WHITE_UV,
                            color: tint,
                        });
                    }
                    fill.indices.extend_from_slice(&[
                        index,
                        index + 1,
                        index + 2,
                        index,
                        index + 2,
                        index + 3,
                    ]);
                }
                line.push(point);
                latest_point = Some(point);
                previous = Some(sample);
            }
        }
        painter.add(egui::Shape::mesh(fill));
        if line.len() > 1 {
            strokes.push(egui::Shape::line(line, Stroke::new(1.7, color)));
        }
        painter.extend(strokes);
        if let Some(point) = latest_point {
            painter.circle_filled(point, 2.0, color);
        }
        if series.current().is_none() && expanded {
            painter.text(
                plot.right_top(),
                Align2::RIGHT_TOP,
                if series.samples.is_empty() {
                    "Waiting"
                } else {
                    "N/A"
                },
                FontId::proportional(10.0),
                ui.visuals().weak_text_color(),
            );
        }
        if expanded {
            painter.text(
                egui::pos2(plot.left(), rect.bottom() - 8.0),
                Align2::LEFT_CENTER,
                format!("−{duration:.0}s"),
                FontId::proportional(9.0),
                ui.visuals().weak_text_color(),
            );
            painter.text(
                egui::pos2(plot.right(), rect.bottom() - 8.0),
                Align2::RIGHT_CENTER,
                "now",
                FontId::proportional(9.0),
                ui.visuals().weak_text_color(),
            );
        }
        if !expanded {
            painter.text(
                egui::pos2(rect.center().x, rect.bottom() - 10.),
                Align2::CENTER_CENTER,
                metric.unit().format(series.current()),
                FontId::proportional(11.),
                ui.visuals().text_color(),
            );
        }
        let hovered = response.hover_pos().map(|pointer| {
            let seconds = start
                + f64::from(((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0)) * duration;
            let sample = series.nearest(seconds);
            if let Some(sample) = sample
                && let Some(value) = sample.value
            {
                let point = position(sample, value);
                painter.line_segment(
                    [
                        egui::pos2(point.x, plot.top()),
                        egui::pos2(point.x, plot.bottom()),
                    ],
                    Stroke::new(1.0, color.gamma_multiply(0.6)),
                );
                painter.circle_filled(point, 3.5, color);
            }
            (seconds, sample)
        });
        response.on_hover_ui(|ui| {
            ui.set_max_width(335.0);
            ui.colored_label(color, egui::RichText::new(metric.label()).strong());
            if let Some((seconds, sample)) = hovered {
                if let Some(sample) = sample {
                    ui.strong(metric.unit().format(sample.value));
                    ui.small(format!("At {} · {:.0}s ago", elapsed(sample.seconds), self.seconds - sample.seconds));
                } else {
                    ui.label("No sample at this time");
                    ui.small(format!("At {}", elapsed(seconds)));
                }
            }
            ui.label(format!("Latest: {}", metric.unit().format(series.current())));
            let (low, high) = series.extrema.map_or((None, None), |(lo, hi)| (Some(lo), Some(hi)));
            ui.label(format!("Session low: {}", metric.unit().format(low)));
            ui.label(format!("Session high: {}", metric.unit().format(high)));
            ui.small(format!("Plot scale: 0 – {}", metric.unit().format(Some(ceiling))));
            ui.separator();
            ui.label(metric.description());
            ui.small("Sampled at 1 Hz. Low/high include the whole open session, including samples older than this graph. Unavailable samples do not change the extrema.");
        })
    }
}

fn elapsed(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hovering_a_rendered_graph_shows_sample_and_full_session_extrema() {
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let mut history = History {
            seconds: 240.0,
            ..Default::default()
        };
        let series = &mut history.series[Metric::Cpu as usize];
        series.record(0.0, Some(90.0));
        series.record(1.0, Some(10.0));
        for second in 2..=240 {
            series.record(f64::from(second), Some(25.0));
        }
        let mut rect = Rect::NOTHING;
        let mut draw = |time, events| {
            crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 640.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        rect = history
                            .chart(ui, Metric::Cpu, Vec2::new(280.0, 110.0), true)
                            .rect;
                    });
                },
            )
        };
        let idle = draw(0.0, vec![]);
        // Position is the center of the first chart in the central panel.
        let position = egui::pos2(148.0, 63.0);
        let _ = draw(0.1, vec![egui::Event::PointerMoved(position)]);
        let hover = draw(1.0, vec![]);
        fn contains(shape: &egui::Shape, needle: &str) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.text().contains(needle),
                egui::Shape::Vec(shapes) => shapes.iter().any(|s| contains(s, needle)),
                _ => false,
            }
        }
        assert!(rect.contains(position));
        assert!(
            !idle
                .shapes
                .iter()
                .any(|s| contains(&s.shape, "Session high"))
        );
        for text in [
            "25.00%",
            "Session low: 10.00%",
            "Session high: 90.00%",
            "At 00:03:00",
        ] {
            assert!(
                hover.shapes.iter().any(|s| contains(&s.shape, text)),
                "Missing hover text: {text}"
            );
        }
    }

    #[test]
    fn six_footer_graphs_and_button_fit_the_minimum_window_width() {
        let ctx = egui::Context::default();
        let history = History::default();
        for width in [960.0, 1280.0] {
            let mut bounds = Rect::NOTHING;
            let output = crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 640.0))),
                    ..Default::default()
                },
                |root| {
                    egui::Panel::bottom("status")
                        .frame(egui::Frame::new().inner_margin(10.0))
                        .show(root, |ui| {
                            bounds = ui
                                .horizontal_wrapped(|ui| history.footer(ui, "Test adapter"))
                                .response
                                .rect;
                        });
                },
            );
            assert!(bounds.width() <= width - 20.0);
            assert!(bounds.height() <= 58.0, "footer wrapped at width {width}");
            assert!(!output.shapes.is_empty());
        }
    }
    #[test]
    fn footer_displays_latest_numbers_and_units_without_hover() {
        fn contains(shape: &egui::Shape, needle: &str) -> bool {
            match shape {
                egui::Shape::Text(t) => t.galley.text().contains(needle),
                egui::Shape::Vec(s) => s.iter().any(|s| contains(s, needle)),
                _ => false,
            }
        }
        let ctx = egui::Context::default();
        let mut history = History::default();
        history.series[Metric::Fps as usize].record(1., Some(59.8));
        history.series[Metric::Cpu as usize].record(1., Some(12.5));
        history.series[Metric::Ram as usize].record(1., Some(104857600.));
        let output = crate::run_test_ui(&ctx, egui::RawInput::default(), |root| {
            root.horizontal(|ui| history.footer(ui, "GPU"));
        });
        for (metric, value) in [
            (Metric::Fps, 59.8),
            (Metric::Cpu, 12.5),
            (Metric::Ram, 104857600.),
        ] {
            let label = metric.unit().format(Some(value));
            assert!(
                output.shapes.iter().any(|s| contains(&s.shape, &label)),
                "Missing visible number {label}"
            );
        }
    }

    #[test]
    fn graph_categories_stay_open_for_interaction_and_outside_click_dismisses() {
        let ctx = egui::Context::default();
        let history = History::default();
        let draw = |time, events| {
            crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 640.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::Panel::bottom("status")
                        .frame(egui::Frame::new().inner_margin(10.0))
                        .show(root, |ui| {
                            ui.horizontal_wrapped(|ui| history.footer(ui, "Test adapter"));
                        });
                },
            )
        };
        fn label(shape: &egui::Shape, name: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == name => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| label(s, name)),
                _ => None,
            }
        }
        let position = |output: &egui::FullOutput, name| {
            output.shapes.iter().find_map(|s| label(&s.shape, name))
        };
        let click = |time: f64, pos| {
            let _ = draw(
                time,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let _ = draw(
                time + 0.1,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            draw(time + 0.4, vec![])
        };
        let initial = draw(0.0, vec![]);
        let expanded = click(0.2, position(&initial, "Graphs").unwrap());
        assert!(position(&expanded, "Performance · this session").is_some());
        let collapsed = click(1.0, position(&expanded, "Overview").unwrap());
        assert!(
            position(&collapsed, "Performance · this session").is_some(),
            "A category click must not dismiss the graphs"
        );
        assert!(position(&collapsed, "Frame timing").is_some());
        let closed = click(2.0, egui::pos2(5.0, 5.0));
        assert!(position(&closed, "Performance · this session").is_none());
    }

    #[test]
    fn rolling_history_retains_lifetime_extrema_and_ignores_missing_values() {
        let mut series = Series::default();
        series.record(0.0, Some(900.0));
        series.record(1.0, Some(0.0));
        for second in 2..10000 {
            series.record(f64::from(second), Some(20.0));
        }
        assert_eq!(series.samples.len(), MAX_SAMPLES);
        assert_eq!(series.extrema, Some((0.0, 900.0)));
        assert!(series.ceiling(Unit::Count) < 25.0);
        for (second, value) in [
            (10000.0, None),
            (10001.0, Some(f64::NAN)),
            (10002.0, Some(f64::INFINITY)),
            (10003.0, Some(-1.0)),
        ] {
            series.record(second, value);
        }
        assert_eq!(series.current(), None);
        assert_eq!(series.extrema, Some((0.0, 900.0)));
        series.record(11000.0, Some(10.0));
        assert_eq!(series.samples.len(), 1);
        assert_eq!(series.extrema, Some((0.0, 900.0)));
        assert_eq!(Series::default().extrema, None);
    }

    #[test]
    fn hover_picks_real_samples_without_inventing_values_across_a_gap() {
        let mut series = Series::default();
        series.record(10.0, Some(25.0));
        series.record(11.0, None);
        series.record(20.0, Some(60.0));
        assert_eq!(series.nearest(10.1).unwrap().value, Some(25.0));
        assert_eq!(series.nearest(11.1).unwrap().value, None);
        assert!(series.nearest(15.0).is_none());
        assert_eq!(series.nearest(20.0).unwrap().value, Some(60.0));
        series.record(20.0, Some(999.0));
        series.record(19.0, Some(999.0));
        assert_eq!(series.extrema, Some((25.0, 60.0)));
        assert!(Series::default().ceiling(Unit::Percent) > 0.0);
    }

    #[test]
    fn tracking_disconnect_and_receiver_reset_preserve_session_highs() {
        let mut history = History::default();
        let usage = Usage {
            managed_ram_bytes: Some(1048576),
            ..Default::default()
        };
        let snapshot = aria_tracking::Snapshot {
            received_at: Some(Instant::now()),
            packets_per_second: 60.0,
            packets: 100,
            ..Default::default()
        };
        history.record_at(1.0, &usage, &snapshot, 60.0, Some(0.002));
        history.record_at(
            2.0,
            &Usage::default(),
            &aria_tracking::Snapshot::default(),
            0.0,
            None,
        );
        assert_eq!(
            history.series[Metric::TrackingRate as usize].current(),
            None
        );
        assert_eq!(
            history.series[Metric::TrackingRate as usize].extrema,
            Some((60.0, 60.0))
        );
        assert_eq!(
            history.series[Metric::Packets as usize].extrema,
            Some((0.0, 100.0))
        );
        assert_eq!(
            history.series[Metric::Ram as usize].extrema,
            Some((1048576.0, 1048576.0))
        );
        assert_eq!(
            history.series[Metric::UiWork as usize]
                .extrema
                .unwrap()
                .0
                .round(),
            2.0
        );
        assert_eq!(
            History::default().series[Metric::Ram as usize].extrema,
            None
        );
        assert_eq!(Unit::Bytes.format(Some(1048576.0)), "1.00 MiB");
        assert_eq!(Unit::BytesPerSecond.format(Some(1024.0)), "1.00 KiB/s");
        assert_eq!(Unit::Percent.format(None), "N/A");
        assert_eq!(Unit::Count.format(Some(0.0)), "0");
    }
}
