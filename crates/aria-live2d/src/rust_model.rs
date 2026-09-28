//! Renderer-facing adapter for ARIA's independent Rust MOC3 evaluator.

use crate::{Blend, Canvas, Drawable, Parameter};
use anyhow::{Result, ensure};
use aria_model_core::{draw_order::RenderOrderEvaluator, geometry::GeometryEvaluator, moc::Moc};
use std::collections::BTreeMap;

pub struct RustModel {
    evaluator: GeometryEvaluator,
    orderer: RenderOrderEvaluator,
    reverse_y: bool,
    pub(crate) parameters: Vec<Parameter>,
    parameter_lookup: BTreeMap<String, usize>,
    pub parts: Vec<Parameter>,
    pub canvas: Canvas,
    pub version: String,
    pub drawables: Vec<Drawable>,
}

impl RustModel {
    pub fn load(bytes: &[u8], texture_count: usize) -> Result<Self> {
        ensure!(
            bytes.len() <= aria_core::asset_limits::MOC_FILE,
            "MOC3 exceeds the file size limit"
        );
        ensure!((1..=32).contains(&texture_count), "Expected 1–32 textures");
        let moc = Moc::parse(bytes)?;
        ensure!(
            moc.offscreen_count()? == 0,
            "Cubism offscreen parts are not yet supported"
        );
        let canvas = moc.canvas()?;
        let layouts = moc.mesh_layouts()?;
        ensure!(
            layouts
                .iter()
                .all(|layout| usize::from(layout.texture) < texture_count),
            "A texture atlas is missing"
        );
        let part_layouts = moc.part_layouts()?;
        let parameters = moc
            .parameters()?
            .into_iter()
            .map(|spec| Parameter {
                id: spec.id,
                min: spec.minimum,
                max: spec.maximum,
                default: spec.default,
                value: spec.default,
            })
            .collect::<Vec<_>>();
        let parameter_lookup = parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.id.clone(), index))
            .collect();
        let parts = part_layouts
            .iter()
            .map(|part| {
                let value = if part.visible { 1.0 } else { 0.0 };
                Parameter {
                    id: part.id.clone(),
                    min: 0.0,
                    max: 1.0,
                    default: value,
                    value,
                }
            })
            .collect();
        let drawables = layouts
            .into_iter()
            .map(|layout| {
                let flags = layout.constant_flags;
                let masked = !layout.masks.is_empty();
                Drawable {
                    id: layout.id,
                    part: layout
                        .parent_part
                        .map_or_else(String::new, |index| part_layouts[index].id.clone()),
                    positions: vec![[0.0; 2]; layout.uvs.len()],
                    uvs: layout.uvs,
                    indices: layout.triangles,
                    texture: usize::from(layout.texture),
                    masks: layout
                        .masks
                        .into_iter()
                        .map(|index| index as usize)
                        .collect(),
                    masked,
                    inverted: flags & 8 != 0,
                    double_sided: flags & 4 != 0,
                    visible: false,
                    order: 0,
                    opacity: 0.0,
                    multiply: [1.0; 4],
                    screen: [0.0, 0.0, 0.0, 1.0],
                    blend: if flags & 1 != 0 {
                        Blend::Add
                    } else if flags & 2 != 0 {
                        Blend::Multiply
                    } else {
                        Blend::Normal
                    },
                }
            })
            .collect();
        let evaluator = GeometryEvaluator::new(&moc)?;
        let orderer = RenderOrderEvaluator::new(&moc)?;
        let mut model = Self {
            evaluator,
            orderer,
            reverse_y: canvas.reverse_y,
            parameters,
            parameter_lookup,
            parts,
            canvas: Canvas {
                size: canvas.size,
                origin: canvas.origin,
                pixels_per_unit: canvas.pixels_per_unit,
            },
            version: format!("ARIA Rust Model Core (MOC3 v{})", moc.version()),
            drawables,
        };
        model.update()?;
        Ok(model)
    }

    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    pub fn set_parameter(&mut self, id: &str, value: f32) {
        if value.is_finite()
            && let Some(&index) = self.parameter_lookup.get(id)
        {
            let parameter = &mut self.parameters[index];
            parameter.value = value.clamp(parameter.min, parameter.max);
        }
    }

    pub fn update(&mut self) -> Result<()> {
        let values = self
            .parameters
            .iter()
            .map(|parameter| parameter.value)
            .collect::<Vec<_>>();
        let parts = self.parts.iter().map(|part| part.value).collect::<Vec<_>>();
        let frames = self.evaluator.frame_with_parts(&values, &parts)?;
        let orders = self.orderer.frame(&values, &frames)?;
        for ((drawable, frame), order) in self.drawables.iter_mut().zip(frames).zip(orders) {
            drawable.order = order;
            if let Some(frame) = frame {
                ensure!(
                    drawable.positions.len() == frame.positions.len(),
                    "ArtMesh vertex count changed"
                );
                for (target, source) in drawable.positions.iter_mut().zip(frame.positions) {
                    *target = [
                        source[0],
                        if self.reverse_y {
                            source[1]
                        } else {
                            -source[1]
                        },
                    ];
                }
                drawable.opacity = frame.opacity.clamp(0.0, 1.0);
                drawable.visible = drawable.opacity != 0.0;
                drawable.multiply = frame.multiply;
                drawable.screen = frame.screen;
            } else {
                drawable.visible = false;
            }
        }
        ensure!(
            self.drawables.iter().all(|drawable| drawable
                .positions
                .iter()
                .flatten()
                .all(|v| v.is_finite())),
            "Non-finite ArtMesh vertex"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CubismModel;
    use std::path::Path;

    #[test]
    #[ignore = "requires ARIA_TEST_MOC; run optimized for meaningful timing"]
    fn local_runtime_update_benchmark() {
        let path = std::env::var_os("ARIA_TEST_MOC").unwrap();
        let bytes = std::fs::read(path).unwrap();
        let mut rust = RustModel::load(&bytes, 32).unwrap();
        let mut current = CubismModel::load(Path::new(""), &bytes, 32).unwrap();
        let parameter = current
            .parameters()
            .iter()
            .find(|p| p.max > p.min)
            .unwrap()
            .clone();
        for (name, model) in [
            ("current", &mut current as &mut dyn RuntimeBenchmark),
            ("Rust", &mut rust as &mut dyn RuntimeBenchmark),
        ] {
            let mut measured = std::time::Duration::ZERO;
            for frame in 0..140 {
                model.set(
                    &parameter.id,
                    if frame % 2 == 0 {
                        parameter.min
                    } else {
                        parameter.max
                    },
                );
                let start = std::time::Instant::now();
                model.evaluate().unwrap();
                if frame >= 20 {
                    measured += start.elapsed();
                }
            }
            eprintln!(
                "{name} direct model: {:.3} ms/frame",
                measured.as_secs_f64() * 1000.0 / 120.0
            );
        }
        let moc = Moc::parse(&bytes).unwrap();
        let evaluator = GeometryEvaluator::new(&moc).unwrap();
        let orderer = RenderOrderEvaluator::new(&moc).unwrap();
        let specs = moc.parameters().unwrap();
        let index = specs.iter().position(|p| p.id == parameter.id).unwrap();
        let mut values = specs.iter().map(|p| p.default).collect::<Vec<_>>();
        let parts = moc
            .part_layouts()
            .unwrap()
            .iter()
            .map(|part| if part.visible { 1.0 } else { 0.0 })
            .collect::<Vec<_>>();
        let (mut geometry_time, mut order_time) =
            (std::time::Duration::ZERO, std::time::Duration::ZERO);
        for frame in 0..140 {
            values[index] = if frame % 2 == 0 {
                parameter.min
            } else {
                parameter.max
            };
            let start = std::time::Instant::now();
            let meshes = evaluator.frame_with_parts(&values, &parts).unwrap();
            let after_geometry = std::time::Instant::now();
            orderer.frame(&values, &meshes).unwrap();
            if frame >= 20 {
                geometry_time += after_geometry - start;
                order_time += after_geometry.elapsed();
            }
        }
        eprintln!(
            "Rust geometry: {:.3} ms/frame; render order: {:.3} ms/frame",
            geometry_time.as_secs_f64() * 1000.0 / 120.0,
            order_time.as_secs_f64() * 1000.0 / 120.0
        );
    }

    trait RuntimeBenchmark {
        fn set(&mut self, id: &str, value: f32);
        fn evaluate(&mut self) -> Result<()>;
    }
    impl RuntimeBenchmark for RustModel {
        fn set(&mut self, id: &str, value: f32) {
            self.set_parameter(id, value);
        }
        fn evaluate(&mut self) -> Result<()> {
            self.update()
        }
    }
    impl RuntimeBenchmark for CubismModel {
        fn set(&mut self, id: &str, value: f32) {
            self.set_parameter(id, value);
        }
        fn evaluate(&mut self) -> Result<()> {
            self.update()
        }
    }
}
