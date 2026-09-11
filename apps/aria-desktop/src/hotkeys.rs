//! Owned Windows hotkey registrations. No keyboard hook or keystroke recording.
use eframe::egui;
#[cfg(windows)]
use std::time::Duration;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

pub const TOGGLE_POSE: u8 = 100;
pub enum Event {
    Pressed {
        generation: u64,
        key: u8,
    },
    Registered {
        generation: u64,
        errors: Vec<String>,
        thread_id: u32,
    },
}
enum Command {
    Configure { generation: u64, keys: Vec<u8> },
    Stop,
}
pub struct Hotkeys {
    tx: Sender<Command>,
    rx: Receiver<Event>,
    worker: Option<JoinHandle<()>>,
    keys: Vec<u8>,
    pub generation: u64,
}
impl Hotkeys {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("aria-hotkeys".into())
            .spawn(move || run(commands, events, ctx))
            .ok();
        Self {
            tx,
            rx,
            worker,
            keys: Vec::new(),
            generation: 0,
        }
    }
    pub fn configure(&mut self, mut keys: Vec<u8>) {
        keys.sort_unstable();
        keys.dedup();
        if self.keys == keys {
            return;
        }
        self.keys = keys.clone();
        self.generation = self.generation.wrapping_add(1);
        let _ = self.tx.send(Command::Configure {
            generation: self.generation,
            keys,
        });
    }
    pub fn events(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
    pub fn available(&self) -> bool {
        self.worker.is_some()
    }
}
impl Drop for Hotkeys {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn label(key: u8) -> String {
    if key == TOGGLE_POSE {
        "Ctrl+Alt+P".into()
    } else {
        format!("Ctrl+Alt+F{key}")
    }
}

#[cfg(windows)]
fn run(commands: Receiver<Command>, events: Sender<Event>, ctx: egui::Context) {
    use windows::Win32::{
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{
                MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
            },
            WindowsAndMessaging::{MSG, PM_REMOVE, PeekMessageW, WM_HOTKEY},
        },
    };
    let mut registered = Vec::new();
    let mut generation = 0;
    let unregister = |keys: &mut Vec<u8>| {
        for key in keys.drain(..) {
            unsafe {
                let _ = UnregisterHotKey(None, i32::from(key));
            }
        }
    };
    loop {
        match commands.recv_timeout(Duration::from_millis(15)) {
            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Configure {
                generation: new,
                keys,
            }) => {
                unregister(&mut registered);
                // Discard key presses queued for the previous model/configuration.
                let mut stale = MSG::default();
                while unsafe { PeekMessageW(&mut stale, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) }
                    .as_bool()
                {}
                generation = new;
                let mut errors = Vec::new();
                for key in keys {
                    let vk = match key {
                        1..=11 => 0x70 + u32::from(key) - 1,
                        TOGGLE_POSE => 0x50,
                        _ => continue,
                    };
                    // A null HWND associates the registration with THIS worker's
                    // queue. Only owned registrations are removed on rebind/drop.
                    match unsafe {
                        RegisterHotKey(
                            None,
                            i32::from(key),
                            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                            vk,
                        )
                    } {
                        Ok(()) => registered.push(key),
                        Err(error) => errors.push(format!(
                            "{} unavailable (possibly used by another app): {error}",
                            label(key)
                        )),
                    }
                }
                let _ = events.send(Event::Registered {
                    generation,
                    errors,
                    thread_id: unsafe { GetCurrentThreadId() },
                });
                ctx.request_repaint();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) }.as_bool()
        {
            if let Ok(key) = u8::try_from(message.wParam.0)
                && registered.contains(&key)
            {
                let _ = events.send(Event::Pressed { generation, key });
                ctx.request_repaint();
            }
        }
    }
    unregister(&mut registered);
}
#[cfg(not(windows))]
fn run(commands: Receiver<Command>, events: Sender<Event>, ctx: egui::Context) {
    while let Ok(command) = commands.recv() {
        match command {
            Command::Stop => break,
            Command::Configure { generation, keys } => {
                let _ = events.send(Event::Registered {
                    generation,
                    errors: if keys.is_empty() {
                        Vec::new()
                    } else {
                        vec!["Global hotkeys currently require Windows. Use preset buttons.".into()]
                    },
                    thread_id: 0,
                });
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "registers temporary desktop hotkeys on an interactive Windows session"]
    fn native_registration_dispatch_conflict_and_cleanup() {
        use windows::Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::WindowsAndMessaging::{PostThreadMessageW, WM_HOTKEY},
        };
        let mut first = Hotkeys::new(egui::Context::default());
        first.configure(vec![11]);
        let event = first.rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let thread_id = match event {
            Event::Registered {
                errors, thread_id, ..
            } => {
                assert!(errors.is_empty(), "{errors:?}");
                thread_id
            }
            _ => panic!("Registration expected"),
        };
        // Inject only into our worker queue; never synthesize OS keyboard input.
        unsafe {
            PostThreadMessageW(thread_id, WM_HOTKEY, WPARAM(11), LPARAM(0)).unwrap();
        }
        assert!(matches!(
            first.rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            Event::Pressed { key: 11, .. }
        ));
        let mut second = Hotkeys::new(egui::Context::default());
        second.configure(vec![11]);
        assert!(
            matches!(second.rx.recv_timeout(Duration::from_secs(3)).unwrap(),Event::Registered {errors,..} if !errors.is_empty())
        );
        drop(first);
        second.configure(Vec::new());
        let _ = second.rx.recv_timeout(Duration::from_secs(3)).unwrap();
        second.configure(vec![11]);
        assert!(
            matches!(second.rx.recv_timeout(Duration::from_secs(3)).unwrap(),Event::Registered {errors,..} if errors.is_empty())
        );
    }
}
