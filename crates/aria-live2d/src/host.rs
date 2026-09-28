//! A private, versioned stdio connection to a separate Cubism process, plus an
//! opt-in in-process adapter for profiling ARIA's Rust model core. The default
//! worker isolates Core crashes; it is not an OS security sandbox.
use crate::{Canvas, CubismModel, Drawable, Parameter, rust_model::RustModel};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::BTreeMap,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread::JoinHandle,
    time::Duration,
};

const MAGIC: &[u8; 8] = b"ARIACORE";
const VERSION: u16 = 4;
const MAX_PACKET: usize = 128 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
enum Request {
    Load { moc: PathBuf, textures: usize },
    Update(Vec<f32>, Vec<f32>),
}
#[derive(Serialize, Deserialize)]
enum Response {
    Loaded {
        canvas: Canvas,
        version: String,
        parameters: Vec<Parameter>,
        parts: Vec<Parameter>,
        draws: Vec<Drawable>,
    },
    Frame(Vec<MovingDrawable>),
    Error(String),
}
#[derive(Serialize, Deserialize)]
struct MovingDrawable {
    positions: Vec<[f32; 2]>,
    visible: bool,
    order: i32,
    opacity: f32,
    multiply: [f32; 4],
    screen: [f32; 4],
}

fn write_packet(writer: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let bytes = postcard::to_allocvec(value)?;
    ensure!(
        bytes.len() <= MAX_PACKET,
        "Cubism host message exceeds 128 MiB"
    );
    writer.write_all(MAGIC)?;
    writer.write_all(&VERSION.to_le_bytes())?;
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}
fn read_packet<T: DeserializeOwned>(reader: &mut impl Read) -> Result<Option<T>> {
    let mut header = [0_u8; 14];
    if reader.read(&mut header[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..])?;
    ensure!(
        &header[..8] == MAGIC && u16::from_le_bytes(header[8..10].try_into()?) == VERSION,
        "Cubism host protocol mismatch; keep ARIA and its runtime host from the same build"
    );
    let length = u32::from_le_bytes(header[10..14].try_into()?) as usize;
    ensure!(
        length > 0 && length <= MAX_PACKET,
        "Invalid Cubism host packet size"
    );
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    let (value, tail) = postcard::take_from_bytes(&bytes)?;
    ensure!(tail.is_empty(), "Unexpected data in Cubism host packet");
    Ok(Some(value))
}

enum HostEngine {
    Current(Box<CubismModel>),
    Rust(Box<RustModel>),
}

fn apply_inputs(
    parameters: &mut [Parameter],
    model_parts: &mut [Parameter],
    values: Vec<f32>,
    parts: Vec<f32>,
) -> Result<()> {
    ensure!(
        values.len() == parameters.len() && values.iter().all(|value| value.is_finite()),
        "Invalid Cubism parameter frame"
    );
    for (parameter, value) in parameters.iter_mut().zip(values) {
        parameter.value = value.clamp(parameter.min, parameter.max);
    }
    ensure!(
        parts.len() == model_parts.len()
            && parts
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value)),
        "Invalid part opacity frame"
    );
    for (part, value) in model_parts.iter_mut().zip(parts) {
        part.value = value;
    }
    Ok(())
}

fn moving(drawables: &[Drawable]) -> Vec<MovingDrawable> {
    drawables
        .iter()
        .map(|drawable| MovingDrawable {
            positions: drawable.positions.clone(),
            visible: drawable.visible,
            order: drawable.order,
            opacity: drawable.opacity,
            multiply: drawable.multiply,
            screen: drawable.screen,
        })
        .collect()
}

