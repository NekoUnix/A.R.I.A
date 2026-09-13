//! A private, versioned stdio connection to a separate Cubism process.
//! This isolates Core crashes and owns its lifetime; it is not an OS security sandbox.
use crate::{Canvas, CubismModel, Drawable, Parameter};
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
const VERSION: u16 = 1;
const MAX_PACKET: usize = 128 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
enum Request {
    Load {
        core: PathBuf,
        moc: PathBuf,
        textures: usize,
    },
    Update(Vec<f32>),
}
#[derive(Serialize, Deserialize)]
enum Response {
    Loaded {
        canvas: Canvas,
        version: String,
        parameters: Vec<Parameter>,
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

/// Worker entry point. Only the worker loads executable Core code; meshes are Rust data.
pub fn serve(reader: impl Read, writer: impl Write) -> Result<()> {
    let (mut reader, mut writer) = (BufReader::new(reader), BufWriter::new(writer));
    let mut model: Option<CubismModel> = None;
    while let Some(request) = read_packet(&mut reader)? {
        let response = (|| -> Result<Response> {
            match request {
                Request::Load {
                    core,
                    moc,
                    textures,
                } => {
                    ensure!(model.is_none(), "A Cubism host owns exactly one model");
                    let mut bytes = Vec::new();
                    std::fs::File::open(&moc)
                        .with_context(|| format!("Cannot open {}", moc.display()))?
                        .take(aria_core::asset_limits::MOC_FILE as u64 + 1)
                        .read_to_end(&mut bytes)?;
                    let loaded = CubismModel::load(&core, &bytes, textures)?;
                    let response = Response::Loaded {
                        canvas: loaded.canvas,
                        version: loaded.version.clone(),
                        parameters: loaded.parameters().to_vec(),
                        draws: loaded.drawables.clone(),
                    };
                    model = Some(loaded);
                    Ok(response)
                }
                Request::Update(values) => {
                    let model = model.as_mut().context("Load a model before updating it")?;
                    ensure!(
                        values.len() == model.parameters.len()
                            && values.iter().all(|v| v.is_finite()),
                        "Invalid Cubism parameter frame"
                    );
                    for (parameter, value) in model.parameters.iter_mut().zip(values) {
                        parameter.value = value.clamp(parameter.min, parameter.max);
                    }
                    model.update()?;
                    Ok(Response::Frame(
                        model
                            .drawables
                            .iter()
                            .map(|d| MovingDrawable {
                                positions: d.positions.clone(),
                                visible: d.visible,
                                order: d.order,
                                opacity: d.opacity,
                                multiply: d.multiply,
                                screen: d.screen,
                            })
                            .collect(),
                    ))
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

/// Main-process model data. No native pointers or libraries cross the process boundary.
pub struct HostedModel {
    connection: Connection,
    parameters: Vec<Parameter>,
    lookup: BTreeMap<String, usize>,
    pub canvas: Canvas,
    pub version: String,
    pub drawables: Vec<Drawable>,
}
impl HostedModel {
    pub fn load(core: &Path, moc: &Path, textures: usize) -> Result<Self> {
        let executable = std::env::var_os("ARIA_CUBISM_HOST")
            .map(PathBuf::from)
            .map_or_else(std::env::current_exe, Ok)?;
        Self::load_with_host(&executable, core, moc, textures)
    }
    pub fn load_with_host(
        executable: &Path,
        core: &Path,
        moc: &Path,
        textures: usize,
    ) -> Result<Self> {
        let core = crate::platform::resolve(core)?;
        let moc = moc.canonicalize().context("Cannot find the moc3 file")?;
        ensure!((1..=32).contains(&textures), "Expected 1–32 textures");
        let mut connection = Connection::start(executable)?;
        let Response::Loaded {
            canvas,
            version,
            parameters,
            draws,
        } = connection.request(
            Request::Load {
                core,
                moc,
                textures,
            },
            Duration::from_secs(30),
        )?
        else {
            bail!("Cubism host returned an unexpected load response");
        };
        ensure!(
            parameters.len() <= 8192 && draws.len() <= 8192,
            "Invalid host model size"
        );
        let lookup = parameters
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();
        Ok(Self {
            connection,
            canvas,
            version,
            parameters,
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
    pub fn update(&mut self) -> Result<()> {
        let Response::Frame(frame) = self.connection.request(
            Request::Update(self.parameters.iter().map(|p| p.value).collect()),
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
            d.positions.copy_from_slice(&next.positions);
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
        write_packet(&mut packet, &Request::Update(vec![0.5, -1.0])).unwrap();
        assert!(matches!(
            read_packet::<Request>(&mut packet.as_slice()).unwrap(),
            Some(Request::Update(_))
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
    #[ignore = "requires ARIA_TEST_HOST, ARIA_CUBISM_CORE and ARIA_TEST_MOC; no artwork is redistributed"]
    fn isolated_core_matches_native_frames_and_handles_worker_exit() {
        let host = PathBuf::from(std::env::var_os("ARIA_TEST_HOST").unwrap());
        let core = PathBuf::from(std::env::var_os("ARIA_CUBISM_CORE").unwrap());
        let moc = PathBuf::from(std::env::var_os("ARIA_TEST_MOC").unwrap());
        let bytes = std::fs::read(&moc).unwrap();
        let mut native = CubismModel::load(&core, &bytes, 32).unwrap();
        let mut hosted = HostedModel::load_with_host(&host, &core, &moc, 32).unwrap();
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
        hosted.connection.child.kill().unwrap();
        let start = std::time::Instant::now();
        assert!(hosted.update().is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(hosted.update().is_err());
    }
}
