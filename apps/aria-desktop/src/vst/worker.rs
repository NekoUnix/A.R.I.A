use super::*;
use anyhow::{Context, Result, ensure};
use rodio::Source;
use std::{
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const BLOCK: usize = 512;
const MAX_PACKET: usize = 1024 * 1024;
const MAGIC: &[u8; 8] = b"ARIAVST1";
#[derive(Serialize, Deserialize)]
enum Request {
    Open {
        settings: Settings,
        rate: u32,
        channels: u16,
    },
    Process {
        samples: Vec<f32>,
        parameters: BTreeMap<u32, f64>,
    },
}
#[derive(Serialize, Deserialize)]
enum Response {
    Opened(Info),
    Audio(Vec<f32>),
    Error(String),
}
fn write_packet(writer: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let data = serde_json::to_vec(value)?;
    ensure!(data.len() <= MAX_PACKET, "VST worker packet exceeds limit");
    writer.write_all(MAGIC)?;
    writer.write_all(&(data.len() as u32).to_le_bytes())?;
    writer.write_all(&data)?;
    writer.flush()?;
    Ok(())
}
fn read_packet<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> Result<T> {
    let mut header = [0; 12];
    reader.read_exact(&mut header)?;
    ensure!(&header[..8] == MAGIC, "Invalid VST worker protocol");
    let size = u32::from_le_bytes(header[8..].try_into().unwrap()) as usize;
    ensure!(
        (1..=MAX_PACKET).contains(&size),
        "Invalid VST worker packet size"
    );
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
struct Client {
    // Reuse the camera helper's kill-on-close job so a parent crash cannot orphan a plugin.
    _group: crate::webcam::ProcessGroup,
    child: Arc<Mutex<Child>>,
    input: BufWriter<ChildStdin>,
    replies: mpsc::Receiver<Result<Response>>,
    clock: Instant,
    deadline: Arc<AtomicU64>,
    watch_stop: Arc<AtomicBool>,
}
impl Client {
    fn spawn(executable: &Path, cancel: Arc<AtomicBool>) -> Result<Self> {
        use std::os::windows::process::CommandExt;
        let mut child = Command::new(executable)
            .arg("--vst-worker")
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let group = match crate::webcam::ProcessGroup::new(&child) {
            Ok(group) => group,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.context("Cannot contain VST worker lifetime"));
            }
        };
        let input = BufWriter::new(child.stdin.take().context("Missing VST worker input")?);
        let output = child.stdout.take().context("Missing VST worker output")?;
        let child = Arc::new(Mutex::new(child));
        let (tx, replies) = mpsc::sync_channel(1);
        let clock = Instant::now();
        let deadline = Arc::new(AtomicU64::new(0));
        let watch_stop = Arc::new(AtomicBool::new(false));
        let result = Self {
            _group: group,
            child,
            input,
            replies,
            clock,
            deadline,
            watch_stop,
        };
        thread::Builder::new()
            .name("aria-vst-replies".into())
            .spawn(move || {
                let mut reader = BufReader::new(output);
                loop {
                    let response = read_packet(&mut reader);
                    let failed = response.is_err();
                    if tx.send(response).is_err() || failed {
                        break;
                    }
                }
            })?;
        let child = result.child.clone();
        let deadline = result.deadline.clone();
        let stopped = result.watch_stop.clone();
        thread::Builder::new()
            .name("aria-vst-watchdog".into())
            .spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    let end = deadline.load(Ordering::Relaxed);
                    if cancel.load(Ordering::Relaxed)
                        || (end != 0 && clock.elapsed().as_millis() as u64 >= end)
                    {
                        let _ = child.lock().unwrap_or_else(|e| e.into_inner()).kill();
                        break;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
            })?;
        Ok(result)
    }
    fn call(&mut self, request: &Request, timeout: Duration) -> Result<Response> {
        self.deadline.store(
            self.clock.elapsed().as_millis() as u64 + timeout.as_millis() as u64,
            Ordering::Relaxed,
        );
        // A separate watchdog also terminates a child stuck before reading its pipe.
        write_packet(&mut self.input, request)?;
        let response = self
            .replies
            .recv_timeout(timeout)
            .context("VST worker stopped or timed out")??;
        self.deadline.store(0, Ordering::Relaxed);
        match response {
            Response::Error(message) => anyhow::bail!("{message}"),
            other => Ok(other),
        }
    }
    fn open(&mut self, settings: Settings, rate: u32, channels: u16) -> Result<Info> {
        match self.call(
            &Request::Open {
                settings,
                rate,
                channels,
            },
            Duration::from_secs(15),
        )? {
            Response::Opened(info) => Ok(info),
            _ => anyhow::bail!("Unexpected VST initialization response"),
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.watch_stop.store(true, Ordering::Relaxed);
        let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}
pub(super) fn probe(path: &Path) -> Result<Info> {
    let mut client = Client::spawn(&std::env::current_exe()?, Arc::new(AtomicBool::new(false)))?;
    client.open(
        Settings {
            path: Some(path.into()),
            ..Default::default()
        },
        48000,
        2,
    )
}

pub(super) fn serve() -> Result<()> {
    let mut input = BufReader::new(std::io::stdin().lock());
    let mut output = BufWriter::new(std::io::stdout().lock());
    let mut plugin: Option<vst3_host::Plugin> = None;
    let mut buffers = vst3_host::AudioBuffers::new(2, 2, BLOCK, 48000.);
    let mut applied = BTreeMap::new();
    let mut editable = std::collections::BTreeSet::new();
    let mut latency = 0;
    loop {
        let request = match read_packet(&mut input) {
            Ok(request) => request,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::UnexpectedEof) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let result: Result<Response> = (|| match request {
            Request::Open {
                mut settings,
                rate,
                channels,
            } => {
                plugin = None;
                applied.clear();
                settings.sanitize();
                ensure!(
                    (8000..=192000).contains(&rate) && (1..=2).contains(&channels),
                    "VST music requires mono/stereo, 8–192 kHz"
                );
                let path = settings.path.context("Choose a VST3 effect")?;
                ensure!(
                    path.extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("vst3"))
                        && path.exists(),
                    "Select an existing .vst3 file or bundle"
                );
                let mut host = vst3_host::Vst3Host::builder()
                    .sample_rate(f64::from(rate))
                    .block_size(BLOCK)
                    .input_channels(usize::from(channels))
                    .output_channels(usize::from(channels))
                    .with_process_isolation(false)
                    .build()?;
                let mut loaded = if settings.class_id.is_empty() {
                    host.load_plugin(&path)?
                } else {
                    host.load_plugin_class(&path, &settings.class_id)?
                };
                let mut layout = loaded.audio_bus_layout()?;
                if layout.inputs.len() == 1
                    && layout.outputs.len() == 1
                    && (layout.inputs[0].channel_count != usize::from(channels)
                        || layout.outputs[0].channel_count != usize::from(channels))
                {
                    let arrangement = if channels == 1 {
                        vst3_host::audio::SpeakerArrangement::MONO
                    } else {
                        vst3_host::audio::SpeakerArrangement::STEREO
                    };
                    loaded.set_bus_arrangements(&[arrangement], &[arrangement])?;
                    layout = loaded.audio_bus_layout()?;
                }
                ensure!(
                    layout.inputs.len() == 1
                        && layout.outputs.len() == 1
                        && layout.inputs[0].channel_count == usize::from(channels)
                        && layout.outputs[0].channel_count == usize::from(channels),
                    "This effect's bus layout is unsupported; use one matching mono/stereo input and output"
                );
                editable = loaded
                    .get_parameters()?
                    .into_iter()
                    .filter(|p| !p.is_read_only)
                    .map(|p| p.id)
                    .collect();
                for (&id, &value) in &settings.parameters {
                    if editable.contains(&id) {
                        loaded.set_parameter(id, value)?;
                    }
                }
                loaded.start_processing()?;
                latency = loaded.latency_samples();
                ensure!(latency <= rate * 2, "Plugin latency exceeds two seconds");
                let info = Info {
                    name: loaded.info().name.chars().take(160).collect(),
                    class_id: loaded.info().uid.clone(),
                    latency,
                    tail: loaded.tail_samples().min(rate * 5),
                    parameters: loaded
                        .get_parameters()?
                        .into_iter()
                        .take(256)
                        .filter(|p| p.value.is_finite())
                        .map(|p| Parameter {
                            id: p.id,
                            name: p.name.chars().take(100).collect(),
                            value: p.value.clamp(0., 1.),
                            read_only: p.is_read_only,
                            steps: p.step_count,
                        })
                        .collect(),
                };
                buffers = vst3_host::AudioBuffers::new(
                    usize::from(channels),
                    usize::from(channels),
                    BLOCK,
                    f64::from(rate),
                );
                applied = settings.parameters;
                plugin = Some(loaded);
                Ok(Response::Opened(info))
            }
            Request::Process {
                samples,
                parameters,
            } => {
                let plugin = plugin.as_mut().context("VST plugin is not loaded")?;
                let channels = buffers.inputs.len();
                ensure!(
                    samples.len() == BLOCK * channels && samples.iter().all(|s| s.is_finite()),
                    "Invalid VST input block"
                );
                ensure!(
                    parameters.len() <= 256
                        && parameters
                            .values()
                            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                    "Invalid VST parameters"
                );
                for (&id, &value) in &parameters {
                    if editable.contains(&id) && applied.get(&id) != Some(&value) {
                        plugin.set_parameter(id, value)?;
                    }
                }
                applied = parameters;
                for channel in 0..channels {
                    buffers.outputs[channel].fill(0.);
                    for frame in 0..BLOCK {
                        buffers.inputs[channel][frame] = samples[frame * channels + channel];
                    }
                }
                plugin.process_audio(&mut buffers)?;
                ensure!(
                    plugin.latency_samples() == latency,
                    "Plugin latency changed; stop and restart playback to apply the new latency"
                );
                let mut samples = Vec::with_capacity(BLOCK * channels);
                for frame in 0..BLOCK {
                    for channel in 0..channels {
                        let value = buffers.outputs[channel][frame];
                        ensure!(value.is_finite(), "Plugin produced nonfinite audio");
                        samples.push(value.clamp(-1., 1.));
                    }
                }
                Ok(Response::Audio(samples))
            }
        })();
        write_packet(
            &mut output,
            &result
                .unwrap_or_else(|e| Response::Error(format!("{e:#}").chars().take(1000).collect())),
        )?;
    }
}