/// Worker entry point. Rust evaluation can be enabled for local parity testing.
pub fn serve(reader: impl Read, writer: impl Write) -> Result<()> {
    let (mut reader, mut writer) = (BufReader::new(reader), BufWriter::new(writer));
    let mut model: Option<HostEngine> = None;
    while let Some(request) = read_packet(&mut reader)? {
        let response = (|| -> Result<Response> {
            match request {
                Request::Load { moc, textures } => {
                    ensure!(model.is_none(), "A Cubism host owns exactly one model");
                    let mut bytes = Vec::new();
                    std::fs::File::open(&moc)
                        .with_context(|| format!("Cannot open {}", moc.display()))?
                        .take(aria_core::asset_limits::MOC_FILE as u64 + 1)
                        .read_to_end(&mut bytes)?;
                    let use_rust =
                        std::env::var("ARIA_EXPERIMENTAL_RUST_CORE").as_deref() == Ok("1");
                    let loaded = if use_rust {
                        HostEngine::Rust(Box::new(RustModel::load(&bytes, textures)?))
                    } else {
                        HostEngine::Current(Box::new(CubismModel::load(
                            Path::new(""),
                            &bytes,
                            textures,
                        )?))
                    };
                    let response = match &loaded {
                        HostEngine::Current(inner) => Response::Loaded {
                            canvas: inner.canvas,
                            version: inner.version.clone(),
                            parameters: inner.parameters().to_vec(),
                            parts: inner.parts.clone(),
                            draws: inner.drawables.clone(),
                        },
                        HostEngine::Rust(inner) => Response::Loaded {
                            canvas: inner.canvas,
                            version: inner.version.clone(),
                            parameters: inner.parameters().to_vec(),
                            parts: inner.parts.clone(),
                            draws: inner.drawables.clone(),
                        },
                    };
                    model = Some(loaded);
                    Ok(response)
                }
                Request::Update(values, parts) => {
                    let model = model.as_mut().context("Load a model before updating it")?;
                    match model {
                        HostEngine::Current(inner) => {
                            apply_inputs(&mut inner.parameters, &mut inner.parts, values, parts)?;
                            inner.update()?;
                            Ok(Response::Frame(moving(&inner.drawables)))
                        }
                        HostEngine::Rust(inner) => {
                            apply_inputs(&mut inner.parameters, &mut inner.parts, values, parts)?;
                            inner.update()?;
                            Ok(Response::Frame(moving(&inner.drawables)))
                        }
                    }
                }
            }
        })()
        .unwrap_or_else(|error| Response::Error(format!("{error:#}")));
        write_packet(&mut writer, &response)?;
    }
    Ok(())
}

