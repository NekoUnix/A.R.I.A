//! Conservative secondary chains for exports without complete spring metadata.
use super::{
    asset::Asset,
    spring::{Group, Joint},
};
use aria_core::vrm::Secondary;
use glam::Vec3;
use std::collections::{BTreeMap, BTreeSet};

pub fn named_secondary(name: &str) -> Option<&'static str> {
    let name = name.to_lowercase();
    let matches = |words: &[&str]| words.iter().any(|word| name.contains(word));
    if rigid_helper(&name) {
        return None;
    }
    if matches(&[
        "breast", "bust", "boob", "oppai", "胸", "乳", "belly", "glute",
    ]) || (name.contains("butt") && !name.contains("button"))
    {
        return Some("Flexible body / breast bone");
    }
    if matches(&[
        "hair", "tail", "ponytail", "ahoge", "kemomimi", "髪", "尻尾", "耳",
    ]) {
        return Some("Hair, tail or animal ear");
    }
    if matches(&[
        "ribbon",
        "skirt",
        "sleeve",
        "cape",
        "ear",
        "bow",
        "accessory",
        "accroot",
        "strap",
        "string",
        "tassel",
        "cloth",
        "dress",
        "coat",
        "belt",
        "necklace",
        "スカート",
        "リボン",
    ]) {
        return Some("Clothing or accessory");
    }
    None
}

fn rigid_helper(name: &str) -> bool {
    let name = name.to_lowercase();
    [
        "eyebrow",
        "eyelid",
        "jaw",
        "tongue",
        "cheek",
        "nose",
        "finger",
        "thumb",
        "index",
        "middle",
        "ringdistal",
        "ringintermediate",
        "ringproximal",
        "little",
        "pinky",
        "hallux",
        "toe",
        "twist",
        "corrective",
        "forearm",
        "upperarm",
        "lowerarm",
        "upperleg",
        "lowerleg",
    ]
    .iter()
    .any(|word| name.contains(word))
}

/// Cached once per asset, independent of the current tracking assignments.
/// Inverse bind matrices put weighted vertices into each joint's local rest space.
#[derive(Default)]
pub struct Inventory {
    pub skeleton: BTreeSet<usize>,
    pub weighted: BTreeSet<usize>,
    pub leaf_tips: BTreeMap<usize, Vec3>,
    pub volumes: BTreeMap<usize, [Vec3; 2]>,
}
impl Inventory {
    pub fn new(asset: &Asset) -> Self {
        let mut result = Self::default();
        let mut sums = vec![(Vec3::ZERO, 0f32, 0f32); asset.nodes.len()];
        for skin in &asset.skins {
            for &joint in &skin.joints {
                let mut next = Some(joint);
                while let Some(n) = next {
                    if !result.skeleton.insert(n) {
                        break;
                    }
                    next = asset.nodes[n].parent;
                }
            }
        }
        // Exported end nodes often carry no weights but still define a bone's length.
        let mesh_nodes: BTreeSet<_> = asset.geometry.iter().map(|g| g.node).collect();
        for n in result.skeleton.clone() {
            for &child in &asset.nodes[n].children {
                if !mesh_nodes.contains(&child) && asset.nodes[child].children.is_empty() {
                    result.skeleton.insert(child);
                }
            }
        }
        for geometry in &asset.geometry {
            let Some(skin) = geometry.skin.map(|s| &asset.skins[s]) else {
                continue;
            };
            for vertex in &geometry.vertices {
                let dominant = vertex.weights.into_iter().fold(0., f32::max);
                for (&joint, &weight) in vertex.joints.iter().zip(&vertex.weights) {
                    if weight <= 0.001 {
                        continue;
                    }
                    let Some(&n) = skin.joints.get(joint as usize) else {
                        continue;
                    };
                    result.weighted.insert(n);
                    let point =
                        skin.inverse[joint as usize].transform_point3(Vec3::from(vertex.position));
                    if weight >= 0.25 && weight >= dominant {
                        let bounds = result.volumes.entry(n).or_insert([point, point]);
                        bounds[0] = bounds[0].min(point);
                        bounds[1] = bounds[1].max(point);
                    }
                    if !asset.nodes[n].children.is_empty() {
                        continue;
                    }
                    let sum = &mut sums[n];
                    sum.0 += point * weight;
                    sum.1 += weight;
                    sum.2 += point.length_squared() * weight;
                }
            }
        }
        let extent = (asset.bounds[1] - asset.bounds[0]).max_element().max(0.001);
        for (n, (sum, weight, radius)) in sums.into_iter().enumerate() {
            if weight <= 0. {
                continue;
            }
            let mean = sum / weight;
            let radius = (radius / weight).sqrt();
            let tip = if mean.length() > radius * 0.1 {
                mean
            } else {
                Vec3::Y * radius * 0.5
            };
            let length = asset.nodes[n].world.transform_vector3(tip).length();
            if tip.is_finite() && length.is_finite() && length > extent * 1e-6 {
                result.leaf_tips.insert(
                    n,
                    tip * (length.clamp(extent * 0.003, extent * 0.12) / length),
                );
            }
        }
        result
    }
    pub fn tip(&self, asset: &Asset, node: usize) -> Option<Vec3> {
        let children: Vec<_> = asset.nodes[node]
            .children
            .iter()
            .filter(|c| self.skeleton.contains(c))
            .collect();
        if children.len() == 1 {
            let tip = asset.nodes[*children[0]].translation;
            return (tip.length_squared() > 1e-10).then_some(tip);
        }
        if children.is_empty() {
            self.leaf_tips.get(&node).copied()
        } else {
            None
        }
    }
}
/// Humanoid bones and their ancestors stay rigid; only active skin skeletons qualify.
pub fn candidates(asset: &Asset) -> BTreeSet<usize> {
    let mut protected = BTreeSet::new();
    for &node in asset.bones.values() {
        let mut next = Some(node);
        while let Some(n) = next {
            protected.insert(n);
            next = asset.nodes[n].parent;
        }
    }
    // Unmapped fingers, facial controls and twist helpers still belong to the rigid rig.
    for (n, node) in asset.nodes.iter().enumerate() {
        if rigid_helper(&node.name) {
            let mut next = Some(n);
            while let Some(p) = next {
                if !protected.insert(p) {
                    break;
                }
                next = asset.nodes[p].parent;
            }
        }
    }
    let mut eligible = BTreeSet::new();
    for geometry in &asset.geometry {
        if let Some(skin) = geometry.skin {
            for &node in &asset.skins[skin].joints {
                let mut next = Some(node);
                while let Some(n) = next {
                    if protected.contains(&n) {
                        break;
                    }
                    eligible.insert(n);
                    next = asset.nodes[n].parent;
                }
            }
        }
    }
    eligible
}

