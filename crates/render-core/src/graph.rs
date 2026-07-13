use crate::collapse::PrecompCollapsePlan;
use crate::precomp::{PrecompDeferredRasterPlan, PrecompGraph, PrecompRenderPlan};
use render_ir::Scene;

pub fn validate_graph_stub(scene: &Scene) -> anyhow::Result<()> {
    validate_graph(scene)
}

pub fn validate_graph(scene: &Scene) -> anyhow::Result<()> {
    PrecompGraph::from_scene(scene)?.validate()
}

pub fn precomp_render_plan(scene: &Scene) -> anyhow::Result<PrecompRenderPlan<'_>> {
    PrecompGraph::from_scene(scene)?.render_plan()
}

pub fn precomp_collapse_plan<'a>(
    scene: &'a Scene,
    parent_composition: &str,
    layer_id: &str,
) -> anyhow::Result<PrecompCollapsePlan<'a>> {
    PrecompGraph::from_scene(scene)?.collapse_plan_for_precomp_layer(parent_composition, layer_id)
}

pub fn precomp_deferred_raster_plan<'a>(
    scene: &'a Scene,
    parent_composition: &str,
    layer_id: &str,
) -> anyhow::Result<PrecompDeferredRasterPlan<'a>> {
    PrecompGraph::from_scene(scene)?
        .deferred_raster_plan_for_precomp_layer(parent_composition, layer_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_ir::{Composition, Layer, Rect, Transform2D};

    #[test]
    fn accepts_scene_without_precomps() {
        let scene = scene_with_layers(vec![solid_layer("solid")]);

        validate_graph_stub(&scene).unwrap();
        let plan = precomp_render_plan(&scene).unwrap();
        assert!(plan.entries.is_empty());
    }

    #[test]
    fn reports_missing_precomp_composition_in_current_ir_model() {
        let scene = scene_with_layers(vec![precomp_layer("missing_pre", "child")]);

        let error = validate_graph(&scene).unwrap_err().to_string();
        assert!(
            error.contains("references missing composition 'child'"),
            "{error}"
        );
    }

    #[test]
    fn reports_self_cycle_when_precomp_targets_root_composition() {
        let scene = scene_with_layers(vec![precomp_layer("self_pre", "root")]);

        let error = validate_graph(&scene).unwrap_err().to_string();
        assert!(
            error.contains("precomp composition cycle detected: root -> root"),
            "{error}"
        );
    }

    fn scene_with_layers(layers: Vec<Layer>) -> Scene {
        Scene {
            version: "test".to_string(),
            composition: Composition {
                id: "root".to_string(),
                width: 64,
                height: 64,
                fps: 24.0,
                duration: 1.0,
                background: [0, 0, 0, 0],
                motion_blur: render_ir::MotionBlurSettings::default(),
            },
            compositions: Vec::new(),
            assets: Vec::new(),
            layers,
        }
    }

    fn precomp_layer(id: &str, composition: &str) -> Layer {
        Layer::Precomp {
            id: id.to_string(),
            start: 0.0,
            duration: 1.0,
            composition: composition.to_string(),
            collapse_transformations: false,
            blend_mode: render_ir::BlendMode::Normal,
            transform: Transform2D::default(),
            effects: Vec::new(),
        }
    }

    fn solid_layer(id: &str) -> Layer {
        Layer::Solid {
            id: id.to_string(),
            start: 0.0,
            duration: 1.0,
            blend_mode: render_ir::BlendMode::Normal,
            color: [255, 255, 255, 255],
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 16.0,
                h: 16.0,
            },
            transform: Transform2D::default(),
            effects: Vec::new(),
        }
    }
}
