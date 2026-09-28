//! Hierarchical ArtMesh render ordering from MOC3 draw groups.

use crate::moc::{
    BindingGraph, DrawGroupLayout, DrawItem, LocalMeshFrame, Moc, ParameterSpec, PartLayout,
};
use anyhow::{Context, Result, ensure};

pub struct RenderOrderEvaluator {
    parameters: Vec<ParameterSpec>,
    graph: BindingGraph,
    parts: Vec<PartLayout>,
    part_keys: Vec<Vec<f32>>,
    groups: Vec<DrawGroupLayout>,
    traversal: Vec<usize>,
    mesh_count: usize,
}

impl RenderOrderEvaluator {
    pub fn new(moc: &Moc<'_>) -> Result<Self> {
        let parameters = moc.parameters()?;
        let graph = moc.binding_graph()?;
        let parts = moc.part_layouts()?;
        let groups = moc.draw_group_layouts()?;
        let part_keys = (0..parts.len())
            .map(|index| moc.part_draw_order_keyforms(index))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            parts.iter().all(|part| part.binding < graph.bindings.len()),
            "Part references an invalid draw-order binding"
        );
        for (part, keys) in parts.iter().zip(&part_keys) {
            let axes = &graph.bindings[part.binding];
            let expected = axes.iter().try_fold(1_usize, |count, &axis| {
                count.checked_mul(graph.tables[axis].keys.len())
            });
            ensure!(
                expected == Some(keys.len()),
                "Part draw-order keyform count differs from binding"
            );
        }
        let mesh_count = moc.counts()?.art_meshes as usize;
        let traversal = validate_groups(&groups, mesh_count)?;
        Ok(Self {
            parameters,
            graph,
            parts,
            part_keys,
            groups,
            traversal,
            mesh_count,
        })
    }

    pub fn frame(&self, values: &[f32], meshes: &[Option<LocalMeshFrame>]) -> Result<Vec<i32>> {
        ensure!(
            values.len() == self.parameters.len(),
            "Parameter count differs from model"
        );
        ensure!(
            meshes.len() == self.mesh_count,
            "ArtMesh count differs from model"
        );
        let values = self
            .parameters
            .iter()
            .zip(values)
            .map(|(spec, value)| spec.resolve(*value))
            .collect::<Result<Vec<_>>>()?;
        let mut active = Vec::with_capacity(self.parts.len());
        let mut part_order = Vec::with_capacity(self.parts.len());
        for (part, keys) in self.parts.iter().zip(&self.part_keys) {
            let parent_active = part.parent.is_none_or(|index| active[index]);
            let enabled =
                part.enabled && parent_active && self.graph.in_range(part.binding, &values)?;
            active.push(enabled);
            let mut order = 0.0_f32;
            if enabled {
                for weight in self.graph.weights(part.binding, &values)? {
                    order += keys
                        .get(weight.index)
                        .context("Part draw-order key is missing")?
                        * weight.weight;
                }
            }
            part_order.push(integer_order(order));
        }
        let mut ranks = (0..self.mesh_count)
            .map(|index| index as i32)
            .collect::<Vec<_>>();
        let mut cursors = vec![0_usize; self.groups.len()];
        for &group_index in &self.traversal {
            let group = &self.groups[group_index];
            let mut sorted = group
                .items
                .iter()
                .enumerate()
                .map(|(source_index, item)| {
                    let order = match item {
                        DrawItem::Mesh(index) => meshes[*index]
                            .as_ref()
                            .map(|frame| frame.integer_draw_order()),
                        DrawItem::Part { index, .. } => {
                            active[*index].then_some(part_order[*index])
                        }
                    }
                    .unwrap_or(group.min_order)
                    .clamp(group.min_order, group.max_order);
                    (order, source_index, item)
                })
                .collect::<Vec<_>>();
            sorted.sort_by_key(|(order, source_index, _)| (*order, *source_index));
            let mut position = cursors[group_index];
            for (_, _, item) in sorted {
                match *item {
                    DrawItem::Mesh(index) => {
                        ranks[index] =
                            i32::try_from(position).context("Render order exceeds i32")?;
                        position = position.checked_add(1).context("Render order overflow")?;
                    }
                    DrawItem::Part { child_group, .. } => {
                        cursors[child_group] = position;
                        position = position
                            .checked_add(self.groups[child_group].total_count)
                            .context("Render order overflow")?;
                    }
                }
            }
        }
        Ok(ranks)
    }
}

