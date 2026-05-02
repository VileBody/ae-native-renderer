//! Collapse transformations planning support.
//!
//! This module only answers whether a precomp boundary can be flattened as
//! vector/text child layers. Pixel render integration is intentionally separate:
//! the render loop can use these plans later to compose the precomp parent matrix
//! with each returned child layer and nested collapsed precomp boundary.

use render_ir::{Layer, Transform2D};
use std::collections::BTreeSet;

pub const ISSUE_TARGET_MISSING: &str = "target composition is missing";
pub const ISSUE_EFFECTS_REQUIRE_RASTER: &str =
    "layer effects require offscreen rasterization before collapse";
pub const ISSUE_FOOTAGE_REQUIRES_RASTER: &str =
    "footage layers require raster source sampling before collapse";
pub const ISSUE_ADJUSTMENT_REQUIRES_RASTER: &str =
    "adjustment layers depend on the offscreen composite before collapse";
pub const ISSUE_NESTED_PRECOMP_RASTERIZES: &str =
    "nested precomp rasterizes before the requested collapse boundary";
pub const ISSUE_NESTED_PRECOMP_CYCLE: &str = "nested collapsed precomp cycle prevents collapse";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseMode {
    RasterizeFirst,
    CollapseSupportedVectors,
}

#[derive(Debug, Clone)]
pub struct PrecompCollapsePlan<'a> {
    pub parent_composition: &'a str,
    pub layer_id: &'a str,
    pub target_composition: &'a str,
    pub collapse_requested: bool,
    pub mode: CollapseMode,
    pub issues: Vec<&'static str>,
    pub flattened_layers: Vec<CollapsedLayerRef<'a>>,
}

impl PrecompCollapsePlan<'_> {
    pub fn can_flatten_with_parent_matrix(&self) -> bool {
        self.collapse_requested
            && self.mode == CollapseMode::CollapseSupportedVectors
            && self.issues.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct CollapsedLayerRef<'a> {
    pub composition_id: &'a str,
    pub layer: &'a Layer,
    pub nested_precomp_path: Vec<CollapsedPrecompBoundary<'a>>,
}

#[derive(Debug, Clone)]
pub struct CollapsedPrecompBoundary<'a> {
    pub parent_composition: &'a str,
    pub layer_id: &'a str,
    pub target_composition: &'a str,
    pub transform: &'a Transform2D,
}

pub fn plan_precomp_collapse<'a>(
    parent_composition: &'a str,
    precomp_layer: &'a Layer,
    layers_for: impl Fn(&str) -> Option<&'a [Layer]>,
) -> anyhow::Result<PrecompCollapsePlan<'a>> {
    let Layer::Precomp {
        id,
        composition,
        collapse_transformations,
        ..
    } = precomp_layer
    else {
        anyhow::bail!(
            "layer '{}' in composition '{}' is not a precomp",
            precomp_layer.id(),
            parent_composition
        );
    };

    let target = composition.as_str();
    if !*collapse_transformations {
        return Ok(PrecompCollapsePlan {
            parent_composition,
            layer_id: id,
            target_composition: target,
            collapse_requested: false,
            mode: CollapseMode::RasterizeFirst,
            issues: Vec::new(),
            flattened_layers: Vec::new(),
        });
    }

    let target_plan = plan_target_collapse(target, layers_for);
    let mode = if target_plan.issues.is_empty() {
        CollapseMode::CollapseSupportedVectors
    } else {
        CollapseMode::RasterizeFirst
    };

    Ok(PrecompCollapsePlan {
        parent_composition,
        layer_id: id,
        target_composition: target,
        collapse_requested: true,
        mode,
        issues: target_plan.issues,
        flattened_layers: target_plan.flattened_layers,
    })
}

#[derive(Debug, Clone)]
pub struct TargetCollapsePlan<'a> {
    pub target_composition: &'a str,
    pub issues: Vec<&'static str>,
    pub flattened_layers: Vec<CollapsedLayerRef<'a>>,
}

pub fn plan_target_collapse<'a>(
    target_composition: &'a str,
    layers_for: impl Fn(&str) -> Option<&'a [Layer]>,
) -> TargetCollapsePlan<'a> {
    let mut planner = CollapsePlanner {
        layers_for: &layers_for,
        issues: Vec::new(),
        flattened_layers: Vec::new(),
        visiting: BTreeSet::new(),
        path: Vec::new(),
    };
    planner.visit_target(target_composition);
    TargetCollapsePlan {
        target_composition,
        issues: planner.issues,
        flattened_layers: planner.flattened_layers,
    }
}

struct CollapsePlanner<'a, 'f, F>
where
    F: Fn(&str) -> Option<&'a [Layer]>,
{
    layers_for: &'f F,
    issues: Vec<&'static str>,
    flattened_layers: Vec<CollapsedLayerRef<'a>>,
    visiting: BTreeSet<&'a str>,
    path: Vec<CollapsedPrecompBoundary<'a>>,
}

impl<'a, 'f, F> CollapsePlanner<'a, 'f, F>
where
    F: Fn(&str) -> Option<&'a [Layer]>,
{
    fn visit_target(&mut self, composition_id: &'a str) {
        if !self.visiting.insert(composition_id) {
            push_unique(&mut self.issues, ISSUE_NESTED_PRECOMP_CYCLE);
            return;
        }

        let Some(layers) = (self.layers_for)(composition_id) else {
            push_unique(&mut self.issues, ISSUE_TARGET_MISSING);
            self.visiting.remove(composition_id);
            return;
        };

        for layer in layers {
            if layer_has_effects(layer) {
                push_unique(&mut self.issues, ISSUE_EFFECTS_REQUIRE_RASTER);
            }

            match layer {
                Layer::Solid { .. } | Layer::Text { .. } => {
                    if !layer_has_effects(layer) {
                        self.flattened_layers.push(CollapsedLayerRef {
                            composition_id,
                            layer,
                            nested_precomp_path: self.path.clone(),
                        });
                    }
                }
                Layer::Footage { .. } => {
                    push_unique(&mut self.issues, ISSUE_FOOTAGE_REQUIRES_RASTER);
                }
                Layer::Adjustment { .. } => {
                    push_unique(&mut self.issues, ISSUE_ADJUSTMENT_REQUIRES_RASTER);
                }
                Layer::Precomp {
                    id,
                    composition,
                    collapse_transformations,
                    transform,
                    ..
                } => {
                    if !*collapse_transformations {
                        push_unique(&mut self.issues, ISSUE_NESTED_PRECOMP_RASTERIZES);
                        continue;
                    }
                    if layer_has_effects(layer) {
                        continue;
                    }

                    let target = composition.as_str();
                    self.path.push(CollapsedPrecompBoundary {
                        parent_composition: composition_id,
                        layer_id: id,
                        target_composition: target,
                        transform,
                    });
                    self.visit_target(target);
                    self.path.pop();
                }
            }
        }

        self.visiting.remove(composition_id);
    }
}

pub fn layer_has_effects(layer: &Layer) -> bool {
    match layer {
        Layer::Solid { effects, .. }
        | Layer::Footage { effects, .. }
        | Layer::Text { effects, .. }
        | Layer::Precomp { effects, .. }
        | Layer::Adjustment { effects, .. } => !effects.is_empty(),
    }
}

pub fn push_unique(values: &mut Vec<&'static str>, value: &'static str) {
    if !values.contains(&value) {
        values.push(value);
    }
}
