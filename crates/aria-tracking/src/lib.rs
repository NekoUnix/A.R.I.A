//! One socket worker, a bounded latest-frame mailbox, and explicit connection ownership.
pub mod ifacial;
pub mod protocol;
pub use protocol::Protocol;

use anyhow::{Context, Result, ensure};
use aria_core::TrackingFrame;
use serde::{Deserialize, Serialize};
use std::{
    io::ErrorKind,
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const STALE_AFTER: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReceiverConfig {
    pub protocol: Protocol,
    /// Accept packets only from this IPv4 address; source ports may differ from the request port.
    pub sender_ip: Ipv4Addr,
    pub request_port: u16,
    pub listen_port: u16,
}

impl Default for ReceiverConfig {
    fn default() -> Self {
        Self {
            protocol: Protocol::VTubeStudio,
            sender_ip: Ipv4Addr::LOCALHOST,
            request_port: 21412,
            listen_port: 11125,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub frame: Option<TrackingFrame>,
    pub received_at: Option<Instant>,
    pub packets: u64,
    pub rejected: u64,
    pub ignored: u64,
    pub requests: u64,
    pub packets_per_second: f32,
    pub last_error: Option<String>,
}

impl Snapshot {
    pub fn fresh_frame(&self) -> Option<&TrackingFrame> {
        self.received_at
            .filter(|t| t.elapsed() < STALE_AFTER)
            .and(self.frame.as_ref())
    }

    pub fn status(&self) -> &'static str {
        match (self.received_at, self.fresh_frame()) {
            (None, _) => "Waiting for data",
            (_, None) => "Signal lost",
            (_, Some(f)) if !f.face_found => "Face not found",
            _ => "Tracking live",
        }
    }
}

pub struct Receiver {
    state: Arc<Mutex<Snapshot>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    pub local_port: u16,
}

impl Receiver {
    pub fn start(config: ReceiverConfig) -> Result<Self> {
        ensure!(
            !config.sender_ip.is_unspecified()
                && !config.sender_ip.is_multicast()
                && !config.sender_ip.is_broadcast(),
            "Enter the sender's unicast IPv4 address"
        );
        ensure!(
            config.protocol == Protocol::AriaJson || config.request_port > 0,
            "Request port must be 1-65535"
        );
        // Loopback tests/tools never open a LAN listener. An explicit LAN peer enables LAN binding.
        let bind_ip = if config.sender_ip.is_loopback() {
            Ipv4Addr::LOCALHOST
        } else {
            Ipv4Addr::UNSPECIFIED
        };
        let socket = UdpSocket::bind(SocketAddrV4::new(bind_ip, config.listen_port))
            .with_context(|| format!("Cannot bind UDP port {}. Another app may be using it; choose a different receive port.", config.listen_port))?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let local_port = socket.local_addr()?.port();
        let state = Arc::new(Mutex::new(Snapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_state = Arc::clone(&state);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("aria-tracking".into())
            .spawn(move || {
                run(socket, config, local_port, worker_state, worker_stop);
            })?;
        Ok(Self {
            state,
            stop,
            worker: Some(worker),
            local_port,
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(
    socket: UdpSocket,
    config: ReceiverConfig,
    local_port: u16,
    state: Arc<Mutex<Snapshot>>,
    stop: Arc<AtomicBool>,
) {
    let request = match config.protocol {
        Protocol::VTubeStudio => protocol::subscription(local_port),
        Protocol::IFacialMocap => ifacial::START.to_vec(),
        Protocol::AriaJson => Vec::new(),
    };
    let peer = SocketAddrV4::new(config.sender_ip, config.request_port);
    // Full UDP maximum buffer avoids treating a truncated datagram as a valid packet.
    let mut buffer = vec![0_u8; 65535];
    let mut next_request = Instant::now();
    let mut rate_start = Instant::now();
    let mut rate_count = 0;
    while !stop.load(Ordering::Acquire) {
        let needs_request = match config.protocol {
            Protocol::VTubeStudio => true,
            Protocol::IFacialMocap => state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .fresh_frame()
                .is_none(),
            Protocol::AriaJson => false,
        };
        if needs_request && Instant::now() >= next_request {
            let result = socket.send_to(&request, peer);
            let mut s = state.lock().unwrap_or_else(|p| p.into_inner());
            match result {
                Ok(_) => s.requests += 1,
                Err(e) => s.last_error = Some(format!("Subscription send failed: {e}")),
            }
            // VTS has a renewable lease. iFacialMocap needs one start command;
            // retry it only while disconnected/stale, without resetting a live stream.
            next_request = Instant::now()
                + Duration::from_secs(if config.protocol == Protocol::IFacialMocap {
                    3
                } else {
                    1
                });
        }
        match socket.recv_from(&mut buffer) {
            Ok((len, from)) => {
                if from.ip() != config.sender_ip {
                    state.lock().unwrap_or_else(|p| p.into_inner()).ignored += 1;
                    continue;
                }
                let decoded = protocol::decode(&buffer[..len], config.protocol);
                let mut s = state.lock().unwrap_or_else(|p| p.into_inner());
                match decoded {
                    Ok(frame) => {
                        // Ignore old UDP frames while live; allow a restarted sender after a gap.
                        if s.fresh_frame().is_some_and(|last| {
                            frame.timestamp > 0 && frame.timestamp < last.timestamp
                        }) {
                            s.ignored += 1;
                            continue;
                        }
                        s.frame = Some(frame);
                        s.received_at = Some(Instant::now());
                        s.packets += 1;
                        rate_count += 1;
                        s.last_error = None;
                    }
                    Err(e) => {
                        s.rejected += 1;
                        s.last_error = Some(format!("Rejected packet: {e}"));
                    }
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) => {}
            Err(e) => {
                state.lock().unwrap_or_else(|p| p.into_inner()).last_error =
                    Some(format!("UDP receive failed: {e}"));
                // Avoid spinning on persistent errors; retry so a transient network loss recovers.
                thread::sleep(Duration::from_millis(100));
            }
        }
        let seconds = rate_start.elapsed().as_secs_f32();
        if seconds >= 1.0 {
            state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .packets_per_second = rate_count as f32 / seconds;
            rate_count = 0;
            rate_start = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_until(mut f: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !f() {
            assert!(Instant::now() < deadline, "timed out");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn ifacial_start_live_stream_loss_retry_and_shutdown() {
        let phone = UdpSocket::bind("127.0.0.1:0").unwrap();
        phone
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let receiver = Receiver::start(ReceiverConfig {
            protocol: Protocol::IFacialMocap,
            request_port: phone.local_addr().unwrap().port(),
            listen_port: 0,
            ..Default::default()
        })
        .unwrap();
        let mut buf = [0; 1024];
        let (len, from) = phone.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..len], ifacial::START);
        assert_eq!(from.port(), receiver.local_port);
        phone.set_nonblocking(true).unwrap();
        let packet = b"jawOpen-80|eyeBlink_L-20|=head#-12,23,4,0,0,0|";
        // A healthy stream must not repeatedly receive initialization commands.
        for _ in 0..85 {
            phone.send_to(packet, from).unwrap();
            thread::sleep(Duration::from_millis(40));
            assert!(phone.recv_from(&mut buf).is_err());
        }
        wait_until(|| receiver.snapshot().packets == 85);
        assert_eq!(receiver.snapshot().requests, 1);
        assert_eq!(receiver.snapshot().frame.unwrap().blend("jawopen"), 0.8);
        phone.send_to(b"jawOpen-30|", from).unwrap();
        wait_until(|| receiver.snapshot().rejected == 1);
        wait_until(|| receiver.snapshot().fresh_frame().is_none());
        phone.set_nonblocking(false).unwrap();
        let (len, _) = phone.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..len], ifacial::START);
        phone.send_to(packet, from).unwrap();
        wait_until(|| receiver.snapshot().status() == "Tracking live");
        let port = receiver.local_port;
        let start = Instant::now();
        drop(receiver);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(UdpSocket::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
    }

    #[test]
    fn vts_subscription_receive_renewal_and_shutdown() {
        let phone = UdpSocket::bind("127.0.0.1:0").unwrap();
        phone
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let receiver = Receiver::start(ReceiverConfig {
            request_port: phone.local_addr().unwrap().port(),
            listen_port: 0,
            ..Default::default()
        })
        .unwrap();
        let mut buf = [0; 1024];
        let (n, from) = phone.recv_from(&mut buf).unwrap();
        let request: serde_json::Value = serde_json::from_slice(&buf[..n]).unwrap();
        assert_eq!(request["messageType"], "iOSTrackingDataRequest");
        assert_eq!(request["time"], 5.0);
        assert_eq!(request["ports"][0], receiver.local_port);
        phone
            .send_to(include_bytes!("../tests/fixtures/vts-frame.json"), from)
            .unwrap();
        wait_until(|| receiver.snapshot().packets == 1);
        assert_eq!(receiver.snapshot().status(), "Tracking live");
        phone.send_to(b"garbage", from).unwrap();
        wait_until(|| receiver.snapshot().rejected == 1);
        assert_eq!(receiver.snapshot().packets, 1);
        // A new request must arrive after the initial request, even without incoming frames.
        phone.recv_from(&mut buf).unwrap();
        wait_until(|| receiver.snapshot().status() == "Signal lost");
        phone
            .send_to(include_bytes!("../tests/fixtures/vts-frame.json"), from)
            .unwrap();
        wait_until(|| receiver.snapshot().packets == 2);
        assert_eq!(receiver.snapshot().status(), "Tracking live");
        let started = Instant::now();
        let port = receiver.local_port;
        drop(receiver);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(UdpSocket::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
    }

    #[test]
    fn rejects_packets_from_unconfigured_sender() {
        let receiver = Receiver::start(ReceiverConfig {
            protocol: Protocol::AriaJson,
            sender_ip: Ipv4Addr::new(127, 0, 0, 2),
            listen_port: 0,
            ..Default::default()
        })
        .unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let bytes = protocol::encode(&aria_core::demo_frame(1.0), Protocol::AriaJson).unwrap();
        sender
            .send_to(&bytes, (Ipv4Addr::LOCALHOST, receiver.local_port))
            .unwrap();
        wait_until(|| receiver.snapshot().ignored == 1);
        assert_eq!(receiver.snapshot().packets, 0);
        assert!(receiver.snapshot().fresh_frame().is_none());
    }

    #[test]
    fn json_receiver_rejects_old_frames_and_reports_bind_conflict() {
        let receiver = Receiver::start(ReceiverConfig {
            protocol: Protocol::AriaJson,
            listen_port: 0,
            ..Default::default()
        })
        .unwrap();
        assert!(
            Receiver::start(ReceiverConfig {
                listen_port: receiver.local_port,
                ..Default::default()
            })
            .is_err()
        );
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut f = aria_core::demo_frame(1.0);
        f.timestamp = 20;
        sender
            .send_to(
                &protocol::encode(&f, Protocol::AriaJson).unwrap(),
                (Ipv4Addr::LOCALHOST, receiver.local_port),
            )
            .unwrap();
        wait_until(|| receiver.snapshot().packets == 1);
        f.timestamp = 10;
        sender
            .send_to(
                &protocol::encode(&f, Protocol::AriaJson).unwrap(),
                (Ipv4Addr::LOCALHOST, receiver.local_port),
            )
            .unwrap();
        wait_until(|| receiver.snapshot().ignored == 1);
        assert_eq!(receiver.snapshot().frame.unwrap().timestamp, 20);
    }
}
