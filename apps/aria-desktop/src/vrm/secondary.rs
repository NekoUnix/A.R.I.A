//! Conservative secondary chains for exports without complete spring metadata.
use super::{
    asset::Asset,
    spring::{Group, Joint},
};
use aria_core::vrm::Secondary;
use glam::Vec3;
use std::collections::BTreeSet;

fn named_secondary(name: &str) -> bool {
    let name = name.to_lowercase();
    [
        "hair",
        "tail",
        "ponytail",
        "ribbon",
        "skirt",
        "sleeve",
        "cape",
        "ear",
        "bow",
        "accessory",
        "髪",
        "尻尾",
        "スカート",
        "リボン",
    ]
    .iter()
    .any(|word| name.contains(word))
        && !["eyebrow", "forearm", "upperarm", "lowerarm", "earring"]
            .iter()
            .any(|word| name.contains(word))
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

pub fn groups(asset: &Asset, settings: &Secondary) -> Vec<Group> {
    let mut groups = asset.springs.clone();
    let eligible = candidates(asset);
    let mut used: BTreeSet<_> = groups
        .iter()
        .flat_map(|g| g.joints.iter().map(|j| j.node))
        .collect();
    let mut budget = 4096usize.saturating_sub(groups.iter().map(|g| g.joints.len()).sum());
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
        while let Some(n) = ancestor.filter(|n| eligible.contains(n)) {
            selected |= settings.manual_roots.contains(&n)
                || (settings.auto_detect && named_secondary(&asset.nodes[n].name));
            ancestor = asset.nodes[n].parent;
        }
        if !selected {
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
            if budget == 0 || !eligible.contains(&n) || !used.insert(n) {
                continue;
            }
            let children: Vec<_> = asset.nodes[n]
                .children
                .iter()
                .copied()
                .filter(|c| eligible.contains(c))
                .collect();
            // Terminals already move with their simulated parent. No invented end length.
            if let Some(&child) = children.first() {
                let tip = asset.nodes[child].translation;
                if tip.length_squared() > 1e-10 && children.len() == 1 {
                    joints.push(Joint {
                        node: n,
                        tip,
                        stiffness: 0.45,
                        drag: 0.22,
                        gravity: -Vec3::Y,
                        power: 0.015,
                        radius: 0.,
                    });
                    budget -= 1;
                }
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
    groups
}
