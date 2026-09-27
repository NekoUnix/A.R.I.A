use super::*;
use std::{path::PathBuf, sync::mpsc, time::Duration};
use wry::{WebView, WebViewBuilder};

enum Event {
    Loaded(u64, String),
    Title(u64, String),
    Popup(String),
    Download(String),
}
struct Tab {
    visible: bool,
    id: u64,
    view: WebView,
    title: String,
    url: String,
    zoom: f64,
}
#[derive(Clone, Copy, PartialEq)]
enum Panel {
    Web,
    Bookmarks,
    History,
    Downloads,
}
struct Browser {
    // Webviews must drop before their shared context.
    tabs: Vec<Tab>,
    web_context: wry::WebContext,
    next_id: u64,
    selected: usize,
    library: Library,
    tx: mpsc::SyncSender<Event>,
    rx: mpsc::Receiver<Event>,
    address: String,
    panel: Panel,
    downloads: Vec<String>,
    error: Option<String>,
    focus_address: bool,
}
fn data_dir() -> PathBuf {
    std::env::var_os("ARIA_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("ARIA")
        })
        .join("browser")
}
pub(super) fn run() -> eframe::Result {
    let directory = data_dir();
    std::fs::create_dir_all(&directory).map_err(|e| eframe::Error::AppCreation(Box::new(e)))?;
    eframe::run_native(
        "ARIA Studio Browser",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("ARIA · Studio browser")
                .with_inner_size([1120., 800.])
                .with_min_inner_size([800., 520.]),
            persistence_path: Some(directory.join("browser.ron")),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::theme::apply(
                &cc.egui_ctx,
                crate::theme::Settings::default().active.colors,
            );
            let (tx, rx) = mpsc::sync_channel(256);
            let mut library: Library = cc
                .storage
                .and_then(|s| eframe::get_value(s, "library"))
                .unwrap_or_default();
            library.sanitize();
            Ok(Box::new(Browser {
                tabs: vec![],
                web_context: wry::WebContext::new(Some(directory.join("webview"))),
                next_id: 1,
                selected: 0,
                library,
                tx,
                rx,
                address: String::new(),
                panel: Panel::Web,
                downloads: vec![],
                error: None,
                focus_address: true,
            }))
        }),
    )
}
impl Browser {
    fn add_tab(
        &mut self,
        frame: &eframe::Frame,
        url: Option<&str>,
        ctx: &egui::Context,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.tabs.len() < MAX_TABS,
            "Close a tab before opening another (12 maximum)"
        );
        let id = self.next_id;
        self.next_id += 1;
        let tx_load = self.tx.clone();
        let tx_title = self.tx.clone();
        let tx_popup = self.tx.clone();
        let tx_download = self.tx.clone();
        let repaint = ctx.clone();
        let mut builder = WebViewBuilder::new_with_web_context(&mut self.web_context)
            .with_visible(false)
            .with_focused(false)
            .with_devtools(false)
            .with_navigation_handler(|url| url == "about:blank" || allowed(&url))
            .with_on_page_load_handler(move |event, url| {
                if matches!(event, wry::PageLoadEvent::Finished) {
                    let _ = tx_load.try_send(Event::Loaded(id, url));
                    repaint.request_repaint();
                }
            })
            .with_document_title_changed_handler(move |title| {
                let _ = tx_title.try_send(Event::Title(id, title));
            })
            .with_new_window_req_handler(move |url, _| {
                if allowed(&url) {
                    let _ = tx_popup.try_send(Event::Popup(url));
                }
                wry::NewWindowResponse::Deny
            })
            .with_download_started_handler(|_, path| {
                let name = path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("download");
                let mut dialog = rfd::FileDialog::new()
                    .set_title("Save browser download")
                    .set_file_name(name);
                // Prefer the runtime's Downloads directory over unrelated file-picker history.
                if let Some(directory) = path.parent().filter(|p| p.is_absolute() && p.is_dir()) {
                    dialog = dialog.set_directory(directory);
                }
                if let Some(destination) = dialog.save_file() {
                    *path = destination;
                    true
                } else {
                    false
                }
            })
            .with_download_completed_handler(move |_, path, success| {
                let label = path
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Download".into());
                let _ = tx_download.try_send(Event::Download(format!(
                    "{}: {label}",
                    if success {
                        "Saved"
                    } else {
                        "Cancelled or failed"
                    }
                )));
            });
        builder = if let Some(url) = url {
            anyhow::ensure!(allowed(url), "Unsupported web address");
            builder.with_url(url)
        } else {
            builder.with_url("about:blank")
        };
        let view = builder.build_as_child(frame)?;
        self.tabs.push(Tab {
            visible: false,
            id,
            view,
            title: "New tab".into(),
            url: url.unwrap_or("").into(),
            zoom: 1.,
        });
        self.select(self.tabs.len() - 1);
        self.focus_address = true;
        Ok(())
    }
    fn select(&mut self, index: usize) {
        self.selected = index.min(self.tabs.len().saturating_sub(1));
        self.address = self
            .tabs
            .get(self.selected)
            .map(|t| t.url.clone())
            .unwrap_or_default();
        self.panel = Panel::Web;
    }
    fn navigate(&mut self) {
        let result = address(&self.address).and_then(|url| {
            if let Some(tab) = self.tabs.get(self.selected) {
                tab.view.load_url(&url)?;
            }
            self.address = url;
            self.panel = Panel::Web;
            Ok(())
        });
        self.error = result.err().map(|e| e.to_string());
    }
    fn events(&mut self, frame: &eframe::Frame, ctx: &egui::Context) {
        for _ in 0..256 {
            let Ok(event) = self.rx.try_recv() else {
                break;
            };
            match event {
                Event::Loaded(id, url) => {
                    if let Some((index, tab)) =
                        self.tabs.iter_mut().enumerate().find(|(_, t)| t.id == id)
                    {
                        tab.url = if allowed(&url) { url } else { String::new() };
                        self.library.visit(tab.title.clone(), tab.url.clone());
                        if index == self.selected && !ctx.egui_wants_keyboard_input() {
                            self.address = tab.url.clone();
                        }
                    }
                }
                Event::Title(id, title) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                        tab.title = title.chars().take(160).collect();
                    }
                }
                Event::Popup(url) => {
                    if let Err(e) = self.add_tab(frame, Some(&url), ctx) {
                        self.error = Some(e.to_string());
                    }
                }
                Event::Download(message) => {
                    self.downloads.insert(0, message);
                    self.downloads.truncate(100);
                }
            }
        }
    }
}
impl eframe::App for Browser {
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.events(frame, &ctx);
        if self.tabs.is_empty()
            && self.error.is_none()
            && let Err(e) = self.add_tab(frame, None, &ctx)
        {
            self.error = Some(format!(
                "Browser engine could not start: {e}. Install or repair Microsoft Edge WebView2 Runtime, then reopen this window."
            ));
        }
        crate::theme::workspace_backdrop(root);
        egui::Panel::top("browser-chrome")
            .frame(crate::theme::chrome(10.))
            .show(root, |ui| {
                let mut close = None;
                let mut select = None;
                egui::ScrollArea::horizontal()
                    .id_salt("tabs")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (index, tab) in self.tabs.iter().enumerate() {
                                if ui
                                    .selectable_label(
                                        index == self.selected,
                                        tab.title.chars().take(22).collect::<String>(),
                                    )
                                    .clicked()
                                {
                                    select = Some(index);
                                }
                                if ui.small_button("×").on_hover_text("Close tab").clicked() {
                                    close = Some(index);
                                }
                            }
                        });
                    });
                if let Some(index) = select {
                    self.select(index);
                }
                if let Some(index) = close {
                    self.tabs.remove(index);
                    let next = if index < self.selected {
                        self.selected - 1
                    } else {
                        self.selected
                    };
                    self.select(next);
                    self.error = None;
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.tabs.len() < MAX_TABS, egui::Button::new("+ Tab"))
                        .clicked()
                        && let Err(e) = self.add_tab(frame, None, &ctx)
                    {
                        self.error = Some(e.to_string());
                    }
                    if let Some(tab) = self.tabs.get(self.selected) {
                        if ui
                            .add_enabled(
                                tab.view.can_go_back().unwrap_or(false),
                                egui::Button::new("←"),
                            )
                            .clicked()
                        {
                            let _ = tab.view.go_back();
                        }
                        if ui
                            .add_enabled(
                                tab.view.can_go_forward().unwrap_or(false),
                                egui::Button::new("→"),
                            )
                            .clicked()
                        {
                            let _ = tab.view.go_forward();
                        }
                        if ui.button("Reload").clicked() {
                            let _ = tab.view.reload();
                        }
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.address)
                            .hint_text("Search or enter address")
                            .char_limit(8192)
                            .desired_width((ui.available_width() - 100.).max(100.)),
                    );
                    if self.focus_address {
                        response.request_focus();
                        self.focus_address = false;
                    }
                    if (response.clicked() || response.gained_focus())
                        && let Some(tab) = self.tabs.get(self.selected)
                    {
                        let _ = tab.view.focus_parent();
                    }
                    let enter =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Go").clicked() || enter {
                        self.navigate();
                    }
                    if let Some(tab) = self.tabs.get(self.selected) {
                        let saved = self.library.bookmarks.iter().any(|e| e.url == tab.url);
                        if ui
                            .add_enabled(
                                allowed(&tab.url),
                                egui::Button::new(if saved { "★" } else { "☆" }),
                            )
                            .on_hover_text("Toggle bookmark")
                            .clicked()
                        {
                            self.library.bookmark(Entry {
                                title: tab.title.clone(),
                                url: tab.url.clone(),
                            });
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    for (panel, name) in [
                        (Panel::Web, "Website"),
                        (Panel::Bookmarks, "Bookmarks"),
                        (Panel::History, "History"),
                        (Panel::Downloads, "Downloads"),
                    ] {
                        ui.selectable_value(&mut self.panel, panel, name);
                    }
                    if let Some(tab) = self.tabs.get_mut(self.selected)
                        && ui
                            .add(egui::Slider::new(&mut tab.zoom, 0.5..=2.).text("Zoom"))
                            .changed()
                    {
                        let _ = tab.view.zoom(tab.zoom);
                    }
                });
                if let Some(error) = &self.error {
                    ui.colored_label(crate::theme::orange(), error);
                }
            });
        egui::CentralPanel::default().show(root, |ui| {
            let rect = ui.max_rect(); let scale = ctx.pixels_per_point();
            for (index, tab) in self.tabs.iter_mut().enumerate() {
                let visible = self.panel == Panel::Web && index == self.selected && !tab.url.is_empty();
                if tab.visible != visible {
                    if let Err(error) = tab.view.set_visible(visible) {
                        self.error = Some(error.to_string());
                    } else {
                        tab.visible = visible;
                        if !visible && index == self.selected { let _ = tab.view.focus_parent(); }
                    }
                }
                if visible {
                    let _ = tab.view.set_bounds(wry::Rect {
                        position: wry::dpi::PhysicalPosition::new((rect.left()*scale) as i32, (rect.top()*scale) as i32).into(),
                        size: wry::dpi::PhysicalSize::new((rect.width()*scale).max(1.) as u32, (rect.height()*scale).max(1.) as u32).into(),
                    });
                }
            }
            match self.panel {
                Panel::Web => {
                    if self.tabs.get(self.selected).is_some_and(|tab| tab.url.is_empty()) {
                        ui.add_space(60.);
                        crate::theme::card(ui, |ui| {
                            ui.heading("Make room for your next idea.");
                            ui.label("Enter a website or search above. Your stage keeps running in ARIA.");
                            crate::theme::caption(ui, "Bookmarks and history stay on this computer.");
                        });
                    }
                },
                Panel::History | Panel::Bookmarks => {
                    let history = self.panel == Panel::History;
                    ui.heading(if history { "History" } else { "Bookmarks" });
                    let entries = if history { &mut self.library.history } else { &mut self.library.bookmarks };
                    if history && ui.button("Clear browsing history").clicked() { entries.clear(); }
                    let mut open = None; let mut remove = None;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (index, entry) in entries.iter().enumerate() {
                            ui.horizontal_wrapped(|ui| {
                                if ui.small_button("Remove").clicked() { remove = Some(index); }
                                if ui.link(&entry.title).on_hover_text(&entry.url).clicked() { open = Some(entry.url.clone()); }
                            });
                        }
                    });
                    if let Some(index) = remove { entries.remove(index); }
                    if let Some(url) = open { self.address = url; self.navigate(); }
                },
                Panel::Downloads => {
                    ui.heading("Downloads");
                    crate::theme::caption(ui, "Choose a save location for each download. Files never run automatically. Completion results for this window appear here; active progress is not yet displayed.");
                    egui::ScrollArea::vertical().show(ui, |ui| { for message in &self.downloads { ui.label(message); } });
                },
            }
        });
        ctx.request_repaint_after(Duration::from_millis(200));
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "library", &self.library);
    }
}
