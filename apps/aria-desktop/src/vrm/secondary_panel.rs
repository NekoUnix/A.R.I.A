//! Inspect every imported skeleton node and review secondary-motion guesses.
use super::{Avatar, secondary};
use aria_core::vrm::Secondary;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, avatar: &mut Avatar, settings: &mut Secondary) {
    ui.checkbox(
        &mut settings.auto_detect,
        "Guess flexible bones automatically",
    );
    ui.small("Recognizes breasts, belly, hair, tails, ears and clothing. Tracking, fingers, facial controls and twist helpers stay rigid. Other bones still follow their parents; review uncertain ones below.");
    ui.collapsing("Bone inspector · all imported bones", |ui| {
        let inventory = &avatar.bone_inventory;
        let asset = &avatar.asset;
        let eligible = secondary::candidates(asset);
        let query = avatar.bone_search.to_lowercase();
        let visible: Vec<_> = asset.order.iter().copied().filter(|n| inventory.skeleton.contains(n))
            .filter(|n| query.is_empty() || asset.nodes[*n].name.to_lowercase().contains(&query)
                || n.to_string() == query).collect();
        ui.label(format!("{} skeleton nodes · {} directly weighted · {} matching",
            inventory.skeleton.len(), inventory.weighted.len(), visible.len()));
        ui.add(egui::TextEdit::singleline(&mut avatar.bone_search).hint_text("Find a bone by name or number…"));
        ui.small("Auto uses the detected role. Simulate adds this branch to physics. Keep rigid excludes it and its children from generated physics. Exported VRM physics is edited in Spring groups below. Every choice saves with this avatar.");
        let row_height = ui.text_style_height(&egui::TextStyle::Body) * 2.
            + ui.text_style_height(&egui::TextStyle::Small) + ui.spacing().item_spacing.y * 4. + 14.;
        egui::ScrollArea::vertical().id_salt("all-skeleton-bones").max_height(380.)
            .show_rows(ui, row_height, visible.len(), |ui, rows| {
                for row in rows {
                    let n = visible[row];
                    let node = &asset.nodes[n];
                    ui.allocate_ui(egui::vec2(ui.available_width(), row_height), |ui| ui.push_id(n, |ui| {
                        ui.horizontal(|ui| {
                            ui.weak(format!("#{n}"));
                            ui.add(egui::Label::new(egui::RichText::new(if node.name.is_empty() { "Unnamed bone" } else { &node.name }).strong()).truncate()).on_hover_text(&node.name);
                        });
                        let group = avatar.spring_groups.iter().find(|g| g.joints.iter().any(|j| j.node == n));
                        let mut ancestor = Some(n);
                        let mut excluded = false;
                        let mut authored = false;
                        while let Some(a) = ancestor {
                            excluded |= settings.excluded_roots.contains(&a);
                            authored |= asset.springs.iter().any(|g| g.joints.iter().any(|j| j.node == a));
                            ancestor = asset.nodes[a].parent;
                        }
                        let reason = if !eligible.contains(&n) {
                            "Tracking / rigid helper / unweighted endpoint"
                        } else if authored {
                            "Exported physics — use its Spring group"
                        } else if excluded {
                            "Kept rigid by this bone or an ancestor"
                        } else if group.is_some() {
                            "Physics group — tune its strength below"
                        } else {
                            secondary::named_secondary(&node.name).unwrap_or("Unclassified — follows its parent; review manually")
                        };
                        ui.add(egui::Label::new(egui::RichText::new(reason).small()).truncate()).on_hover_text(format!("{reason}\nParent: {}\n{}\n{}",
                            node.parent.map_or("None", |p| asset.nodes[p].name.as_str()),
                            if inventory.weighted.contains(&n) { "Directly deforms mesh vertices" } else { "Transform or endpoint; may drive weighted children" },
                            if inventory.leaf_tips.contains_key(&n) { "Uses a virtual endpoint estimated from its weighted mesh" } else { "Uses the exported hierarchy and endpoint" }));
                        ui.horizontal(|ui| {
                            let manual = settings.manual_roots.contains(&n);
                            let rigid = settings.excluded_roots.contains(&n);
                            if ui.selectable_label(!manual && !rigid, "Auto").clicked() {
                                settings.manual_roots.remove(&n);
                                settings.excluded_roots.remove(&n);
                            }
                            let can_simulate = eligible.contains(&n) && !authored
                                && (inventory.tip(asset, n).is_some() || !node.children.is_empty())
                                && (manual || settings.manual_roots.len() < 128);
                            if ui.add_enabled(can_simulate, egui::Button::new("Simulate").selected(manual)).clicked() {
                                settings.manual_roots.insert(n);
                                settings.excluded_roots.remove(&n);
                            }
                            if ui.add_enabled(eligible.contains(&n) && !authored, egui::Button::new("Keep rigid").selected(rigid)).clicked() {
                                settings.manual_roots.remove(&n);
                                settings.excluded_roots.insert(n);
                            }
                        });
                        ui.separator();
                    }));
                }
            });
        ui.small("A guess cannot recover Unity PhysBone settings. Unknown names need review. Bones with no mesh influence cannot create new deformation; unrigged geometry needs a rigged export.");
    });
}
