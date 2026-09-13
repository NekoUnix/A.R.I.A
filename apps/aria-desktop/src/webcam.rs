//! Camera inference lives in an owned process; rendering consumes one latest frame.
use aria_tracking::Snapshot;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub device: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub confidence: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            device: 0,
            width: 640,
            height: 480,
            fps: 30,
            confidence: 0.5,
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Runtime {
    pub python: String,
    pub setup_python: String,
    pub sdk: String,
}
fn home() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ARIA/tracking")
}
pub fn assets() -> PathBuf {
    let beside = std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("tracking");
    if beside.join("worker.py").is_file() {
        beside
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tracking")
    }
}
impl Runtime {
    fn python(&self) -> PathBuf {
        if self.python.is_empty() {
            home().join("Scripts/python.exe")
        } else {
            PathBuf::from(&self.python)
        }
    }
    fn model(&self) -> PathBuf {
        self.python()
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("."))
            .join("face_landmarker.task")
    }
}
fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.stdin(Stdio::null());
}
#[cfg(windows)]
struct ProcessGroup(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl ProcessGroup {
    fn new(child: &Child) -> anyhow::Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{Foundation::HANDLE, System::JobObjects::*};
        unsafe {
            let group = Self(CreateJobObjectW(None, None)?);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                group.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )?;
            AssignProcessToJobObject(group.0, HANDLE(child.as_raw_handle()))?;
            Ok(group)
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
struct Worker {
    child: Child,
    readers: Vec<thread::JoinHandle<()>>,
    state: Arc<Mutex<Snapshot>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}
struct Job {
    child: Child,
    output: Arc<Mutex<String>>,
    reader: Option<thread::JoinHandle<()>>,
    devices: bool,
    install: bool,
    #[cfg(windows)]
    group: Option<ProcessGroup>,
}
impl Drop for Job {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            self.group = None;
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(r) = self.reader.take() {
            let _ = r.join();
        }
    }
}
#[derive(Default)]
pub struct Camera {
    worker: Option<Worker>,
    job: Option<Job>,
    pub message: Option<String>,
    devices: Vec<Device>,
}
#[derive(Deserialize)]
struct Device {
    index: u32,
    name: String,
}
impl Camera {
    pub fn running(&self) -> bool {
        self.worker.is_some()
    }
    pub fn stop(&mut self) {
        self.worker = None;
    }
    pub fn snapshot(&mut self) -> Snapshot {
        let Some(worker) = &mut self.worker else {
            return Snapshot::default();
        };
        let mut snapshot = worker.state.lock().unwrap().clone();
        if snapshot.packets > 0
            && self
                .message
                .as_ref()
                .is_some_and(|m| m.starts_with("Starting camera"))
        {
            self.message = Some("Camera inference is running locally.".into());
        }
        if let Ok(Some(status)) = worker.child.try_wait() {
            snapshot.frame = None;
            snapshot.received_at = None;
            self.message = Some(snapshot.last_error.clone().unwrap_or_else(|| {
                format!(
                    "Camera worker exited ({status}). Check the runtime and camera, then retry."
                )
            }));
            self.worker = None;
        }
        snapshot
    }
    pub fn start(
        &mut self,
        settings: &Settings,
        runtime: &Runtime,
        rtx: bool,
        ctx: &egui::Context,
    ) -> anyhow::Result<()> {
        self.stop();
        anyhow::ensure!(
            runtime.python().is_file(),
            "Install the webcam runtime first, or select its Scripts/python.exe below."
        );
        anyhow::ensure!(
            rtx || runtime.model().is_file(),
            "Face model is missing. Run Install webcam runtime to repair it."
        );
        let mut command = Command::new(runtime.python());
        command
            .arg("-u")
            .arg(assets().join("worker.py"))
            .args(["--backend", if rtx { "nvidia" } else { "mediapipe" }])
            .arg("--device")
            .arg(settings.device.to_string())
            .arg("--width")
            .arg(settings.width.to_string())
            .arg("--height")
            .arg(settings.height.to_string())
            .arg("--fps")
            .arg(settings.fps.to_string())
            .arg("--confidence")
            .arg(settings.confidence.to_string())
            .arg("--model")
            .arg(runtime.model())
            .arg("--sdk")
            .arg(&runtime.sdk)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hidden(&mut command);
        let mut child = command.spawn()?;
        let state = Arc::new(Mutex::new(Snapshot::default()));
        let data = state.clone();
        let errors = state.clone();
        let repaint = ctx.clone();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let reader = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut bytes = Vec::with_capacity(8192);
            loop {
                bytes.clear();
                let result = reader.by_ref().take(16385).read_until(b'\n', &mut bytes);
                if !matches!(result,Ok(n) if n>0) {
                    break;
                }
                let mut s = data.lock().unwrap();
                match decode(&bytes) {
                    Ok(frame) => {
                        let now = Instant::now();
                        if let Some(t) = s.received_at {
                            s.packets_per_second =
                                1. / now.duration_since(t).as_secs_f32().max(0.001);
                        }
                        s.frame = Some(frame);
                        s.received_at = Some(now);
                        s.packets += 1;
                    }
                    Err(e) => {
                        s.rejected += 1;
                        s.frame = None;
                        s.last_error = Some(e.to_string());
                    }
                }
                drop(s);
                repaint.request_repaint();
                if bytes.len() > 16384 {
                    break;
                }
            }
        });
        let reader2 = thread::spawn(move || {
            let mut r = BufReader::new(stderr);
            loop {
                let mut b = Vec::new();
                if !matches!(r.by_ref().take(2048).read_until(b'\n',&mut b),Ok(n) if n>0) {
                    break;
                }
                errors.lock().unwrap().last_error =
                    Some(String::from_utf8_lossy(&b).trim().to_owned());
            }
        });
        self.worker = Some(Worker {
            child,
            readers: vec![reader, reader2],
            state,
        });
        self.message =
            Some("Starting camera inference… first model load can take a moment.".into());
        Ok(())
    }
    fn job(&mut self, mut command: Command, devices: bool) -> anyhow::Result<()> {
        hidden(&mut command);
        command.stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = command.spawn()?;
        #[cfg(windows)]
        let group = match ProcessGroup::new(&child) {
            Ok(g) => Some(g),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        let stdout = child.stdout.take().unwrap();
        let output = Arc::new(Mutex::new(String::new()));
        let dst = output.clone();
        let reader = thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            loop {
                let mut b = Vec::new();
                if !matches!(r.by_ref().take(8192).read_until(b'\n',&mut b),Ok(n) if n>0) {
                    break;
                }
                *dst.lock().unwrap() = String::from_utf8_lossy(&b).trim().to_owned();
            }
        });
        self.job = Some(Job {
            child,
            output,
            reader: Some(reader),
            devices,
            install: false,
            #[cfg(windows)]
            group,
        });
        Ok(())
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut Settings,
        runtime: &mut Runtime,
        rtx: bool,
    ) {
        crate::help::label(
            ui,
            if rtx {
                "NVIDIA RTX face tracking"
            } else {
                "Webcam face tracking"
            },
            "webcam",
        );
        if let Some(job) = &mut self.job {
            ui.spinner();
            ui.label(if job.devices {
                "Finding cameras…"
            } else {
                "Installing runtime… internet required. This can take several minutes."
            });
            if let Ok(Some(status)) = job.child.try_wait() {
                if let Some(r) = job.reader.take() {
                    let _ = r.join();
                }
                let result = job.output.lock().unwrap().clone();
                if status.success() && job.devices {
                    match serde_json::from_str(&result) {
                        Ok(devices) => {
                            self.devices = devices;
                            self.message = Some("Choose a camera below.".into());
                        }
                        Err(_) => {
                            self.message =
                                Some("Device enumeration returned no usable list.".into())
                        }
                    }
                } else {
                    self.message = Some(if status.success() {
                        if job.install {
                            runtime.python.clear();
                        }
                        if result.is_empty() {
                            "Setup completed. Choose a camera and press Start camera.".into()
                        } else {
                            result
                        }
                    } else {
                        format!(
                            "Setup failed. {result} Check Python 3.12, network access and docs/webcam.md."
                        )
                    });
                }
                self.job = None;
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        ui.add_enabled_ui(!self.running() && self.job.is_none(),|ui|{
            ui.horizontal(|ui|{ui.label("Camera index");ui.add(egui::DragValue::new(&mut settings.device).range(0..=63));});
            if !self.devices.is_empty(){egui::ComboBox::from_id_salt("webcam-device").selected_text(self.devices.iter().find(|d|d.index==settings.device).map_or("Choose camera",|d|d.name.as_str())).show_ui(ui,|ui|{for d in &self.devices{ui.selectable_value(&mut settings.device,d.index,&d.name);}});}
            if ui.button("Find cameras").clicked(){let mut c=Command::new(runtime.python());c.arg(assets().join("worker.py")).arg("--list");if let Err(e)=self.job(c,true){self.message=Some(e.to_string());}}
            egui::ComboBox::from_id_salt("webcam-resolution").selected_text(format!("{} × {}",settings.width,settings.height)).show_ui(ui,|ui|{
                for (w,h) in [(320,240),(640,480),(1280,720),(1920,1080)]{if ui.selectable_label(settings.width==w,format!("{w} × {h}")).clicked(){settings.width=w;settings.height=h;}}
            });
            ui.add(egui::Slider::new(&mut settings.fps,5..=60).text("Camera FPS"));
            if !rtx {ui.add(egui::Slider::new(&mut settings.confidence,0.1..=0.99).text("Confidence"));}
            if rtx {
                ui.label("NVIDIA AR SDK folder"); ui.text_edit_singleline(&mut runtime.sdk);
                if ui.button("Choose SDK folder…").clicked() && let Some(p)=rfd::FileDialog::new().pick_folder(){runtime.sdk=p.display().to_string();}
                ui.hyperlink_to("NVIDIA SDK setup & requirements","https://docs.nvidia.com/maxine/ar/latest/WindowsARSDK/InstalltheARSDK.html");
                if ui.button("Build RTX bridge for this SDK").clicked(){
                    let mut c=Command::new("powershell.exe");c.args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File"]).arg(assets().parent().unwrap().join("scripts/build-nvidia-bridge.ps1")).arg("-Sdk").arg(&runtime.sdk);
                    if let Err(e)=self.job(c,false){self.message=Some(e.to_string());}
                }
                crate::theme::caption(ui,"Requires the AR SDK, FaceExpressions feature for your GPU, and the ARIA NVIDIA bridge. NVIDIA Broadcast alone does not install this SDK. See the ? guide.");
            }
            if ui.button("Start camera").clicked() && let Err(e)=self.start(settings,runtime,rtx,ui.ctx()){self.message=Some(e.to_string());}
        });
        if self.running() && ui.button("Stop camera").clicked() {
            self.stop();
            self.message = Some("Camera stopped and released.".into());
        }
        if let Some(message) = &self.message {
            crate::theme::caption(ui, message);
        }
        crate::theme::caption(
            ui,
            "Camera frames stay on this PC and are not recorded. Start is always explicit; changing avatar or source stops the camera. Then use Guided tracking setup for your face.",
        );
        egui::CollapsingHeader::new("Install / repair camera runtime").show(ui,|ui|{
            ui.hyperlink_to("1. Install Python 3.12 x64","https://www.python.org/downloads/windows/");
            ui.label("2. Install local dependencies (MediaPipe + OpenCV)");
            ui.label("Python for setup (blank uses py -3.12)");ui.text_edit_singleline(&mut runtime.setup_python);
            if ui.add_enabled(self.job.is_none()&&!self.running(),egui::Button::new("Install webcam runtime")).clicked(){
                let mut c=Command::new("powershell.exe");c.args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File"]).arg(assets().parent().unwrap().join("scripts/setup-webcam.ps1"));
                if !runtime.setup_python.is_empty(){c.arg("-Python").arg(&runtime.setup_python);}
                match self.job(c,false){Err(e)=>self.message=Some(e.to_string()),Ok(())=>{if let Some(job)=&mut self.job{job.install=true;}}}
            }
            ui.label("Advanced: existing runtime Scripts/python.exe");ui.text_edit_singleline(&mut runtime.python);
            if ui.button("Choose runtime interpreter…").clicked() && let Some(p)=rfd::FileDialog::new().add_filter("Python executable", &["exe"]).pick_file(){runtime.python=p.display().to_string();}
            crate::theme::caption(ui,"Blank uses %LOCALAPPDATA%/ARIA/tracking. Install once per PC; camera device, resolution, FPS and confidence belong to each avatar.");
        });
    }
}
fn decode(bytes: &[u8]) -> anyhow::Result<aria_core::TrackingFrame> {
    anyhow::ensure!(bytes.len() <= 16384, "Oversized camera frame");
    #[derive(Deserialize)]
    struct Packet {
        version: u32,
        #[serde(flatten)]
        frame: aria_core::TrackingFrame,
    }
    let mut packet: Packet = serde_json::from_slice(bytes)?;
    anyhow::ensure!(
        packet.version == 1
            && packet.frame.is_finite()
            && packet.frame.blend_shapes.len() <= 128
            && packet.frame.blend_shapes.keys().all(|k| k.len() <= 64),
        "Invalid camera frame"
    );
    packet.frame.normalize();
    Ok(packet.frame)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_nonfinite_and_oversized_camera_data() {
        assert!(decode(&vec![b' '; 16385]).is_err());
        assert!(decode(br#"{"version":2}"#).is_err());
        let f = aria_core::demo_frame(1.);
        let mut v = serde_json::to_value(&f).unwrap();
        v["version"] = 1.into();
        let frame = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
        assert!(frame.is_finite());
    }
}