#[cfg(test)]
pub fn groups(asset: &Asset, settings: &Secondary) -> Vec<Group> {
    groups_with_inventory(asset, settings, &Inventory::new(asset))
}

pub fn groups_with_inventory(
    asset: &Asset,
    settings: &Secondary,
    inventory: &Inventory,
) -> Vec<Group> {
    let mut groups = asset.springs.clone();
    let eligible = candidates(asset);
    let mut used: BTreeSet<_> = groups
        .iter()
        .flat_map(|g| g.joints.iter().map(|j| j.node))
        .collect();
    let mut budget = 4096usize.saturating_sub(groups.iter().map(|g| g.joints.len()).sum());
    // Authored endpoints and child branches retain the export's existing behavior.
    for n in used.clone() {
        let mut stack = asset.nodes[n].children.clone();
        while let Some(child) = stack.pop() {
            if used.insert(child) {
                stack.extend(&asset.nodes[child].children);
            }
        }
    }
    // Do not place a generated ancestor above an authored chain.
    for n in used.clone() {
        let mut parent = asset.nodes[n].parent;
        while let Some(p) = parent {
            used.insert(p);
            parent = asset.nodes[p].parent;
        }
    }
    for &root in &asset.order {
        if groups.len() >= 256 || budget == 0 {
            break;
        }
        if !eligible.contains(&root) || used.contains(&root) {
            continue;
        }
        let mut ancestor = Some(root);
        let mut selected = false;
        let mut excluded = false;
        while let Some(n) = ancestor.filter(|n| eligible.contains(n)) {
            excluded |= settings.excluded_roots.contains(&n);
            selected |= settings.manual_roots.contains(&n)
                || (settings.auto_detect && named_secondary(&asset.nodes[n].name).is_some());
            ancestor = asset.nodes[n].parent;
        }
        if !selected || excluded {
            continue;
        }
        // A branching container anchors several strands. Start each child separately.
        if asset.nodes[root]
            .children
            .iter()
            .filter(|c| eligible.contains(c))
            .count()
            > 1
        {
            continue;
        }
        let mut joints = Vec::new();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if budget == 0
                || !eligible.contains(&n)
                || settings.excluded_roots.contains(&n)
                || !used.insert(n)
            {
                continue;
            }
            let children: Vec<_> = asset.nodes[n]
                .children
                .iter()
                .copied()
                .filter(|c| eligible.contains(c))
                .collect();
            // Retain unweighted end nodes; weighted leaves use a bounded mesh-derived tip.
            if let Some(tip) = inventory.tip(asset, n)
                && children.len() <= 1
            {
                let body =
                    named_secondary(&asset.nodes[root].name) == Some("Flexible body / breast bone");
                joints.push(Joint {
                    node: n,
                    tip,
                    stiffness: if body { 0.85 } else { 0.45 },
                    drag: if body { 0.3 } else { 0.22 },
                    gravity: -Vec3::Y,
                    power: if body { 0.006 } else { 0.015 },
                    radius: 0.,
                });
                budget -= 1;
            }
            stack.extend(children.into_iter().rev());
        }
        if !joints.is_empty() {
            groups.push(Group {
                id: format!("aria:spring:{root}"),
                name: asset.nodes[root].name.clone(),
                center: None,
                joints,
                colliders: vec![],
            });
        }
    }
    super::collision::attach(asset, inventory, settings, &mut groups);
    groups
}
