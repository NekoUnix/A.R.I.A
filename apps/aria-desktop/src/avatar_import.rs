//! Guided primary-avatar import. Accessories remain independent from the avatar type.
use crate::{help, media, theme};
use aria_core::image_actions::Trigger;
use eframe::egui;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Images,
    Live2d,
}
#[derive(Clone)]
pub struct Artwork {
    pub path: PathBuf,
    pub trigger: Trigger,
    pub info: Result<media::Info, String>,
}
impl Artwork {
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        let trigger = if name.contains("deafen") || name.contains("mute") {
            Trigger::Manual
        } else if name.contains("talk") {
            Trigger::Talking
        } else if name.contains("idle") || name.contains("inactive") || name.contains("quiet") {
            Trigger::Idle
        } else if name.contains("blink") {
            Trigger::Blink
        } else {
            Trigger::Manual
        };
        let info = media::inspect(&path).map_err(|e| format!("{e:#}"));
        Self {
            path,
            trigger,
            info,
        }
    }
}
pub enum Request {
    Images { artwork: Vec<Artwork>, budget: u32 },
    Live2d(PathBuf),
    Cancel,
    Microphone,
    Controls,
}
pub struct Wizard {
    pub open: bool,
    pub kind: Option<Kind>,
    pub artwork: Vec<Artwork>,
    pub budget: u32,
    pub model: Option<PathBuf>,
    step: usize,
    model_summary: Option<Result<String, String>>,
    pub error: Option<String>,
    pub complete: bool,
}
impl Default for Wizard {
    fn default() -> Self {
        Self {
            open: false,
            kind: None,
            artwork: Vec::new(),
            budget: aria_core::asset_limits::GIF_PLAYBACK_MIB,
            model: None,
            step: 0,
            model_summary: None,
            error: None,
            complete: false,
        }
    }
}
impl Wizard {
    pub fn start(&mut self, kind: Option<Kind>) {
        *self = Self {
            open: true,
            step: usize::from(kind.is_some()),
            kind,
            ..Self::default()
        };
    }
    pub fn add(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            if !self.artwork.iter().any(|a| a.path == path) {
                self.artwork.push(Artwork::new(path));
            }
        }
        if !self.artwork.iter().any(|a| a.trigger == Trigger::Idle)
            && let Some(a) = self.artwork.first_mut()
        {
            a.trigger = Trigger::Idle;
        }
        self.error = None;
    }
    pub fn finished(&mut self) {
        self.complete = true;
        self.error = None;
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        core: &mut String,
        progress: Option<(usize, usize)>,
    ) -> Option<Request> {
        if !self.open {
            return None;
        }
        let mut open = true;
        let mut request = None;
        egui::Window::new("Import your avatar").id(egui::Id::new("guided-avatar-import"))
            .open(&mut open).collapsible(false).default_width(650.0).resizable(true).show(ctx,|ui| {
            help::label(ui,"Choose → Prepare → Import → Create","guided-import");
            egui::ScrollArea::vertical().max_height(560.0).show(ui,|ui| {
                if self.complete {
                    ui.heading("Your avatar is ready");
                    ui.label("The studio now shows tools for this avatar type. Save profile keeps its settings together.");
                    if self.kind==Some(Kind::Images) {
                        ui.label("Additional image actions load in the background. Idle is the fallback; talking responds to the microphone or phone mouth input. Mute and deafened images are manual actions until you assign hotkeys.");
                        ui.label("Idle artwork  →  talking input  →  Talking artwork");
                        if ui.button("Set up microphone").clicked() { request=Some(Request::Microphone); }
                    } else {
                        ui.label("Use Tracking for phone/microphone inputs, and Avatar in the Inspector for physics groups and expression hotkeys. The exported model defines the available controls.");
                    }
                    if ui.button("Open avatar controls").clicked() { request=Some(Request::Controls); }
                    return;
                }
                if let Some((done,total))=progress {
                    ui.heading("Importing artwork");
                    ui.label(format!("Preparing frame {done} of {total}. The current avatar stays available until import succeeds."));
                    ui.add(egui::ProgressBar::new(if total>0 {done as f32/total as f32}else{0.0}).show_percentage());
                    theme::caption(ui,"Large GIFs are composited one frame at a time and fitted to the selected playback memory budget. This can take a little while.");
                    if ui.button("Cancel import").clicked() {request=Some(Request::Cancel);}
                    return;
                }
                if self.step==0 {
                    ui.heading("1. Choose your avatar type");
                    ui.label("PNG / GIF uses artwork states for idle, talking, blinking and hotkeys. Live2D uses a rigged Cubism model with parameters, expressions and physics.");
                    ui.columns(2,|cols| {
                        if cols[0].add_sized([cols[0].available_width(),58.0],egui::Button::new("PNG / GIF avatar")).clicked() {self.kind=Some(Kind::Images);self.step=1;}
                        if cols[1].add_sized([cols[1].available_width(),58.0],egui::Button::new("Live2D avatar")).clicked() {self.kind=Some(Kind::Live2d);self.step=1;}
                    });
                } else if self.step==1 {
                    ui.heading("2. Choose and prepare files");
                    if self.kind==Some(Kind::Images) {
                        ui.label("Choose one or several PNG/GIF files. Mark exactly one as Idle / base; assign the others a starting role. You can change triggers, fades and hotkeys after import.");
                        ui.horizontal(|ui| {
                            if ui.button("Choose PNG / GIF files…").clicked() && let Some(paths)=rfd::FileDialog::new().add_filter("PNG / GIF",&["png","gif"]).pick_files(){self.add(paths);}
                            if ui.button("Choose artwork folder…").clicked() && let Some(folder)=rfd::FileDialog::new().pick_folder(){
                                match std::fs::read_dir(folder) {
                                    Ok(entries)=>self.add(entries.filter_map(Result::ok).map(|e|e.path()).filter(|p|p.is_file()&&p.extension().is_some_and(|e|e.eq_ignore_ascii_case("png")||e.eq_ignore_ascii_case("gif"))).collect()),
                                    Err(e)=>self.error=Some(e.to_string()),
                                }
                            }
                        });
                        let mut remove=None;
                        for (index,art) in self.artwork.iter_mut().enumerate() {
                            ui.push_id(index,|ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized([(ui.available_width()-235.0).max(100.0),24.0],egui::Label::new(art.path.file_name().unwrap_or_default().to_string_lossy()).truncate()).on_hover_text(art.path.display().to_string());
                                    egui::ComboBox::from_id_salt("role").selected_text(role(art.trigger)).show_ui(ui,|ui|{
                                        for (value,label) in [(Trigger::Idle,"Idle / base"),(Trigger::Talking,"Talking"),(Trigger::Quiet,"Quiet"),(Trigger::Blink,"Blink"),(Trigger::Manual,"Manual / hotkey")]{ui.selectable_value(&mut art.trigger,value,label);}
                                    });
                                    if ui.small_button("Remove").clicked(){remove=Some(index);}
                                });
                                match &art.info {
                                    Ok(info)=>{ui.small(format!("{} × {} · {} frames · {:.2}s · {:.1} MiB file",info.size[0],info.size[1],info.frames,info.duration,info.file_bytes as f64/1048576.0));}
                                    Err(e)=>{ui.colored_label(egui::Color32::LIGHT_RED,e);}
                                }
                            });
                        }
                        if let Some(index)=remove {self.artwork.remove(index);}
                        if self.artwork.len()>128 {ui.colored_label(egui::Color32::LIGHT_RED,"Choose up to 128 image actions per avatar. Remove some files to continue.");}
                        help::control(ui,"gif-memory",|ui|egui::ComboBox::from_id_salt("import-gif-budget").selected_text(format!("{} MiB playback per GIF",self.budget)).show_ui(ui,|ui|{
                            for n in [64,128,256,512,1024] {ui.selectable_value(&mut self.budget,n,format!("{n} MiB per GIF"));}
                        }));
                        theme::caption(ui,"The default 256 MiB fits large animations without storing all full-size frames. Higher budgets retain more detail and consume more VRAM. Source files are unchanged.");
                    } else {
                        ui.label("Choose the exported .model3.json, or its .moc3 with a matching manifest beside it. Keep the atlas PNGs, physics and expressions in the exported folder structure.");
                        if ui.button("Choose Live2D export…").clicked() && let Some(path)=rfd::FileDialog::new().add_filter("Cubism export",&["json","moc3"]).pick_file(){self.model=Some(path);self.model_summary=None;self.error=None;}
                        if let Some(path)=&self.model {ui.label(path.display().to_string());}
                        help::label(ui,"Cubism Core x64 runtime","runtime");
                        ui.label("Select Core/dll/windows/x86_64/Live2DCubismCore.dll from the official Native SDK. ARIA remembers this path.");
                        ui.text_edit_singleline(core);
                        if ui.button("Choose Cubism Core DLL…").clicked()&&let Some(path)=rfd::FileDialog::new().add_filter("Cubism Core",&["dll"]).pick_file(){*core=path.display().to_string();}
                        ui.hyperlink_to("Get the official Cubism Native SDK","https://www.live2d.com/en/sdk/download/native/");
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Back").clicked(){self.step=0;}
                        let ready = if self.kind==Some(Kind::Images) {
                            !self.artwork.is_empty()&&self.artwork.len()<=128&&self.artwork.iter().all(|a|a.info.is_ok())&&self.artwork.iter().filter(|a|a.trigger==Trigger::Idle).count()==1
                        }else{self.model.as_ref().is_some_and(|p|p.is_file())&&Path::new(core.trim()).is_file()};
                        if ui.add_enabled(ready,egui::Button::new("Review import →")).clicked(){self.step=2;self.error=None;}
                    });
                } else {
                    ui.heading("3. Review and import");
                    if self.kind==Some(Kind::Images) {
                        let maximum=ctx.input(|i|i.max_texture_side as u32);
                        let mut retained=0u64;
                        for art in &self.artwork {
                            if let Ok(info)=&art.info {
                                let size=if info.frames>1 {info.playback_size(maximum,u64::from(self.budget)*1048576)}else{aria_core::asset_limits::texture_size(info.size[0],info.size[1],maximum)};
                                retained+=u64::from(size[0])*u64::from(size[1])*4*info.frames as u64;
                                ui.label(format!("{}: {} → {} × {} playback",art.path.file_name().unwrap_or_default().to_string_lossy(),role(art.trigger),size[0],size[1]));
                            }
                        }
                        ui.label("Original files, timing and artwork identity are preserved. Playback textures may be smaller to fit the memory budget. Existing settings for the same base artwork are restored; new action files are added to that profile.");
                        ui.label(format!("Selected artwork: {:.0} MiB of playback textures",retained as f64/1048576.0));
                        let fits=retained<=aria_core::asset_limits::IMAGE_COLLECTION;
                        if !fits {ui.colored_label(egui::Color32::LIGHT_RED,"This selection exceeds the 2560 MiB collection budget. Go back and lower the per-GIF budget or remove artwork.");}
                        if ui.add_enabled(fits,egui::Button::new("Import PNG / GIF avatar")).clicked(){request=Some(Request::Images{artwork:self.artwork.clone(),budget:self.budget});}
                    } else if let Some(path)=&self.model {
                        let summary=self.model_summary.get_or_insert_with(||aria_model::load_files(path).map(|files|format!("{} atlas textures · {} expressions · physics {}",files.textures.len(),files.expressions.len(),if files.physics.is_some(){"included"}else{"not included"})).map_err(|e|format!("{e:#}")));
                        match summary {Ok(info)=>{ui.label(info.as_str());},Err(e)=>{ui.colored_label(egui::Color32::LIGHT_RED,e.as_str());ui.small("For a bare moc3, the next dialog can pair atlas textures manually.");}}
                        ui.label("Import loads this model's parameters and optional tracking profile, physics groups and expressions. Saved settings are restored for this model only.");
                        if ui.button("Import Live2D avatar").clicked(){request=Some(Request::Live2d(path.clone()));}
                    }
                    if ui.button("Back to files").clicked(){self.step=1;}
                }
                if let Some(error)=&self.error {ui.colored_label(egui::Color32::LIGHT_RED,error);}
            });
        });
        if !open {
            self.open = false;
            if progress.is_some() {
                request = Some(Request::Cancel);
            }
        }
        request
    }
}
fn role(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Idle => "Idle / base",
        Trigger::Talking => "Talking",
        Trigger::Quiet => "Quiet",
        Trigger::Blink => "Blink",
        _ => "Manual / hotkey",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_roles_and_duplicate_selection_preserve_one_base() {
        let dir = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for name in [
            "GIFTUBER DEAFENED.png",
            "GIFTUBER INACTIVE.png",
            "GIFTUBER MUTE.png",
            "GIFTUBER TALKING.png",
        ] {
            let path = dir.path().join(name);
            image::RgbaImage::from_pixel(16, 16, image::Rgba([120, 80, 60, 255]))
                .save(&path)
                .unwrap();
            paths.push(path);
        }
        let mut wizard = Wizard::default();
        wizard.start(Some(Kind::Images));
        wizard.add(paths.clone());
        wizard.add(paths);
        assert_eq!(wizard.artwork.len(), 4);
        assert_eq!(
            wizard.artwork.iter().map(|a| a.trigger).collect::<Vec<_>>(),
            vec![
                Trigger::Manual,
                Trigger::Idle,
                Trigger::Manual,
                Trigger::Talking
            ]
        );
        assert!(wizard.artwork.iter().all(|a| a.info.is_ok()));
        wizard.start(Some(Kind::Live2d));
        assert!(wizard.artwork.is_empty());
        assert_eq!(wizard.step, 1);
    }
}
