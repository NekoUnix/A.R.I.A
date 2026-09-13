use anyhow::{Context, Result, ensure};
use aria_core::{MappingSettings, ParameterPipeline, demo_frame};
use aria_tracking::{Protocol, Receiver, ReceiverConfig, protocol};
use clap::{Parser, Subcommand};
use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    path::PathBuf,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
#[command(version, about = "A.R.I.A. tracking diagnostics and phone simulator")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Emulate the VTube Studio iPhone subscription protocol on loopback.
    SimulateVts {
        #[arg(long, default_value_t = 21412)]
        port: u16,
        #[arg(long, default_value_t = 60)]
        fps: u32,
        #[arg(long, default_value_t = 60)]
        seconds: u32,
    },
    /// Emulate live iFacialMocap UDP on loopback (diagnostics, not a real camera).
    SimulateIfacial {
        #[arg(long, default_value_t = 49984)]
        port: u16,
        #[arg(long, default_value_t = 60)]
        fps: u32,
        #[arg(long, default_value_t = 60)]
        seconds: u32,
    },
    /// Send versioned ARIA JSON tracking to an explicit destination.
    SendJson {
        #[arg(long, default_value = "127.0.0.1:11125")]
        target: SocketAddr,
        #[arg(long, default_value_t = 60)]
        seconds: u32,
    },
    /// Receive tracking without a GPU; emit diagnostics and mapped parameters as JSON lines.
    Listen {
        #[arg(long, default_value = "127.0.0.1")]
        sender: Ipv4Addr,
        #[arg(long)]
        request_port: Option<u16>,
        #[arg(long)]
        listen_port: Option<u16>,
        #[arg(long, conflicts_with = "ifacialmocap")]
        json: bool,
        /// Receive iFacialMocap UDP; both ports default to 49983.
        #[arg(long)]
        ifacialmocap: bool,
        #[arg(long, default_value_t = 30)]
        seconds: u32,
    },
    /// Check a .model3.json file and its local references; does not render .moc3.
    Inspect { path: PathBuf },
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn simulate_ifacial(port: u16, fps: u32, seconds: u32) -> Result<()> {
    ensure!(
        port > 0 && (1..=120).contains(&fps) && seconds > 0,
        "Use a nonzero port/duration and 1–120 FPS"
    );
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, port))?;
    socket.set_nonblocking(true)?;
    eprintln!(
        "Synthetic iFacialMocap phone on 127.0.0.1:{port}. This is a diagnostic simulation, not camera tracking."
    );
    let start = Instant::now();
    let mut client = None;
    let mut buffer = [0; 1024];
    while start.elapsed() < Duration::from_secs(seconds.into()) {
        // Bound request processing so a noisy local peer cannot starve frame sending.
        for _ in 0..32 {
            match socket.recv_from(&mut buffer) {
                Ok((len, from)) if &buffer[..len] == aria_tracking::ifacial::START => {
                    client = Some(from)
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }
        if let Some(client) = client {
            socket.send_to(
                &protocol::encode(
                    &demo_frame(start.elapsed().as_secs_f32()),
                    Protocol::IFacialMocap,
                )?,
                client,
            )?;
        }
        thread::sleep(Duration::from_secs_f64(1.0 / f64::from(fps)));
    }
    Ok(())
}

fn main() -> Result<()> {
    match Args::parse().command {
        Command::SimulateVts { port, fps, seconds } => simulate_vts(port, fps, seconds),
        Command::SimulateIfacial { port, fps, seconds } => simulate_ifacial(port, fps, seconds),
        Command::SendJson { target, seconds } => {
            ensure!(seconds > 0, "seconds must be positive");
            let socket = UdpSocket::bind("0.0.0.0:0")?;
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(seconds.into()) {
                let mut f = demo_frame(start.elapsed().as_secs_f32());
                f.timestamp = now_ms();
                socket.send_to(&protocol::encode(&f, Protocol::AriaJson)?, target)?;
                thread::sleep(Duration::from_secs_f64(1.0 / 60.0));
            }
            Ok(())
        }
        Command::Listen {
            sender,
            request_port,
            listen_port,
            json,
            ifacialmocap,
            seconds,
        } => {
            ensure!(seconds > 0, "seconds must be positive");
            let receiver = Receiver::start(ReceiverConfig {
                sender_ip: sender,
                request_port: request_port.unwrap_or(if ifacialmocap {
                    aria_tracking::ifacial::PORT
                } else {
                    21412
                }),
                listen_port: listen_port.unwrap_or(if ifacialmocap {
                    aria_tracking::ifacial::PORT
                } else {
                    11125
                }),
                protocol: if ifacialmocap {
                    Protocol::IFacialMocap
                } else if json {
                    Protocol::AriaJson
                } else {
                    Protocol::VTubeStudio
                },
            })?;
            eprintln!(
                "Listening on UDP {} for {sender}; Ctrl+C to stop.",
                receiver.local_port
            );
            let start = Instant::now();
            let mut pipeline = ParameterPipeline::default();
            while start.elapsed() < Duration::from_secs(seconds.into()) {
                let s = receiver.snapshot();
                let p = pipeline.update(s.fresh_frame(), &MappingSettings::default(), 0.1);
                println!(
                    "{}",
                    serde_json::json!({ "status": s.status(), "packets": s.packets,
                    "rejected": s.rejected, "requests": s.requests, "parameters": p.named(), "error": s.last_error })
                );
                thread::sleep(Duration::from_millis(100));
            }
            ensure!(
                receiver.snapshot().packets > 0,
                "No valid tracking packets received"
            );
            Ok(())
        }
        Command::Inspect { path } => {
            let r = aria_model::inspect(&path)?;
            println!(
                "{}\n{} textures, {} expressions, {} motions; {} bytes on disk",
                r.manifest.display(),
                r.texture_count,
                r.expression_count,
                r.motion_count,
                r.total_bytes()
            );
            for a in &r.assets {
                println!(
                    "{}: {} — {}",
                    a.kind,
                    a.relative_path,
                    a.problem.as_deref().unwrap_or("present")
                );
            }
            println!("Inspection only. Cubism rendering is not implemented in v0.1.");
            ensure!(
                r.problem_count() == 0,
                "{} asset problems",
                r.problem_count()
            );
            Ok(())
        }
    }
}

fn simulate_vts(port: u16, fps: u32, seconds: u32) -> Result<()> {
    ensure!(
        (1..=120).contains(&fps) && seconds > 0,
        "FPS must be 1-120 and seconds must be positive"
    );
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, port))
        .context("Cannot bind simulator request port")?;
    socket.set_nonblocking(true)?;
    eprintln!(
        "VTS phone simulator at 127.0.0.1:{} for {seconds}s. Waiting for subscriptions…",
        socket.local_addr()?.port()
    );
    let start = Instant::now();
    let mut clients: Vec<(SocketAddr, Instant)> = Vec::new();
    let mut buffer = [0_u8; 2048];
    while start.elapsed() < Duration::from_secs(seconds.into()) {
        // Bound work per frame even if a local client floods requests.
        for _ in 0..32 {
            match socket.recv_from(&mut buffer) {
                Ok((n, sender)) => {
                    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buffer[..n]) else {
                        continue;
                    };
                    if v["messageType"] != "iOSTrackingDataRequest" {
                        continue;
                    }
                    let Some(time) = v["time"].as_f64().filter(|t| (0.5..=10.0).contains(t)) else {
                        continue;
                    };
                    let Some(ports) = v["ports"]
                        .as_array()
                        .filter(|p| !p.is_empty() && p.len() <= 32)
                    else {
                        continue;
                    };
                    for port in ports
                        .iter()
                        .filter_map(|p| p.as_u64())
                        .filter(|p| (1..=65535).contains(p))
                    {
                        let address = SocketAddr::new(sender.ip(), port as u16);
                        let until = Instant::now() + Duration::from_secs_f64(time);
                        if let Some(client) = clients.iter_mut().find(|(a, _)| *a == address) {
                            client.1 = until;
                        } else if clients.len() < 32 {
                            clients.push((address, until));
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }
        let mut f = demo_frame(start.elapsed().as_secs_f32());
        f.timestamp = now_ms();
        let packet = protocol::encode(&f, Protocol::VTubeStudio)?;
        clients.retain(|(_, until)| Instant::now() < *until);
        for (address, _) in &clients {
            socket.send_to(&packet, address)?;
        }
        thread::sleep(Duration::from_secs_f64(1.0 / fps as f64));
    }
    Ok(())
}