struct Connection {
    child: Child,
    send: Option<mpsc::SyncSender<Request>>,
    replies: mpsc::Receiver<Result<Response, String>>,
    io: Option<JoinHandle<()>>,
    stopped: bool,
}
impl Connection {
    fn start(executable: &Path) -> Result<Self> {
        let mut command = Command::new(executable);
        command
            .arg("--cubism-host")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW: no extra console per avatar.
        }
        let child = command
            .spawn()
            .context("Cannot start the Cubism runtime host")?;
        let (send, commands) = mpsc::sync_channel(1);
        let (reply, replies) = mpsc::channel();
        let mut connection = Self {
            child,
            send: Some(send),
            replies,
            io: None,
            stopped: false,
        };
        let input = connection
            .child
            .stdin
            .take()
            .context("Missing host input pipe")?;
        let output = connection
            .child
            .stdout
            .take()
            .context("Missing host output pipe")?;
        connection.io = Some(
            std::thread::Builder::new()
                .name("aria-cubism-transport".into())
                .spawn(move || {
                    let (mut input, mut output) = (BufWriter::new(input), BufReader::new(output));
                    while let Ok(command) = commands.recv() {
                        let result = write_packet(&mut input, &command)
                            .and_then(|_| {
                                read_packet(&mut output)?
                                    .context("Cubism runtime host exited unexpectedly")
                            })
                            .map_err(|e| format!("{e:#}"));
                        let failed = result.is_err();
                        if reply.send(result).is_err() || failed {
                            break;
                        }
                    }
                })?,
        );
        Ok(connection)
    }
    fn stop(&mut self) {
        self.stopped = true;
        self.send.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(io) = self.io.take() {
            let _ = io.join();
        }
    }
    fn request(&mut self, request: Request, timeout: Duration) -> Result<Response> {
        ensure!(
            !self.stopped,
            "Cubism runtime stopped; reload this model to retry"
        );
        let result = (|| -> Result<Response> {
            self.send
                .as_ref()
                .context("Cubism host closed")?
                .try_send(request)
                .context("Cubism host is unavailable")?;
            let response = self
                .replies
                .recv_timeout(timeout)
                .context("Cubism host stopped responding")?
                .map_err(anyhow::Error::msg)?;
            if let Response::Error(error) = response {
                bail!("{error}");
            }
            Ok(response)
        })();
        if result.is_err() {
            self.stop();
        }
        result
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Main-process model data. The default worker keeps native pointers isolated;
/// the experimental direct Rust path has no native library boundary.
pub struct HostedModel {
    connection: Option<Connection>,
    direct: Option<Box<RustModel>>,
    parameters: Vec<Parameter>,
    pub parts: Vec<Parameter>,
    lookup: BTreeMap<String, usize>,
    pub canvas: Canvas,
    pub version: String,
    pub drawables: Vec<Drawable>,
}
impl HostedModel {
    pub fn process_id(&self) -> u32 {
        self.connection
            .as_ref()
            .map_or_else(std::process::id, |connection| connection.child.id())
    }
    /// The legacy Core path is ignored. The default worker uses statically linked
    /// Purism Core; the explicit experimental flag runs ARIA's Rust core in process.
    pub fn load(core: &Path, moc: &Path, textures: usize) -> Result<Self> {
        if std::env::var("ARIA_EXPERIMENTAL_DIRECT_RUST_CORE").as_deref() == Ok("1") {
            return Self::load_direct_rust(moc, textures);
        }
        let executable = std::env::var_os("ARIA_CUBISM_HOST")
            .map(PathBuf::from)
            .map_or_else(std::env::current_exe, Ok)?;
        Self::load_with_host(&executable, core, moc, textures)
    }

    fn load_rust(moc: &Path, textures: usize) -> Result<RustModel> {
        let moc = moc.canonicalize().context("Cannot find the moc3 file")?;
        let mut bytes = Vec::new();
        std::fs::File::open(&moc)
            .with_context(|| format!("Cannot open {}", moc.display()))?
            .take(aria_core::asset_limits::MOC_FILE as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= aria_core::asset_limits::MOC_FILE,
            "MOC3 file exceeds the model-size limit"
        );
        RustModel::load(&bytes, textures)
    }

    fn load_direct_rust(moc: &Path, textures: usize) -> Result<Self> {
        let direct = Self::load_rust(moc, textures)?;
        let parameters = direct.parameters().to_vec();
        let lookup = parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.id.clone(), index))
            .collect();
        let hosted = Self {
            connection: None,
            canvas: direct.canvas,
            version: direct.version.clone(),
            parameters,
            parts: direct.parts.clone(),
            lookup,
            drawables: direct.drawables.clone(),
            direct: Some(Box::new(direct)),
        };
        Ok(hosted)
    }

    /// Use an explicit ARIA worker executable; the legacy Core path is ignored.
    pub fn load_with_host(
        executable: &Path,
        _legacy_core_path: &Path,
        moc: &Path,
        textures: usize,
    ) -> Result<Self> {
        let moc = moc.canonicalize().context("Cannot find the moc3 file")?;
        ensure!((1..=32).contains(&textures), "Expected 1–32 textures");
        let mut connection = Connection::start(executable)?;
        let Response::Loaded {
            canvas,
            version,
            parameters,
            parts,
            draws,
        } = connection.request(Request::Load { moc, textures }, Duration::from_secs(30))?
        else {
            bail!("Cubism host returned an unexpected load response");
        };
        ensure!(
            parameters.len() <= 8192 && parts.len() <= 8192 && draws.len() <= 8192,
            "Invalid host model size"
        );
        let lookup = parameters
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();
        Ok(Self {
            connection: Some(connection),
            direct: None,
            canvas,
            version,
            parameters,
            parts,
            lookup,
            drawables: draws,
        })
    }
    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }
    pub fn set_parameter(&mut self, id: &str, value: f32) {
        if value.is_finite()
            && let Some(&index) = self.lookup.get(id)
        {
            let p = &mut self.parameters[index];
            p.value = value.clamp(p.min, p.max);
        }
    }
    pub fn reset_parameters(&mut self) {
        for p in &mut self.parameters {
            p.value = p.default;
        }
    }
    pub fn uses_direct_rust(&self) -> bool {
        self.direct.is_some()
    }

    /// Evaluate renderer metadata without CPU vertex deformation. Only the
    /// independent in-process Rust model supports this GPU companion path.
    pub fn update_metadata(&mut self) -> Result<()> {
        ensure!(
            self.direct.is_some(),
            "GPU metadata requires the Rust model core"
        );
        self.update_direct(true)
    }

    fn update_direct(&mut self, metadata_only: bool) -> Result<()> {
        let direct = self.direct.as_mut().context("Missing direct Rust model")?;
        ensure!(
            self.parameters.len() == direct.parameters.len()
                && self.parts.len() == direct.parts.len(),
            "Rust core topology changed"
        );
        for (source, target) in self.parameters.iter().zip(&mut direct.parameters) {
            target.value = source.value;
        }
        for (source, target) in self.parts.iter().zip(&mut direct.parts) {
            target.value = source.value;
        }
        std::mem::swap(&mut self.drawables, &mut direct.drawables);
        let result = if metadata_only {
            direct.update_metadata()
        } else {
            direct.update()
        };
        std::mem::swap(&mut self.drawables, &mut direct.drawables);
        result
    }

    pub fn update(&mut self) -> Result<()> {
        if self.direct.is_some() {
            return self.update_direct(false);
        }
        let Response::Frame(frame) = self
            .connection
            .as_mut()
            .context("Missing model worker")?
            .request(
                Request::Update(
                    self.parameters.iter().map(|p| p.value).collect(),
                    self.parts.iter().map(|p| p.value).collect(),
                ),
                Duration::from_secs(2),
            )?
        else {
            bail!("Cubism host returned an unexpected frame");
        };
        ensure!(
            frame.len() == self.drawables.len(),
            "Cubism host changed mesh topology"
        );
        for (d, next) in self.drawables.iter_mut().zip(frame) {
            ensure!(
                next.positions.len() == d.positions.len()
                    && next
                        .positions
                        .iter()
                        .flatten()
                        .chain(&next.multiply)
                        .chain(&next.screen)
                        .chain([&next.opacity])
                        .all(|v| v.is_finite()),
                "Cubism host returned invalid mesh data"
            );
            d.positions = next.positions;
            d.visible = next.visible;
            d.order = next.order;
            d.opacity = next.opacity;
            d.multiply = next.multiply;
            d.screen = next.screen;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_rejects_bad_versions_sizes_truncation_and_trailing_data() {
        let mut packet = Vec::new();
        write_packet(&mut packet, &Request::Update(vec![0.5, -1.0], vec![])).unwrap();
        assert!(matches!(
            read_packet::<Request>(&mut packet.as_slice()).unwrap(),
            Some(Request::Update(_, _))
        ));
        for index in [0, 8] {
            let mut corrupt = packet.clone();
            corrupt[index] ^= 1;
            assert!(read_packet::<Request>(&mut corrupt.as_slice()).is_err());
        }
        let mut oversized = packet.clone();
        oversized[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_packet::<Request>(&mut oversized.as_slice()).is_err());
        assert!(read_packet::<Request>(&mut &packet[..packet.len() - 1]).is_err());
        let mut trailing = packet.clone();
        trailing.push(0);
        let length = (trailing.len() - 14) as u32;
        trailing[10..14].copy_from_slice(&length.to_le_bytes());
        assert!(read_packet::<Request>(&mut trailing.as_slice()).is_err());
        let mut reply = Vec::new();
        serve(packet.as_slice(), &mut reply).unwrap();
        assert!(matches!(
            read_packet::<Response>(&mut reply.as_slice()).unwrap(),
            Some(Response::Error(_))
        ));
    }
    #[test]
    #[ignore = "requires ARIA_TEST_HOST, ARIA_TEST_MOC; no artwork is redistributed"]
    fn isolated_core_matches_native_frames_and_handles_worker_exit() {
        let host = PathBuf::from(std::env::var_os("ARIA_TEST_HOST").unwrap());
        let core = PathBuf::from(std::ffi::OsString::new());
        let moc = PathBuf::from(std::env::var_os("ARIA_TEST_MOC").unwrap());
        let bytes = std::fs::read(&moc).unwrap();
        let mut native = CubismModel::load(&core, &bytes, 32).unwrap();
        let mut hosted = HostedModel::load_with_host(&host, &core, &moc, 32).unwrap();
        assert!(
            native
                .drawables
                .iter()
                .zip(&hosted.drawables)
                .all(|(a, b)| a.id == b.id && a.part == b.part)
        );
        for max in [true, false, true] {
            for p in native.parameters().to_vec() {
                let value = if max { p.max } else { p.min };
                native.set_parameter(&p.id, value);
                hosted.set_parameter(&p.id, value);
            }
            native.update().unwrap();
            hosted.update().unwrap();
            for (a, b) in native.drawables.iter().zip(&hosted.drawables) {
                assert_eq!(a.positions, b.positions);
                assert_eq!(a.uvs, b.uvs);
                assert_eq!(a.indices, b.indices);
                assert_eq!(a.opacity, b.opacity);
            }
        }
        hosted.connection.as_mut().unwrap().child.kill().unwrap();
        let start = std::time::Instant::now();
        assert!(hosted.update().is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(hosted.update().is_err());
    }

    #[test]
    #[ignore = "requires ARIA_TEST_HOST, ARIA_TEST_MOC, ARIA_EXPERIMENTAL_RUST_CORE=1 and local artwork"]
    fn isolated_rust_core_serves_renderer_frames() {
        assert_eq!(
            std::env::var("ARIA_EXPERIMENTAL_RUST_CORE").as_deref(),
            Ok("1")
        );
        let host = PathBuf::from(std::env::var_os("ARIA_TEST_HOST").unwrap());
        let moc = PathBuf::from(std::env::var_os("ARIA_TEST_MOC").unwrap());
        let bytes = std::fs::read(&moc).unwrap();
        let mut native = RustModel::load(&bytes, 32).unwrap();
        let mut hosted = HostedModel::load_with_host(&host, Path::new(""), &moc, 32).unwrap();
        assert!(hosted.version.starts_with("ARIA Rust Model Core"));
        assert_eq!(native.drawables.len(), hosted.drawables.len());
        for (expected, actual) in native.drawables.iter().zip(&hosted.drawables) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.uvs, expected.uvs);
            assert_eq!(actual.indices, expected.indices);
            assert_eq!(actual.masks, expected.masks);
        }
        for maximum in [true, false, true] {
            for parameter in native.parameters().to_vec() {
                let value = if maximum {
                    parameter.max
                } else {
                    parameter.min
                };
                native.set_parameter(&parameter.id, value);
                hosted.set_parameter(&parameter.id, value);
            }
            native.update().unwrap();
            hosted.update().unwrap();
            for (expected, actual) in native.drawables.iter().zip(&hosted.drawables) {
                assert_eq!(actual.visible, expected.visible);
                assert_eq!(actual.order, expected.order);
                if expected.visible {
                    assert!((actual.opacity - expected.opacity).abs() <= 0.001);
                    for (left, right) in actual.positions.iter().zip(&expected.positions) {
                        assert!((left[0] - right[0]).abs() <= 0.001);
                        assert!((left[1] - right[1]).abs() <= 0.001);
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires ARIA_TEST_MOC and ARIA_EXPERIMENTAL_DIRECT_RUST_CORE=1"]
    fn direct_rust_core_keeps_independent_avatar_frames() {
        assert_eq!(
            std::env::var("ARIA_EXPERIMENTAL_DIRECT_RUST_CORE").as_deref(),
            Ok("1")
        );
        let moc = PathBuf::from(std::env::var_os("ARIA_TEST_MOC").unwrap());
        let mut first = HostedModel::load(Path::new(""), &moc, 32).unwrap();
        let mut second = HostedModel::load(Path::new(""), &moc, 32).unwrap();
        assert_eq!(first.process_id(), std::process::id());
        assert_eq!(second.process_id(), std::process::id());
        assert!(first.connection.is_none() && second.connection.is_none());
        let parameter = first
            .parameters()
            .iter()
            .find(|parameter| parameter.max > parameter.min)
            .unwrap()
            .clone();
        first.set_parameter(&parameter.id, parameter.min);
        second.set_parameter(&parameter.id, parameter.max);
        first.update().unwrap();
        second.update().unwrap();
        assert!(
            first
                .drawables
                .iter()
                .zip(&second.drawables)
                .any(|(left, right)| left.positions != right.positions)
        );
        let first_positions = first
            .drawables
            .iter()
            .map(|drawable| drawable.positions.clone())
            .collect::<Vec<_>>();
        second.update().unwrap();
        assert!(
            first
                .drawables
                .iter()
                .zip(first_positions)
                .all(|(drawable, positions)| drawable.positions == positions)
        );
    }

    #[test]
    #[ignore = "requires ARIA_TEST_MOC; compares direct Rust metadata with full frames"]
    fn direct_rust_metadata_keeps_vertices_and_matches_render_state() {
        let moc = PathBuf::from(std::env::var_os("ARIA_TEST_MOC").unwrap());
        let mut full = HostedModel::load_direct_rust(&moc, 32).unwrap();
        let mut metadata = HostedModel::load_direct_rust(&moc, 32).unwrap();
        assert!(metadata.uses_direct_rust());
        let initial_positions = metadata
            .drawables
            .iter()
            .map(|drawable| drawable.positions.clone())
            .collect::<Vec<_>>();
        for pose in 0..3 {
            for (index, parameter) in full.parameters().to_vec().iter().enumerate() {
                let value = match pose {
                    0 => parameter.default,
                    1 => parameter.min + (parameter.max - parameter.min) * 0.37,
                    _ => {
                        let phase = ((index * 17 + 3) % 23) as f32 / 22.0;
                        parameter.min + (parameter.max - parameter.min) * phase
                    }
                };
                full.set_parameter(&parameter.id, value);
                metadata.set_parameter(&parameter.id, value);
            }
            full.update().unwrap();
            metadata.update_metadata().unwrap();
            for ((expected, actual), initial) in full
                .drawables
                .iter()
                .zip(&metadata.drawables)
                .zip(&initial_positions)
            {
                assert_eq!(actual.visible, expected.visible);
                assert_eq!(actual.order, expected.order);
                assert_eq!(actual.opacity, expected.opacity);
                assert_eq!(actual.multiply, expected.multiply);
                assert_eq!(actual.screen, expected.screen);
                assert_eq!(&actual.positions, initial);
            }
        }
    }

    #[test]
    #[ignore = "requires ARIA_TEST_MOC and ARIA_EXPERIMENTAL_DIRECT_RUST_CORE=1; run optimized"]
    fn direct_rust_update_benchmark() {
        assert_eq!(
            std::env::var("ARIA_EXPERIMENTAL_DIRECT_RUST_CORE").as_deref(),
            Ok("1")
        );
        let moc = PathBuf::from(std::env::var_os("ARIA_TEST_MOC").unwrap());
        let mut full = HostedModel::load(Path::new(""), &moc, 32).unwrap();
        let mut metadata = HostedModel::load(Path::new(""), &moc, 32).unwrap();
        let parameter = full
            .parameters()
            .iter()
            .find(|parameter| parameter.max > parameter.min)
            .unwrap()
            .clone();
        for (label, model, metadata_only) in [
            ("full", &mut full, false),
            ("metadata", &mut metadata, true),
        ] {
            let mut measured = std::time::Duration::ZERO;
            for frame in 0..140 {
                model.set_parameter(
                    &parameter.id,
                    if frame % 2 == 0 {
                        parameter.min
                    } else {
                        parameter.max
                    },
                );
                let start = std::time::Instant::now();
                if metadata_only {
                    model.update_metadata().unwrap();
                } else {
                    model.update().unwrap();
                }
                if frame >= 20 {
                    measured += start.elapsed();
                }
            }
            eprintln!(
                "{} {label}: 120 direct hosted frames {:.3} ms/frame after warmup",
                model.version,
                measured.as_secs_f64() * 1000.0 / 120.0
            );
        }
    }

    #[test]
    #[ignore = "requires ARIA_TEST_HOST and ARIA_TEST_MOC; run optimized for meaningful timing"]
    fn isolated_worker_update_benchmark() {
        let host = PathBuf::from(std::env::var_os("ARIA_TEST_HOST").unwrap());
        let moc = PathBuf::from(std::env::var_os("ARIA_TEST_MOC").unwrap());
        let mut hosted = HostedModel::load_with_host(&host, Path::new(""), &moc, 32).unwrap();
        let parameter = hosted
            .parameters()
            .iter()
            .find(|parameter| parameter.max > parameter.min)
            .unwrap()
            .clone();
        let start = std::time::Instant::now();
        let mut measured = std::time::Duration::ZERO;
        for frame in 0..140 {
            hosted.set_parameter(
                &parameter.id,
                if frame % 2 == 0 {
                    parameter.min
                } else {
                    parameter.max
                },
            );
            let before = std::time::Instant::now();
            hosted.update().unwrap();
            if frame >= 20 {
                measured += before.elapsed();
            }
        }
        eprintln!(
            "{}: 120 worker frames {:.3} ms/frame after warmup, {:.2} s total",
            hosted.version,
            measured.as_secs_f64() * 1000.0 / 120.0,
            start.elapsed().as_secs_f64()
        );
    }
}
