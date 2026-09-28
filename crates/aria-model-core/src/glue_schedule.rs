//! Conflict-free glue passes for a GPU geometry evaluator.

use crate::moc::GlueLayout;
use std::collections::{BTreeMap, BTreeSet};

/// Preserve source glue order for shared vertices while running independent
/// glues in parallel. Returns `None` when a single glue reuses a vertex; its
/// pairs need a finer schedule or the sequential CPU fallback.
pub fn gpu_glue_levels(glues: &[GlueLayout]) -> Option<Vec<Vec<usize>>> {
    let mut levels = Vec::<Vec<usize>>::new();
    let mut last_level = BTreeMap::<(usize, usize), usize>::new();
    for (index, glue) in glues.iter().enumerate() {
        if glue.pairs.is_empty() {
            continue;
        }
        let mut touched = BTreeSet::new();
        for pair in &glue.pairs {
            if !touched.insert((glue.left_mesh, pair.left))
                || !touched.insert((glue.right_mesh, pair.right))
            {
                return None;
            }
        }
        let level = touched
            .iter()
            .filter_map(|vertex| last_level.get(vertex).map(|last| last + 1))
            .max()
            .unwrap_or(0);
        if levels.len() <= level {
            levels.resize_with(level + 1, Vec::new);
        }
        levels[level].push(index);
        for vertex in touched {
            last_level.insert(vertex, level);
        }
    }
    Some(levels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moc::GlueVertexPair;

    fn glue(left_mesh: usize, right_mesh: usize, pairs: &[(usize, usize)]) -> GlueLayout {
        GlueLayout {
            left_mesh,
            right_mesh,
            binding: 0,
            intensities: vec![1.0],
            pairs: pairs
                .iter()
                .map(|&(left, right)| GlueVertexPair {
                    left,
                    right,
                    left_weight: 0.5,
                    right_weight: 0.5,
                })
                .collect(),
        }
    }

    #[test]
    fn independent_pairs_share_a_pass_and_conflicts_keep_source_order() {
        let glues = [
            glue(0, 1, &[(0, 0)]),
            glue(2, 3, &[(0, 0)]),
            glue(1, 4, &[(0, 0)]),
            glue(4, 5, &[(0, 0)]),
            glue(6, 7, &[(0, 0)]),
        ];
        assert_eq!(
            gpu_glue_levels(&glues),
            Some(vec![vec![0, 1, 4], vec![2], vec![3]])
        );
    }

    #[test]
    fn repeated_vertex_within_one_glue_needs_fallback() {
        assert_eq!(gpu_glue_levels(&[glue(0, 1, &[(0, 0), (0, 1)])]), None);
        assert_eq!(gpu_glue_levels(&[glue(0, 1, &[])]), Some(vec![]));
    }
}