fn validate_groups(groups: &[DrawGroupLayout], mesh_count: usize) -> Result<Vec<usize>> {
    let mut mesh_owner = vec![None; mesh_count];
    let mut group_parent = vec![None; groups.len()];
    for (parent, group) in groups.iter().enumerate() {
        for item in &group.items {
            match *item {
                DrawItem::Mesh(index) => {
                    ensure!(
                        index < mesh_count,
                        "Draw group references an invalid ArtMesh"
                    );
                    ensure!(
                        mesh_owner[index].replace(parent).is_none(),
                        "ArtMesh belongs to multiple draw groups"
                    );
                }
                DrawItem::Part { child_group, .. } => {
                    ensure!(child_group < groups.len(), "Invalid child draw group");
                    ensure!(
                        group_parent[child_group].replace(parent).is_none(),
                        "Draw group has multiple parents"
                    );
                }
            }
        }
    }
    let mut traversal = Vec::with_capacity(groups.len());
    let mut queue = std::collections::VecDeque::new();
    queue.extend((0..groups.len()).filter(|&index| group_parent[index].is_none()));
    while let Some(index) = queue.pop_front() {
        traversal.push(index);
        for item in &groups[index].items {
            if let DrawItem::Part { child_group, .. } = item {
                queue.push_back(*child_group);
            }
        }
    }
    ensure!(
        traversal.len() == groups.len(),
        "Draw-group graph has a cycle"
    );
    let mut descendants = vec![0_usize; groups.len()];
    for &index in traversal.iter().rev() {
        let group = &groups[index];
        descendants[index] = group.items.iter().try_fold(0_usize, |sum, item| {
            let count = match *item {
                DrawItem::Mesh(_) => 1,
                DrawItem::Part { child_group, .. } => descendants[child_group],
            };
            sum.checked_add(count).context("Draw-group count overflow")
        })?;
        ensure!(
            descendants[index] == group.total_count,
            "Draw-group descendant count differs from its declared size"
        );
    }
    Ok(traversal)
}

fn integer_order(value: f32) -> i32 {
    let rounded = value.round();
    let tolerance = 2.0 * f32::EPSILON * value.abs().max(1.0);
    if (value - rounded).abs() <= tolerance {
        rounded as i32
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(order: f32) -> Option<LocalMeshFrame> {
        Some(LocalMeshFrame {
            positions: Vec::new(),
            opacity: 1.0,
            draw_order: order,
            multiply: [1.0; 4],
            screen: [0.0; 4],
            parent_deformer: None,
        })
    }

    #[test]
    fn child_group_reserves_its_mesh_slots_at_the_parent_order() {
        let evaluator = RenderOrderEvaluator {
            parameters: Vec::new(),
            graph: BindingGraph {
                tables: Vec::new(),
                bindings: vec![Vec::new()],
            },
            parts: vec![PartLayout {
                id: "Part".into(),
                binding: 0,
                parent: None,
                visible: true,
                enabled: true,
            }],
            part_keys: vec![vec![5.0]],
            groups: vec![
                DrawGroupLayout {
                    min_order: 0,
                    max_order: 10,
                    total_count: 3,
                    items: vec![
                        DrawItem::Mesh(0),
                        DrawItem::Part {
                            index: 0,
                            child_group: 1,
                        },
                    ],
                },
                DrawGroupLayout {
                    min_order: 0,
                    max_order: 10,
                    total_count: 2,
                    items: vec![DrawItem::Mesh(1), DrawItem::Mesh(2)],
                },
            ],
            traversal: vec![0, 1],
            mesh_count: 3,
        };
        assert_eq!(
            evaluator
                .frame(&[], &[mesh(10.0), mesh(3.0), mesh(1.0)])
                .unwrap(),
            [2, 1, 0]
        );
        assert_eq!(
            evaluator
                .frame(&[], &[mesh(0.0), mesh(3.0), mesh(1.0)])
                .unwrap(),
            [0, 2, 1]
        );
    }

    #[test]
    fn malformed_draw_group_graph_is_rejected_before_frame_evaluation() {
        let mut groups = vec![
            DrawGroupLayout {
                min_order: 0,
                max_order: 1,
                total_count: 1,
                items: vec![DrawItem::Part {
                    index: 0,
                    child_group: 1,
                }],
            },
            DrawGroupLayout {
                min_order: 0,
                max_order: 1,
                total_count: 1,
                items: vec![DrawItem::Mesh(0)],
            },
        ];
        assert_eq!(validate_groups(&groups, 1).unwrap(), [0, 1]);
        groups[1].items.push(DrawItem::Part {
            index: 0,
            child_group: 0,
        });
        assert!(validate_groups(&groups, 1).is_err());
        groups[1].items.pop();
        groups[0].items.push(DrawItem::Mesh(0));
        assert!(validate_groups(&groups, 1).is_err());
        groups[0].items.pop();
        groups[0].total_count = 2;
        assert!(validate_groups(&groups, 1).is_err());
        groups[0].total_count = 1;
        groups.swap(0, 1);
        if let DrawItem::Part { child_group, .. } = &mut groups[1].items[0] {
            *child_group = 0;
        }
        assert_eq!(validate_groups(&groups, 1).unwrap(), [1, 0]);
    }
}
