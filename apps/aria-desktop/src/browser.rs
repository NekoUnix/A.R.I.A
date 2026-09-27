//! Browser metadata and launcher. Web content has no ARIA command bridge.
use eframe::egui;
use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod native;

const MAX_ENTRIES: usize = 500;
const MAX_TABS: usize = 12;

fn address(input: &str) -> anyhow::Result<String> {
    let input = input.trim();
    anyhow::ensure!(
        !input.is_empty() && input.len() <= 8192,
        "Enter a web address"
    );
    let candidate = if input.contains("://") || input.starts_with("about:") {
        input.to_owned()
    } else if !input.contains(char::is_whitespace)
        && (input.contains('.') || input.starts_with("localhost"))
    {
        format!("https://{input}")
    } else {
        let mut url = reqwest::Url::parse("https://www.google.com/search")?;
        url.query_pairs_mut().append_pair("q", input);
        return Ok(url.to_string());
    };
    let url = reqwest::Url::parse(&candidate)?;
    anyhow::ensure!(
        allowed(&candidate),
        "Only HTTP and HTTPS websites are supported"
    );
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Use the website's sign-in page instead of credentials in a URL"
    );
    Ok(url.to_string())
}

fn allowed(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    title: String,
    url: String,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Library {
    bookmarks: Vec<Entry>,
    history: Vec<Entry>,
}
impl Library {
    fn sanitize(&mut self) {
        for entries in [&mut self.bookmarks, &mut self.history] {
            entries.retain(|e| e.url.len() <= 8192 && allowed(&e.url));
            entries.truncate(MAX_ENTRIES);
            for entry in entries {
                entry.title = entry.title.chars().take(160).collect();
            }
        }
    }
    fn visit(&mut self, title: String, url: String) {
        if !allowed(&url) || url.len() > 8192 {
            return;
        }
        self.history.retain(|e| e.url != url);
        self.history.insert(
            0,
            Entry {
                title: title.chars().take(160).collect(),
                url,
            },
        );
        self.history.truncate(MAX_ENTRIES);
    }
    fn bookmark(&mut self, entry: Entry) {
        if let Some(index) = self.bookmarks.iter().position(|e| e.url == entry.url) {
            self.bookmarks.remove(index);
        } else if allowed(&entry.url) && self.bookmarks.len() < MAX_ENTRIES {
            self.bookmarks.push(entry);
        }
    }
}

#[derive(Default)]
pub struct Launcher {
    child: Option<std::process::Child>,
    error: Option<String>,
}
impl Launcher {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        self.error =
                            Some("Browser closed unexpectedly. Open it again to retry.".into());
                    }
                    self.child = None;
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                    self.child = None;
                }
                _ => {}
            }
        }
        if ui
            .add_enabled(
                cfg!(windows) && self.child.is_none(),
                egui::Button::new("Open studio browser"),
            )
            .on_hover_text("Tabs, bookmarks, history and downloads in a separate ARIA window")
            .clicked()
        {
            match launch() {
                Ok(child) => {
                    self.child = Some(child);
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if self.child.is_some() {
            crate::theme::caption(ui, "Browser window is open");
        }
        if !cfg!(windows) {
            crate::theme::caption(
                ui,
                "Studio browser currently requires Windows and WebView2.",
            );
        }
        if let Some(error) = &self.error {
            ui.colored_label(crate::theme::orange(), error);
        }
    }
}
fn launch() -> anyhow::Result<std::process::Child> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.arg("--studio-browser");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command.spawn()?)
}

pub fn run() -> eframe::Result {
    #[cfg(windows)]
    {
        native::run()
    }
    #[cfg(not(windows))]
    {
        Err(eframe::Error::AppCreation(
            "Studio browser requires Windows".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn addresses_cannot_invoke_local_files_scripts_or_credentials() {
        for value in [
            "file:///C:/private.txt",
            "javascript://alert(1)",
            "aria://command",
            "https://me:secret@example.org",
            "data://text/html",
            "about:blank",
        ] {
            assert!(address(value).is_err(), "{value}");
            assert!(!allowed(value));
        }
        assert_eq!(address("example.org").unwrap(), "https://example.org/");
        assert_eq!(
            address("http://localhost:3210/test").unwrap(),
            "http://localhost:3210/test"
        );
        assert!(
            address("avatar movement & lighting")
                .unwrap()
                .contains("q=avatar+movement+%26+lighting")
        );
    }
    #[test]
    fn history_and_bookmarks_are_bounded_deduplicated_and_repairable() {
        let mut library = Library::default();
        for i in 0..600 {
            library.visit(format!("Title {i}"), format!("https://example.org/{i}"));
        }
        assert_eq!(library.history.len(), MAX_ENTRIES);
        library.visit("Updated".into(), "https://example.org/599".into());
        assert_eq!(library.history.len(), MAX_ENTRIES);
        assert_eq!(library.history[0].title, "Updated");
        let entry = library.history[0].clone();
        library.bookmark(entry.clone());
        library.bookmark(entry);
        assert!(library.bookmarks.is_empty());
        library.history.push(Entry {
            title: "bad".into(),
            url: "file:///secret".into(),
        });
        library.sanitize();
        assert_eq!(library.history.len(), MAX_ENTRIES);
        assert!(library.history.iter().all(|e| allowed(&e.url)));
    }
}