struct Stream {
    receiver: mpsc::Receiver<Vec<f32>>,
    buffer: std::vec::IntoIter<f32>,
    channels: std::num::NonZeroU16,
    rate: std::num::NonZeroU32,
    duration: Option<Duration>,
    control: Control,
    ended: bool,
    silence_remaining: usize,
}
impl Iterator for Stream {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.control.cancel.load(Ordering::Relaxed) || self.ended {
            return None;
        }
        if let Some(value) = self.buffer.next() {
            return Some(value);
        }
        if self.silence_remaining > 0 {
            self.silence_remaining -= 1;
            return Some(0.);
        }
        match self.receiver.try_recv() {
            Ok(block) => {
                self.buffer = block.into_iter();
                self.buffer.next()
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.control.underruns.fetch_add(1, Ordering::Relaxed);
                // Silence must be a complete frame; never shift stereo channel alignment.
                self.silence_remaining = usize::from(self.channels.get()) - 1;
                Some(0.)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ended = true;
                None
            }
        }
    }
}
impl Source for Stream {
    fn current_span_len(&self) -> Option<usize> {
        Some(if self.ended {
            0
        } else {
            BLOCK * usize::from(self.channels.get())
        })
    }
    fn channels(&self) -> std::num::NonZeroU16 {
        self.channels
    }
    fn sample_rate(&self) -> std::num::NonZeroU32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.control.stop();
    }
}
pub(super) fn stream(source: AudioSource, control: Control) -> Result<AudioSource> {
    stream_with_executable(source, control, std::env::current_exe()?)
}
fn stream_with_executable(
    mut source: AudioSource,
    control: Control,
    executable: PathBuf,
) -> Result<AudioSource> {
    let channels = source.channels();
    let rate = source.sample_rate();
    let duration = source.total_duration();
    let (tx, receiver) = mpsc::sync_channel(8);
    let (duration_tx, duration_rx) = mpsc::sync_channel(1);
    let worker_control = control.clone();
    thread::Builder::new()
        .name("aria-vst-audio".into())
        .spawn(move || {
            let result: Result<()> = (|| {
                let mut client = Client::spawn(&executable, worker_control.cancel.clone())?;
                let initial = worker_control
                    .settings
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let info = client.open(initial, rate.get(), channels.get())?;
                let end_samples = info.latency + info.tail;
                let _ = duration_tx.send(duration.map(|d| {
                    d + Duration::from_secs_f64(f64::from(end_samples) / f64::from(rate.get()))
                }));
                let size = BLOCK * usize::from(channels.get());
                let mut eof = false;
                let mut tail = 0;
                loop {
                    if worker_control.cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let mut samples = Vec::with_capacity(size);
                    if !eof {
                        for _ in 0..size {
                            if let Some(sample) = source.next() {
                                ensure!(sample.is_finite(), "Music contains nonfinite audio");
                                samples.push(sample);
                            } else {
                                eof = true;
                                tail = end_samples as usize * usize::from(channels.get());
                                break;
                            }
                        }
                    }
                    if eof {
                        let padding = tail.min(size - samples.len());
                        tail -= padding;
                        samples.resize(samples.len() + padding, 0.);
                    }
                    let valid = samples.len();
                    if valid == 0 {
                        break;
                    }
                    samples.resize(size, 0.);
                    let parameters = worker_control
                        .settings
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .parameters
                        .clone();
                    let Response::Audio(mut samples) = client.call(
                        &Request::Process {
                            samples,
                            parameters,
                        },
                        Duration::from_secs(2),
                    )?
                    else {
                        anyhow::bail!("Invalid VST audio response");
                    };
                    ensure!(
                        samples.len() == size
                            && samples.iter().all(|s| s.is_finite() && s.abs() <= 1.),
                        "Invalid VST output block"
                    );
                    samples.truncate(valid);
                    if !queue_block(&tx, &worker_control.cancel, samples) {
                        break;
                    }
                }
                Ok(())
            })();
            if let Err(error) = result
                && !worker_control.cancel.load(Ordering::Relaxed)
            {
                *worker_control
                    .error
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) =
                    Some(format!("VST music stopped: {error:#}"));
            }
        })?;
    let first = match receiver.recv_timeout(Duration::from_secs(17)) {
        Ok(first) => first,
        Err(error) => {
            control.stop();
            return Err(anyhow::anyhow!(control.take_error().unwrap_or_else(
                || format!("VST effect could not produce its first audio block: {error}")
            )));
        }
    };
    let duration = duration_rx.try_recv().unwrap_or(duration);
    Ok(Box::new(Stream {
        receiver,
        buffer: first.into_iter(),
        channels,
        rate,
        duration,
        control,
        ended: false,
        silence_remaining: 0,
    }))
}

