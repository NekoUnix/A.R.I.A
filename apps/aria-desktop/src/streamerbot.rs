//! Streamer.bot local-HTTP setup and secret-free action code generation.
use crate::actions::{Choice, Command, Mode, Target};
use crate::effect_api::{Action, Api, Settings};
use base64::Engine;
use eframe::egui;

const TEMPLATE: &str = include_str!("../../../templates/streamerbot/AriaConnector.cs");
pub fn supports_mode(target: &Target) -> bool {
    matches!(
        target,
        Target::Avatar {
            command: Command::Pose
                | Command::Expression(_)
                | Command::Layers(_)
                | Command::Item(_)
                | Command::Image(_),
            ..
        }
    )
}
pub fn script(action: Option<&Action>, port: u16) -> String {
    let json = action
        .map(|a| serde_json::to_vec(a).expect("serializable action"))
        .unwrap_or_default();
    // Replace the constant only; preserve the generic template's fallback marker.
    TEMPLATE
        .replacen(
            "\"__ARIA_ACTION_BASE64__\"",
            &format!(
                "\"{}\"",
                base64::engine::general_purpose::STANDARD.encode(json)
            ),
            1,
        )
        .replace("DefaultPort = 39421;", &format!("DefaultPort = {port};"))
}
#[derive(Default)]
pub struct Setup {
    pub open: bool,
    selected: Option<String>,
    filter: String,
    mode: Mode,
    advanced: bool,
    json: String,
    message: Option<String>,
}
impl Setup {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        choices: &[Choice],
        api: &mut Api,
        settings: &mut Settings,
    ) {
        let mut open = self.open;
        egui::Window::new("Streamer.bot setup").id(egui::Id::new("streamerbot-setup"))
            .open(&mut open).default_width(650.).vscroll(true).show(ctx, |ui| {
                crate::help::button(ui, "streamerbot");
                ui.heading("1 · Enable ARIA's local connection");
                api.ui(ui, settings);
                ui.label(if api.listening() { "ARIA listener is running. Run the test script in Streamer.bot to check the complete connection." } else { "ARIA listener is not running yet. Enable it above and resolve any displayed error." });
                ui.separator();
                ui.heading("2 · Set up Streamer.bot once");
                ui.label("Open Global Variables → Persisted Globals. Add ariaApiKey as text and paste Copy API key. Add ariaPort with the port above. Keep both apps on the same PC.");
                ui.label("Create an ARIA action in Streamer.bot. Add Core → C# → Execute C# Code and paste the script below. Compile it, then test it before adding a chat, reward or event trigger. No Streamer.bot WebSocket server is needed.");
                if ui.button("Copy connection-test C#").clicked() { ctx.copy_text(script(None,settings.port)); self.message=Some("Paste in a separate Streamer.bot test action. It reads status without changing your avatar.".into()); }
                ui.separator();
                ui.heading("3 · Choose what the stream event does");
                ui.label("Every saved hotkey/action target is available here. Load the avatar to discover its expressions, groups and effects. Profile load/switch actions are available while unloaded.");
                ui.text_edit_singleline(&mut self.filter);
                let filter=self.filter.to_lowercase();
                let mut entries: Vec<(String, Action)> = choices.iter().map(|c| (c.label.clone(),Action::Workspace{target:c.target.clone(),mode:Mode::Toggle})).collect();
                entries.push(("Stop all pending actions".into(),Action::StopActions));
                entries.push(("Current avatar · Release all parameter holds".into(),Action::ReleaseParameters{ids:vec![]}));
                entries.push(("Current avatar · Save profile".into(),Action::SaveProfile));
                for (index,name) in crate::output::NAMES.iter().enumerate() {
                    for open in [true,false] { entries.push((format!("Output · {name} · {}",if open {"Open"} else {"Close"}),Action::Output{index,open,zoom:None,position:None})); }
                }
                egui::ScrollArea::vertical().id_salt("streamerbot-choices").max_height(180.).show(ui,|ui| {
                    for (label,action) in &entries {
                        if !label.to_lowercase().contains(&filter) { continue; }
                        let key=serde_json::to_string(action).unwrap();
                        if ui.selectable_label(self.selected.as_ref()==Some(&key),label).clicked() {
                            self.selected=Some(key); self.mode=Mode::Toggle; self.advanced=false;
                        }
                    }
                });
                let mut action = self.selected.as_ref().and_then(|key| entries.iter().find(|(_,a)| serde_json::to_string(a).ok().as_ref()==Some(key))).map(|(_,a)| a.clone());
                if self.selected.is_some() && action.is_none() { ui.colored_label(egui::Color32::YELLOW,"Target unavailable. Reload its avatar or choose another action."); }
                if let Some(Action::Workspace{target,mode}) = &mut action {
                    if supports_mode(target) {
                        ui.horizontal(|ui| { for (value,label) in [(Mode::Toggle,"Toggle"),(Mode::On,"On"),(Mode::Off,"Off")] {ui.selectable_value(&mut self.mode,value,label);} });
                        *mode=self.mode;
                    } else { ui.small("One-shot action: runs once per trigger."); }
                }
                if ui.checkbox(&mut self.advanced,"Advanced: edit API action JSON").changed() && self.advanced {
                    self.json=action.as_ref().map(|a|serde_json::to_string_pretty(a).unwrap()).unwrap_or_else(||"{\"type\":\"theme\",\"name\":\"Sakura\"}".into());
                }
                if self.advanced {
                    ui.small("Supports every documented control action, including parameter values, themes and output framing. Current-avatar commands follow the editing stage. Explicit workspace targets keep their profile ID.");
                    ui.add(egui::TextEdit::multiline(&mut self.json).code_editor().desired_rows(6).desired_width(f32::INFINITY));
                    action=match serde_json::from_str::<Action>(&self.json) {Ok(a)=>Some(a),Err(e)=>{ui.colored_label(egui::Color32::YELLOW,format!("Invalid action: {e}"));None}};
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(action.is_some(),egui::Button::new("Copy action C#")).clicked() {ctx.copy_text(script(action.as_ref(),settings.port)); self.message=Some("Copied. Paste into Streamer.bot's Execute C# Code. Your API key is not included.".into());}
                    if ui.add_enabled(action.is_some(),egui::Button::new("Copy action JSON")).clicked() {ctx.copy_text(serde_json::to_string(action.as_ref().unwrap()).unwrap());}
                });
                ui.small("Use a sequential ARIA queue in Streamer.bot. Scripts wait for an applied/rejected result; delayed graphs or model loading may take time. Never automatically repeat a timed-out trigger. Rotating the key requires updating ariaApiKey.");
                if let Some(message)=&self.message {ui.label(message);}
            });
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_script_roundtrips_unicode_targets_without_code_injection_or_secrets() {
        let action = Action::Workspace {
            target: Target::Avatar {
                profile: 42,
                command: Command::Preset("Odette \"猫\"\npath\\pose".into()),
            },
            mode: Mode::Toggle,
        };
        let code = script(Some(&action), 40123);
        let encoded = code
            .split("DefaultActionBase64 = \"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let json = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&json).unwrap(),
            serde_json::to_value(action).unwrap()
        );
        assert!(code.contains("DefaultPort = 40123;"));
        assert!(script(None, 39421).contains("DefaultActionBase64 = \"\";"));
        assert!(!supports_mode(&Target::Graph(1)));
        assert!(supports_mode(&Target::Avatar {
            profile: 42,
            command: Command::Item(1)
        }));
    }
}