// A paused or disconnected device must not trap the producer behind a full queue.
// Only this background thread waits; the audio callback always uses try_recv.
fn queue_block(
    tx: &mpsc::SyncSender<Vec<f32>>,
    cancel: &AtomicBool,
    mut samples: Vec<f32>,
) -> bool {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        match tx.try_send(samples) {
            Ok(()) => return true,
            Err(mpsc::TrySendError::Disconnected(_)) => return false,
            Err(mpsc::TrySendError::Full(block)) => samples = block,
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelling_a_full_audio_queue_releases_the_producer() {
        let (tx, _rx) = mpsc::sync_channel(1);
        tx.send(vec![0.]).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (done, finished) = mpsc::channel();
        let thread = thread::spawn(move || {
            done.send(queue_block(&tx, &flag, vec![1.])).unwrap();
        });
        cancel.store(true, Ordering::Relaxed);
        assert!(!finished.recv_timeout(Duration::from_secs(1)).unwrap());
        thread.join().unwrap();
    }
    #[test]
    fn audio_callback_preserves_frames_on_underrun_and_drains_before_eof() {
        let (tx, receiver) = mpsc::sync_channel(2);
        let control = Control::default();
        let mut stream = Stream {
            receiver,
            buffer: vec![0.1, 0.2].into_iter(),
            channels: 2.try_into().unwrap(),
            rate: 48000.try_into().unwrap(),
            duration: None,
            control: control.clone(),
            ended: false,
            silence_remaining: 0,
        };
        assert_eq!(stream.next(), Some(0.1));
        assert_eq!(stream.next(), Some(0.2));
        assert_eq!(stream.next(), Some(0.));
        tx.send(vec![0.3, 0.4]).unwrap();
        assert_eq!(
            stream.next(),
            Some(0.),
            "Finish silence frame before starting queued stereo audio"
        );
        drop(tx);
        assert_eq!(stream.next(), Some(0.3));
        assert_eq!(stream.next(), Some(0.4));
        assert_eq!(stream.next(), None);
        assert_eq!(control.underruns.load(Ordering::Relaxed), 1);
        assert_eq!(stream.current_span_len(), Some(0));
    }

    #[test]
    #[ignore = "requires built desktop executable and two locally installed VST3 effects; no audible output"]
    fn native_vst_rates_silence_impulse_parameters_missing_files_and_cancellation() {
        let exe =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/aria-desktop.exe");
        for name in ["LoudMax.vst3", "Elgato/ElgatoEq.vst3"] {
            for (rate, channels) in [
                (44100, 1),
                (44100, 2),
                (48000, 1),
                (48000, 2),
                (96000, 1),
                (96000, 2),
            ] {
                let path = PathBuf::from("C:/Program Files/Common Files/VST3").join(name);
                let cancel = Arc::new(AtomicBool::new(false));
                let mut client = Client::spawn(&exe, cancel.clone()).unwrap();
                let info = client
                    .open(
                        Settings {
                            path: Some(path.clone()),
                            ..Default::default()
                        },
                        rate,
                        channels,
                    )
                    .unwrap();
                eprintln!(
                    "{name} at {rate}, {channels} channels: {} samples latency, {} parameters",
                    info.latency,
                    info.parameters.len()
                );
                let mut parameters: BTreeMap<_, _> = info
                    .parameters
                    .iter()
                    .filter(|p| !p.read_only)
                    .map(|p| (p.id, p.value))
                    .collect();
                let adjustable = info
                    .parameters
                    .iter()
                    .find(|p| !p.read_only)
                    .expect("Test effect needs an editable parameter");
                let defaults = parameters.clone();
                parameters.insert(adjustable.id, if adjustable.value < 0.5 { 1. } else { 0. });
                let reopened = client
                    .open(
                        Settings {
                            path: Some(path.clone()),
                            class_id: info.class_id.clone(),
                            parameters: parameters.clone(),
                            ..Default::default()
                        },
                        rate,
                        channels,
                    )
                    .unwrap();
                for p in reopened.parameters.iter().filter(|p| !p.read_only) {
                    assert!(
                        (p.value - parameters[&p.id]).abs() < 1e-5,
                        "Parameter restore: {}",
                        p.name
                    );
                }
                parameters = defaults;
                let reopened = client
                    .open(
                        Settings {
                            path: Some(path),
                            class_id: info.class_id,
                            parameters: parameters.clone(),
                            ..Default::default()
                        },
                        rate,
                        channels,
                    )
                    .unwrap();
                let mut first_signal = None;
                let blocks = (reopened.latency as usize + rate as usize / 2) / BLOCK + 2;
                for block in 0..blocks {
                    let mut samples = vec![0.; BLOCK * usize::from(channels)];
                    if block == 0 {
                        samples[0] = 0.1;
                        if channels == 2 {
                            samples[1] = -0.1;
                        }
                    }
                    let Response::Audio(out) = client
                        .call(
                            &Request::Process {
                                samples,
                                parameters: parameters.clone(),
                            },
                            Duration::from_secs(2),
                        )
                        .unwrap()
                    else {
                        panic!("Expected audio")
                    };
                    assert_eq!(out.len(), BLOCK * usize::from(channels));
                    assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.));
                    if first_signal.is_none() {
                        first_signal = out
                            .chunks_exact(usize::from(channels))
                            .position(|v| v.iter().any(|s| s.abs() > 1e-6))
                            .map(|frame| block * BLOCK + frame);
                    }
                    if block >= blocks - 2 {
                        assert!(out.iter().all(|v| v.abs() < 1e-5), "Silence should settle");
                    }
                }
                assert_eq!(
                    first_signal,
                    Some(reopened.latency as usize),
                    "Reported impulse latency must match actual audio"
                );
                cancel.store(true, Ordering::Relaxed);
                let deadline = Instant::now() + Duration::from_secs(2);
                while client.child.lock().unwrap().try_wait().unwrap().is_none() {
                    assert!(
                        Instant::now() < deadline,
                        "Cancellation failed to stop child"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
        let mut client = Client::spawn(&exe, Arc::new(AtomicBool::new(false))).unwrap();
        assert!(
            client
                .open(
                    Settings {
                        path: Some("C:/nonexistent-aria-plugin.vst3".into()),
                        ..Default::default()
                    },
                    48000,
                    2
                )
                .is_err()
        );
        assert!(client.open(Settings::default(), 48000, 3).is_err());
        client.deadline.store(
            client.clock.elapsed().as_millis() as u64 + 10,
            Ordering::Relaxed,
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        while client.child.lock().unwrap().try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "Watchdog expiry failed to stop child"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[ignore = "requires built desktop executable and LoudMax; consumes buffered music without playing audio"]
    fn native_vst_buffered_music_drains_and_stops() {
        let exe =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/aria-desktop.exe");
        let control = Control::default();
        control.update(&Settings {
            path: Some("C:/Program Files/Common Files/VST3/LoudMax.vst3".into()),
            enabled: true,
            ..Default::default()
        });
        let samples: Vec<_> = (0..48000 * 2)
            .map(|n| (n as f32 * 0.01).sin() * 0.1)
            .collect();
        let source = rodio::buffer::SamplesBuffer::new(
            2.try_into().unwrap(),
            48000.try_into().unwrap(),
            samples,
        );
        let mut stream = stream_with_executable(Box::new(source), control.clone(), exe).unwrap();
        let mut count = 0;
        let mut peak = 0_f32;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut ended = false;
            for _ in 0..BLOCK * 2 {
                match stream.next() {
                    Some(v) => {
                        assert!(v.is_finite());
                        peak = peak.max(v.abs());
                        count += 1;
                    }
                    None => {
                        ended = true;
                        break;
                    }
                }
            }
            if ended {
                break;
            }
            assert!(Instant::now() < deadline, "Stream did not finish");
            thread::sleep(Duration::from_millis(11));
        }
        assert!(count >= 96000 && peak > 0.01);
        assert!(control.take_error().is_none());
        assert_eq!(
            control.underruns.load(Ordering::Relaxed),
            0,
            "Buffered producer must sustain playback"
        );
        drop(stream);
        assert!(control.cancel.load(Ordering::Relaxed));
    }
    #[test]
    fn packets_reject_wrong_magic_and_excessive_allocation() {
        let mut bad = Vec::from(*MAGIC);
        bad.extend((MAX_PACKET as u32 + 1).to_le_bytes());
        assert!(read_packet::<Request>(&mut bad.as_slice()).is_err());
        assert!(read_packet::<Request>(&mut [0u8; 12].as_slice()).is_err());
        let request = Request::Process {
            samples: vec![0.; BLOCK * 2],
            parameters: BTreeMap::new(),
        };
        let mut bytes = vec![];
        write_packet(&mut bytes, &request).unwrap();
        assert!(
            matches!(read_packet::<Request>(&mut bytes.as_slice()).unwrap(), Request::Process { samples, .. } if samples.len() == BLOCK * 2)
        );
    }
    #[test]
    #[ignore = "requires built desktop executable and local VST3 effect; never plays audio"]
    fn native_vst_worker_processes_loudmax_and_contains_worker_failure() {
        let exe =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/aria-desktop.exe");
        let path = PathBuf::from("C:/Program Files/Common Files/VST3/LoudMax.vst3");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut client = Client::spawn(&exe, cancel.clone()).unwrap();
        let info = client
            .open(
                Settings {
                    path: Some(path),
                    ..Default::default()
                },
                48000,
                2,
            )
            .unwrap();
        assert!(!info.parameters.is_empty() && !info.name.is_empty());
        let samples: Vec<_> = (0..BLOCK * 2)
            .map(|n| (n as f32 * 0.04).sin() * 0.2)
            .collect();
        for _ in 0..12 {
            let Response::Audio(output) = client
                .call(
                    &Request::Process {
                        samples: samples.clone(),
                        parameters: BTreeMap::new(),
                    },
                    Duration::from_secs(2),
                )
                .unwrap()
            else {
                panic!("Expected audio");
            };
            assert_eq!(output.len(), samples.len());
            assert!(output.iter().all(|v| v.is_finite() && v.abs() <= 1.));
        }
        let output_id = info
            .parameters
            .iter()
            .find(|p| p.name == "Output")
            .expect("LoudMax output parameter")
            .id;
        let mut peak = 1_f32;
        for _ in 0..12 {
            let Response::Audio(output) = client
                .call(
                    &Request::Process {
                        samples: samples.clone(),
                        parameters: BTreeMap::from([(output_id, 0.2)]),
                    },
                    Duration::from_secs(2),
                )
                .unwrap()
            else {
                panic!("Expected audio");
            };
            peak = output.iter().fold(0_f32, |max, value| max.max(value.abs()));
        }
        assert!(
            peak > 0. && peak < 0.05,
            "Live output parameter must audibly change samples: peak {peak}"
        );
        client.child.lock().unwrap().kill().unwrap();
        assert!(
            client
                .call(
                    &Request::Process {
                        samples,
                        parameters: BTreeMap::new()
                    },
                    Duration::from_secs(1)
                )
                .is_err()
        );
    }
}
