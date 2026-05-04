//! Precomp evaluation skeleton.
//!
//! v0: render nested composition into offscreen canvas, then composite as a normal layer.
//! v1: cache precomp frames.
//! v2: implement controlled collapse transformations.

pub use crate::collapse::CollapseMode;
use crate::collapse::{
    plan_precomp_collapse, plan_target_collapse, PrecompCollapsePlan, ISSUE_TARGET_MISSING,
};
use render_ir::{Layer, Scene, Transform2D};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
pub struct CompositionNode<'a> {
    pub id: &'a str,
    pub layers: &'a [Layer],
}

#[derive(Debug, Clone)]
pub struct PrecompGraph<'a> {
    root_id: &'a str,
    compositions: BTreeMap<&'a str, &'a [Layer]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecompPlanEntry<'a> {
    pub parent_composition: &'a str,
    pub layer_id: &'a str,
    pub target_composition: &'a str,
    pub collapse_requested: bool,
    pub collapse_mode: CollapseMode,
    pub collapse_issues: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecompRenderPlan<'a> {
    pub root_composition: &'a str,
    pub entries: Vec<PrecompPlanEntry<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollapseIssue<'a> {
    pub parent_composition: &'a str,
    pub layer_id: &'a str,
    pub target_composition: &'a str,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct PrecompDeferredRasterPlan<'a> {
    pub parent_composition: &'a str,
    pub layer_id: &'a str,
    pub target_composition: &'a str,
    pub collapse_requested: bool,
    pub mode: CollapseMode,
    pub deferred_primitives: Vec<DeferredRasterPrimitive<'a>>,
    pub raster_barriers: Vec<DeferredRasterBarrier<'a>>,
}

impl PrecompDeferredRasterPlan<'_> {
    pub fn can_defer_all_primitives(&self) -> bool {
        self.collapse_requested
            && self.mode == CollapseMode::CollapseSupportedVectors
            && !self.deferred_primitives.is_empty()
            && self.raster_barriers.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct DeferredRasterPrimitive<'a> {
    pub kind: DeferredPrimitiveKind,
    pub source_composition: &'a str,
    pub source_layer_id: &'a str,
    pub source_layer: &'a Layer,
    pub transform_steps: Vec<DeferredTransformStep<'a>>,
    pub source_time_steps: Vec<DeferredSourceTimeStep<'a>>,
    pub active_window: DeferredActiveWindow,
    pub opacity: DeferredOpacityContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredPrimitiveKind {
    SolidVector,
    TextVector,
}

#[derive(Debug, Clone)]
pub struct DeferredTransformStep<'a> {
    pub composition_id: &'a str,
    pub layer_id: &'a str,
    pub kind: DeferredTransformStepKind,
    pub transform: &'a Transform2D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredTransformStepKind {
    RootPrecompBoundary,
    NestedPrecompBoundary,
    SourceLayer,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeferredSourceTimeStep<'a> {
    pub composition_id: &'a str,
    pub layer_id: &'a str,
    pub start: f64,
    pub duration: f64,
    pub rule: DeferredSourceTimeRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredSourceTimeRule {
    LayerRelativeClampedToZero,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeferredActiveWindow {
    pub start: f64,
    pub duration: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeferredOpacityContract {
    pub base_opacity_percent: f32,
    pub rule: DeferredOpacityRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredOpacityRule {
    EvaluateSourceTransformAtFinalSourceTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredRasterBarrier<'a> {
    pub composition_id: &'a str,
    pub layer_id: &'a str,
    pub reason: &'static str,
}

impl<'a> PrecompGraph<'a> {
    pub fn from_scene(scene: &'a Scene) -> anyhow::Result<Self> {
        let root = CompositionNode {
            id: scene.composition.id.as_str(),
            layers: scene.layers.as_slice(),
        };
        let nested = scene.compositions.iter().map(|node| CompositionNode {
            id: node.composition.id.as_str(),
            layers: node.layers.as_slice(),
        });
        Self::from_nodes(
            scene.composition.id.as_str(),
            std::iter::once(root).chain(nested),
        )
    }

    pub fn from_nodes(
        root_id: &'a str,
        nodes: impl IntoIterator<Item = CompositionNode<'a>>,
    ) -> anyhow::Result<Self> {
        let mut compositions = BTreeMap::new();
        for node in nodes {
            if compositions.insert(node.id, node.layers).is_some() {
                anyhow::bail!("duplicate composition id '{}'", node.id);
            }
        }
        if !compositions.contains_key(root_id) {
            anyhow::bail!("root composition '{}' is missing", root_id);
        }
        Ok(Self {
            root_id,
            compositions,
        })
    }

    pub fn root_id(&self) -> &'a str {
        self.root_id
    }

    pub fn layers_for(&self, composition_id: &str) -> Option<&'a [Layer]> {
        self.compositions.get(composition_id).copied()
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        self.validate_references()?;
        self.validate_acyclic()
    }

    pub fn validate_references(&self) -> anyhow::Result<()> {
        for (composition_id, layers) in &self.compositions {
            for layer in *layers {
                let Layer::Precomp {
                    id, composition, ..
                } = layer
                else {
                    continue;
                };
                if !self.compositions.contains_key(composition.as_str()) {
                    anyhow::bail!(
                        "precomp layer '{}' in composition '{}' references missing composition '{}'",
                        id,
                        composition_id,
                        composition
                    );
                }
            }
        }
        Ok(())
    }

    pub fn validate_acyclic(&self) -> anyhow::Result<()> {
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();
        self.visit_for_cycles(self.root_id, &mut visiting, &mut visited, &mut stack)
    }

    pub fn render_plan(&self) -> anyhow::Result<PrecompRenderPlan<'a>> {
        self.validate()?;
        let mut entries = Vec::new();
        let mut stack = Vec::new();
        self.collect_plan_entries(self.root_id, &mut stack, &mut entries)?;
        Ok(PrecompRenderPlan {
            root_composition: self.root_id,
            entries,
        })
    }

    pub fn collapse_issues(&self) -> Vec<CollapseIssue<'a>> {
        let mut issues = Vec::new();
        for (parent_composition, layers) in &self.compositions {
            for layer in *layers {
                let Layer::Precomp {
                    id,
                    composition,
                    collapse_transformations,
                    ..
                } = layer
                else {
                    continue;
                };
                if !*collapse_transformations {
                    continue;
                }
                for reason in self.collapse_issues_for_target(composition) {
                    issues.push(CollapseIssue {
                        parent_composition,
                        layer_id: id,
                        target_composition: composition,
                        reason,
                    });
                }
            }
        }
        issues
    }

    pub fn collapse_plan_for_precomp_layer(
        &self,
        parent_composition: &str,
        layer_id: &str,
    ) -> anyhow::Result<PrecompCollapsePlan<'a>> {
        let (parent_composition, layers) = self
            .compositions
            .iter()
            .find(|(id, _)| **id == parent_composition)
            .map(|(id, layers)| (*id, *layers))
            .ok_or_else(|| anyhow::anyhow!("composition '{}' is missing", parent_composition))?;

        let layer = layers
            .iter()
            .find(|layer| layer.id() == layer_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "layer '{}' in composition '{}' is missing",
                    layer_id,
                    parent_composition
                )
            })?;

        plan_precomp_collapse(parent_composition, layer, |composition_id| {
            self.layers_for(composition_id)
        })
    }

    pub fn deferred_raster_plan_for_precomp_layer(
        &self,
        parent_composition: &str,
        layer_id: &str,
    ) -> anyhow::Result<PrecompDeferredRasterPlan<'a>> {
        let (parent_composition, layer) = self
            .find_layer(parent_composition, layer_id)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "layer '{}' in composition '{}' is missing",
                    layer_id,
                    parent_composition
                )
            })?;

        let collapse_plan = plan_precomp_collapse(parent_composition, layer, |composition_id| {
            self.layers_for(composition_id)
        })?;

        let Layer::Precomp {
            id,
            composition,
            collapse_transformations,
            transform,
            ..
        } = layer
        else {
            anyhow::bail!(
                "layer '{}' in composition '{}' is not a precomp",
                layer.id(),
                parent_composition
            );
        };

        let target = composition.as_str();
        let raster_barriers = if *collapse_transformations {
            self.deferred_raster_barriers_for_precomp(parent_composition, id, target)
        } else {
            Vec::new()
        };
        let mode = if *collapse_transformations && raster_barriers.is_empty() {
            CollapseMode::CollapseSupportedVectors
        } else {
            CollapseMode::RasterizeFirst
        };

        let deferred_primitives = if *collapse_transformations {
            collapse_plan
                .flattened_layers
                .iter()
                .filter_map(|layer_ref| {
                    self.deferred_primitive_for_layer_ref(
                        parent_composition,
                        id,
                        transform,
                        layer_ref,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(PrecompDeferredRasterPlan {
            parent_composition,
            layer_id: id,
            target_composition: target,
            collapse_requested: *collapse_transformations,
            mode,
            deferred_primitives,
            raster_barriers,
        })
    }

    pub fn validate_collapse_feasibility(&self) -> anyhow::Result<()> {
        let issues = self.collapse_issues();
        if issues.is_empty() {
            return Ok(());
        }
        let first = &issues[0];
        anyhow::bail!(
            "precomp layer '{}' in composition '{}' cannot collapse '{}': {}",
            first.layer_id,
            first.parent_composition,
            first.target_composition,
            first.reason
        );
    }

    fn visit_for_cycles(
        &self,
        composition_id: &'a str,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
        stack: &mut Vec<&'a str>,
    ) -> anyhow::Result<()> {
        if visited.contains(composition_id) {
            return Ok(());
        }
        if visiting.contains(composition_id) {
            return Err(cycle_error(stack, composition_id));
        }

        visiting.insert(composition_id);
        stack.push(composition_id);

        if let Some(layers) = self.layers_for(composition_id) {
            for layer in layers {
                let Layer::Precomp { composition, .. } = layer else {
                    continue;
                };
                let target = composition.as_str();
                if self.compositions.contains_key(target) {
                    self.visit_for_cycles(target, visiting, visited, stack)?;
                }
            }
        }

        stack.pop();
        visiting.remove(composition_id);
        visited.insert(composition_id);
        Ok(())
    }

    fn collect_plan_entries(
        &self,
        composition_id: &'a str,
        stack: &mut Vec<&'a str>,
        entries: &mut Vec<PrecompPlanEntry<'a>>,
    ) -> anyhow::Result<()> {
        if stack.contains(&composition_id) {
            return Err(cycle_error(stack, composition_id));
        }
        stack.push(composition_id);

        let Some(layers) = self.layers_for(composition_id) else {
            anyhow::bail!("composition '{}' is missing", composition_id);
        };

        for layer in layers {
            let Layer::Precomp {
                id,
                composition,
                collapse_transformations,
                ..
            } = layer
            else {
                continue;
            };
            let target = composition.as_str();
            self.collect_plan_entries(target, stack, entries)?;
            let collapse_issues = self.collapse_issues_for_target(target);
            let collapse_mode = if *collapse_transformations && collapse_issues.is_empty() {
                CollapseMode::CollapseSupportedVectors
            } else {
                CollapseMode::RasterizeFirst
            };
            entries.push(PrecompPlanEntry {
                parent_composition: composition_id,
                layer_id: id,
                target_composition: target,
                collapse_requested: *collapse_transformations,
                collapse_mode,
                collapse_issues,
            });
        }

        stack.pop();
        Ok(())
    }

    fn collapse_issues_for_target(&self, target: &str) -> Vec<&'static str> {
        let Some((target, _)) = self.compositions.iter().find(|(id, _)| **id == target) else {
            return vec![ISSUE_TARGET_MISSING];
        };
        plan_target_collapse(target, |composition_id| self.layers_for(composition_id)).issues
    }

    fn find_layer(
        &self,
        composition_id: &str,
        layer_id: &str,
    ) -> anyhow::Result<Option<(&'a str, &'a Layer)>> {
        let (composition_id, layers) = self
            .compositions
            .iter()
            .find(|(id, _)| **id == composition_id)
            .map(|(id, layers)| (*id, *layers))
            .ok_or_else(|| anyhow::anyhow!("composition '{}' is missing", composition_id))?;

        Ok(layers
            .iter()
            .find(|layer| layer.id() == layer_id)
            .map(|layer| (composition_id, layer)))
    }

    fn deferred_primitive_for_layer_ref(
        &self,
        parent_composition: &'a str,
        root_layer_id: &'a str,
        root_transform: &'a Transform2D,
        layer_ref: &crate::collapse::CollapsedLayerRef<'a>,
    ) -> Option<DeferredRasterPrimitive<'a>> {
        let (kind, source_transform) = deferred_primitive_kind_and_transform(layer_ref.layer)?;
        let mut transform_steps =
            Vec::with_capacity(layer_ref.nested_precomp_path.len().saturating_add(2));
        transform_steps.push(DeferredTransformStep {
            composition_id: parent_composition,
            layer_id: root_layer_id,
            kind: DeferredTransformStepKind::RootPrecompBoundary,
            transform: root_transform,
        });
        for boundary in &layer_ref.nested_precomp_path {
            transform_steps.push(DeferredTransformStep {
                composition_id: boundary.parent_composition,
                layer_id: boundary.layer_id,
                kind: DeferredTransformStepKind::NestedPrecompBoundary,
                transform: boundary.transform,
            });
        }
        transform_steps.push(DeferredTransformStep {
            composition_id: layer_ref.composition_id,
            layer_id: layer_ref.layer.id(),
            kind: DeferredTransformStepKind::SourceLayer,
            transform: source_transform,
        });

        let mut source_time_steps =
            Vec::with_capacity(layer_ref.nested_precomp_path.len().saturating_add(1));
        if let Some((_, root_layer)) = self.find_layer(parent_composition, root_layer_id).ok()? {
            let (start, duration) = root_layer.time_range();
            source_time_steps.push(DeferredSourceTimeStep {
                composition_id: parent_composition,
                layer_id: root_layer_id,
                start,
                duration,
                rule: DeferredSourceTimeRule::LayerRelativeClampedToZero,
            });
        }
        for boundary in &layer_ref.nested_precomp_path {
            let (_, boundary_layer) = self
                .find_layer(boundary.parent_composition, boundary.layer_id)
                .ok()??;
            let (start, duration) = boundary_layer.time_range();
            source_time_steps.push(DeferredSourceTimeStep {
                composition_id: boundary.parent_composition,
                layer_id: boundary.layer_id,
                start,
                duration,
                rule: DeferredSourceTimeRule::LayerRelativeClampedToZero,
            });
        }

        let (start, duration) = layer_ref.layer.time_range();
        Some(DeferredRasterPrimitive {
            kind,
            source_composition: layer_ref.composition_id,
            source_layer_id: layer_ref.layer.id(),
            source_layer: layer_ref.layer,
            transform_steps,
            source_time_steps,
            active_window: DeferredActiveWindow { start, duration },
            opacity: DeferredOpacityContract {
                base_opacity_percent: source_transform.opacity,
                rule: DeferredOpacityRule::EvaluateSourceTransformAtFinalSourceTime,
            },
        })
    }

    fn deferred_raster_barriers_for_precomp(
        &self,
        parent_composition: &'a str,
        layer_id: &'a str,
        target: &str,
    ) -> Vec<DeferredRasterBarrier<'a>> {
        let Some(target) = self.canonical_composition_id(target) else {
            return vec![DeferredRasterBarrier {
                composition_id: parent_composition,
                layer_id,
                reason: ISSUE_TARGET_MISSING,
            }];
        };
        let mut visiting = BTreeSet::new();
        let mut barriers = Vec::new();
        self.collect_deferred_raster_barriers(target, &mut visiting, &mut barriers);
        barriers
    }

    fn collect_deferred_raster_barriers(
        &self,
        composition_id: &'a str,
        visiting: &mut BTreeSet<&'a str>,
        barriers: &mut Vec<DeferredRasterBarrier<'a>>,
    ) {
        if !visiting.insert(composition_id) {
            return;
        }

        let Some(layers) = self.layers_for(composition_id) else {
            visiting.remove(composition_id);
            return;
        };

        for layer in layers {
            if crate::collapse::layer_has_effects(layer) {
                push_barrier_unique(
                    barriers,
                    DeferredRasterBarrier {
                        composition_id,
                        layer_id: layer.id(),
                        reason: crate::collapse::ISSUE_EFFECTS_REQUIRE_RASTER,
                    },
                );
            }

            match layer {
                Layer::Solid { .. } | Layer::Text { .. } => {}
                Layer::Footage { .. } => push_barrier_unique(
                    barriers,
                    DeferredRasterBarrier {
                        composition_id,
                        layer_id: layer.id(),
                        reason: crate::collapse::ISSUE_FOOTAGE_REQUIRES_RASTER,
                    },
                ),
                Layer::Adjustment { .. } => push_barrier_unique(
                    barriers,
                    DeferredRasterBarrier {
                        composition_id,
                        layer_id: layer.id(),
                        reason: crate::collapse::ISSUE_ADJUSTMENT_REQUIRES_RASTER,
                    },
                ),
                Layer::Precomp {
                    composition,
                    collapse_transformations,
                    ..
                } => {
                    if !*collapse_transformations {
                        push_barrier_unique(
                            barriers,
                            DeferredRasterBarrier {
                                composition_id,
                                layer_id: layer.id(),
                                reason: crate::collapse::ISSUE_NESTED_PRECOMP_RASTERIZES,
                            },
                        );
                        continue;
                    }
                    if crate::collapse::layer_has_effects(layer) {
                        continue;
                    }
                    let Some(target) = self.canonical_composition_id(composition) else {
                        push_barrier_unique(
                            barriers,
                            DeferredRasterBarrier {
                                composition_id,
                                layer_id: layer.id(),
                                reason: ISSUE_TARGET_MISSING,
                            },
                        );
                        continue;
                    };
                    if visiting.contains(target) {
                        push_barrier_unique(
                            barriers,
                            DeferredRasterBarrier {
                                composition_id,
                                layer_id: layer.id(),
                                reason: crate::collapse::ISSUE_NESTED_PRECOMP_CYCLE,
                            },
                        );
                        continue;
                    }
                    self.collect_deferred_raster_barriers(target, visiting, barriers);
                }
            }
        }

        visiting.remove(composition_id);
    }

    fn canonical_composition_id(&self, composition_id: &str) -> Option<&'a str> {
        self.compositions
            .keys()
            .find(|id| **id == composition_id)
            .copied()
    }
}

fn cycle_error(stack: &[&str], repeated: &str) -> anyhow::Error {
    let start = stack
        .iter()
        .position(|composition_id| *composition_id == repeated)
        .unwrap_or(0);
    let mut cycle = stack[start..].to_vec();
    cycle.push(repeated);
    anyhow::anyhow!("precomp composition cycle detected: {}", cycle.join(" -> "))
}

fn deferred_primitive_kind_and_transform(
    layer: &Layer,
) -> Option<(DeferredPrimitiveKind, &Transform2D)> {
    match layer {
        Layer::Solid { transform, .. } => Some((DeferredPrimitiveKind::SolidVector, transform)),
        Layer::Text { transform, .. } => Some((DeferredPrimitiveKind::TextVector, transform)),
        _ => None,
    }
}

fn push_barrier_unique<'a>(
    barriers: &mut Vec<DeferredRasterBarrier<'a>>,
    barrier: DeferredRasterBarrier<'a>,
) {
    if !barriers.contains(&barrier) {
        barriers.push(barrier);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_ir::{Composition, EffectSpec, Rect, Transform2D};

    #[test]
    fn builds_lookup_from_scene_root() {
        let scene = scene_with_layers("root", vec![solid_layer("solid")]);
        let graph = PrecompGraph::from_scene(&scene).unwrap();

        assert_eq!(graph.root_id(), "root");
        assert_eq!(graph.layers_for("root").unwrap().len(), 1);
        assert!(graph.layers_for("missing").is_none());
        graph.validate().unwrap();
    }

    #[test]
    fn validates_explicit_nested_compositions() {
        let root_layers = vec![precomp_layer("root_pre", "child", false)];
        let child_layers = vec![precomp_layer("child_pre", "grandchild", false)];
        let grandchild_layers = vec![solid_layer("leaf")];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
                CompositionNode {
                    id: "grandchild",
                    layers: &grandchild_layers,
                },
            ],
        )
        .unwrap();

        graph.validate().unwrap();
        let plan = graph.render_plan().unwrap();
        assert_eq!(
            plan.entries
                .iter()
                .map(|entry| entry.layer_id)
                .collect::<Vec<_>>(),
            vec!["child_pre", "root_pre"]
        );
    }

    #[test]
    fn detects_cycle_in_explicit_nested_compositions() {
        let root_layers = vec![precomp_layer("root_pre", "child", false)];
        let child_layers = vec![precomp_layer("child_pre", "root", false)];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
            ],
        )
        .unwrap();

        let error = graph.validate().unwrap_err().to_string();
        assert!(error.contains("root -> child -> root"), "{error}");
    }

    #[test]
    fn reports_collapse_feasibility_issues() {
        let root_layers = vec![precomp_layer("root_pre", "child", true)];
        let child_layers = vec![
            footage_layer("movie"),
            adjustment_layer("grade"),
            solid_layer_with_effect("shadowed"),
        ];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
            ],
        )
        .unwrap();

        let issues = graph.collapse_issues();
        assert_eq!(issues.len(), 3);
        let plan = graph.render_plan().unwrap();
        assert_eq!(plan.entries[0].collapse_mode, CollapseMode::RasterizeFirst);
        assert_eq!(plan.entries[0].collapse_issues.len(), 3);
        assert!(graph.validate_collapse_feasibility().is_err());

        let collapse_plan = graph
            .collapse_plan_for_precomp_layer("root", "root_pre")
            .unwrap();
        assert!(!collapse_plan.can_flatten_with_parent_matrix());
        assert_eq!(collapse_plan.mode, CollapseMode::RasterizeFirst);
        assert_eq!(collapse_plan.issues.len(), 3);
    }

    #[test]
    fn marks_vector_only_collapse_as_supported() {
        let root_layers = vec![precomp_layer("root_pre", "child", true)];
        let child_layers = vec![solid_layer("shape"), text_layer("title")];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
            ],
        )
        .unwrap();

        graph.validate_collapse_feasibility().unwrap();
        let plan = graph.render_plan().unwrap();
        assert_eq!(
            plan.entries[0].collapse_mode,
            CollapseMode::CollapseSupportedVectors
        );

        let collapse_plan = graph
            .collapse_plan_for_precomp_layer("root", "root_pre")
            .unwrap();
        assert!(collapse_plan.can_flatten_with_parent_matrix());
        assert_eq!(collapse_plan.flattened_layers.len(), 2);
        assert!(collapse_plan
            .flattened_layers
            .iter()
            .all(|layer| layer.nested_precomp_path.is_empty()));
    }

    #[test]
    fn deferred_contract_lists_vector_and_text_primitives() {
        let mut root_pre = precomp_layer("root_pre", "child", true);
        if let Layer::Precomp {
            start, transform, ..
        } = &mut root_pre
        {
            *start = 2.0;
            transform.scale = [180.0, 180.0];
        }
        let mut title = text_layer("title");
        if let Layer::Text {
            start, duration, ..
        } = &mut title
        {
            *start = 0.5;
            *duration = 3.0;
        }
        let child_layers = vec![solid_layer("shape"), title];
        let root_layers = vec![root_pre];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
            ],
        )
        .unwrap();

        let plan = graph
            .deferred_raster_plan_for_precomp_layer("root", "root_pre")
            .unwrap();

        assert!(plan.can_defer_all_primitives());
        assert_eq!(plan.mode, CollapseMode::CollapseSupportedVectors);
        assert!(plan.raster_barriers.is_empty());
        assert_eq!(plan.deferred_primitives.len(), 2);
        assert_eq!(
            plan.deferred_primitives
                .iter()
                .map(|primitive| (primitive.source_layer_id, primitive.kind))
                .collect::<Vec<_>>(),
            vec![
                ("shape", DeferredPrimitiveKind::SolidVector),
                ("title", DeferredPrimitiveKind::TextVector)
            ]
        );

        let title = plan
            .deferred_primitives
            .iter()
            .find(|primitive| primitive.source_layer_id == "title")
            .unwrap();
        assert_eq!(title.source_composition, "child");
        assert_eq!(
            title
                .transform_steps
                .iter()
                .map(|step| (step.layer_id, step.kind))
                .collect::<Vec<_>>(),
            vec![
                ("root_pre", DeferredTransformStepKind::RootPrecompBoundary),
                ("title", DeferredTransformStepKind::SourceLayer)
            ]
        );
        assert_eq!(title.transform_steps[0].transform.scale, [180.0, 180.0]);
        assert_eq!(
            title.source_time_steps,
            vec![DeferredSourceTimeStep {
                composition_id: "root",
                layer_id: "root_pre",
                start: 2.0,
                duration: 1.0,
                rule: DeferredSourceTimeRule::LayerRelativeClampedToZero,
            }]
        );
        assert_eq!(
            title.active_window,
            DeferredActiveWindow {
                start: 0.5,
                duration: 3.0,
            }
        );
        assert_eq!(
            title.opacity.rule,
            DeferredOpacityRule::EvaluateSourceTransformAtFinalSourceTime
        );
    }

    #[test]
    fn deferred_contract_accumulates_nested_precomp_boundaries_in_order() {
        let root_layers = vec![precomp_layer("root_pre", "child", true)];
        let mut child_pre = precomp_layer("child_pre", "grandchild", true);
        if let Layer::Precomp {
            start, transform, ..
        } = &mut child_pre
        {
            *start = 1.25;
            transform.position = [10.0, 20.0];
        }
        let child_layers = vec![solid_layer("shape"), child_pre];
        let grandchild_layers = vec![text_layer("title")];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
                CompositionNode {
                    id: "grandchild",
                    layers: &grandchild_layers,
                },
            ],
        )
        .unwrap();

        let plan = graph
            .deferred_raster_plan_for_precomp_layer("root", "root_pre")
            .unwrap();

        assert!(plan.can_defer_all_primitives());
        let title = plan
            .deferred_primitives
            .iter()
            .find(|primitive| primitive.source_layer_id == "title")
            .unwrap();
        assert_eq!(title.source_composition, "grandchild");
        assert_eq!(
            title
                .transform_steps
                .iter()
                .map(|step| (step.composition_id, step.layer_id, step.kind))
                .collect::<Vec<_>>(),
            vec![
                (
                    "root",
                    "root_pre",
                    DeferredTransformStepKind::RootPrecompBoundary
                ),
                (
                    "child",
                    "child_pre",
                    DeferredTransformStepKind::NestedPrecompBoundary
                ),
                (
                    "grandchild",
                    "title",
                    DeferredTransformStepKind::SourceLayer
                )
            ]
        );
        assert_eq!(title.transform_steps[1].transform.position, [10.0, 20.0]);
        assert_eq!(
            title
                .source_time_steps
                .iter()
                .map(|step| (step.composition_id, step.layer_id, step.start, step.rule))
                .collect::<Vec<_>>(),
            vec![
                (
                    "root",
                    "root_pre",
                    0.0,
                    DeferredSourceTimeRule::LayerRelativeClampedToZero
                ),
                (
                    "child",
                    "child_pre",
                    1.25,
                    DeferredSourceTimeRule::LayerRelativeClampedToZero
                )
            ]
        );
    }

    #[test]
    fn deferred_contract_reports_raster_barriers_by_layer() {
        let root_layers = vec![precomp_layer("root_pre", "child", true)];
        let child_layers = vec![
            text_layer("title"),
            footage_layer("movie"),
            adjustment_layer("grade"),
            solid_layer_with_effect("shadowed"),
            precomp_layer("child_pre", "grandchild", false),
        ];
        let grandchild_layers = vec![text_layer("nested_title")];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
                CompositionNode {
                    id: "grandchild",
                    layers: &grandchild_layers,
                },
            ],
        )
        .unwrap();

        let plan = graph
            .deferred_raster_plan_for_precomp_layer("root", "root_pre")
            .unwrap();

        assert!(!plan.can_defer_all_primitives());
        assert_eq!(plan.mode, CollapseMode::RasterizeFirst);
        assert_eq!(plan.deferred_primitives.len(), 1);
        assert_eq!(plan.deferred_primitives[0].source_layer_id, "title");
        assert_eq!(
            plan.raster_barriers,
            vec![
                DeferredRasterBarrier {
                    composition_id: "child",
                    layer_id: "movie",
                    reason: crate::collapse::ISSUE_FOOTAGE_REQUIRES_RASTER,
                },
                DeferredRasterBarrier {
                    composition_id: "child",
                    layer_id: "grade",
                    reason: crate::collapse::ISSUE_ADJUSTMENT_REQUIRES_RASTER,
                },
                DeferredRasterBarrier {
                    composition_id: "child",
                    layer_id: "shadowed",
                    reason: crate::collapse::ISSUE_EFFECTS_REQUIRE_RASTER,
                },
                DeferredRasterBarrier {
                    composition_id: "child",
                    layer_id: "child_pre",
                    reason: crate::collapse::ISSUE_NESTED_PRECOMP_RASTERIZES,
                },
            ]
        );
    }

    #[test]
    fn marks_nested_collapsed_vector_precomp_as_supported() {
        let root_layers = vec![precomp_layer("root_pre", "child", true)];
        let child_layers = vec![
            solid_layer("shape"),
            precomp_layer("child_pre", "grandchild", true),
        ];
        let grandchild_layers = vec![text_layer("title")];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
                CompositionNode {
                    id: "grandchild",
                    layers: &grandchild_layers,
                },
            ],
        )
        .unwrap();

        graph.validate_collapse_feasibility().unwrap();
        let plan = graph.render_plan().unwrap();
        assert_eq!(plan.entries.len(), 2);
        assert!(plan
            .entries
            .iter()
            .all(|entry| entry.collapse_mode == CollapseMode::CollapseSupportedVectors));

        let collapse_plan = graph
            .collapse_plan_for_precomp_layer("root", "root_pre")
            .unwrap();
        assert!(collapse_plan.can_flatten_with_parent_matrix());
        assert_eq!(collapse_plan.flattened_layers.len(), 2);
        let nested_leaf = collapse_plan
            .flattened_layers
            .iter()
            .find(|layer| layer.layer.id() == "title")
            .unwrap();
        assert_eq!(nested_leaf.composition_id, "grandchild");
        assert_eq!(nested_leaf.nested_precomp_path.len(), 1);
        assert_eq!(nested_leaf.nested_precomp_path[0].layer_id, "child_pre");
    }

    #[test]
    fn rejects_nested_non_collapsed_precomp_for_collapse() {
        let root_layers = vec![precomp_layer("root_pre", "child", true)];
        let child_layers = vec![precomp_layer("child_pre", "grandchild", false)];
        let grandchild_layers = vec![text_layer("title")];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
                CompositionNode {
                    id: "grandchild",
                    layers: &grandchild_layers,
                },
            ],
        )
        .unwrap();

        let collapse_plan = graph
            .collapse_plan_for_precomp_layer("root", "root_pre")
            .unwrap();
        assert_eq!(collapse_plan.mode, CollapseMode::RasterizeFirst);
        assert!(collapse_plan
            .issues
            .contains(&crate::collapse::ISSUE_NESTED_PRECOMP_RASTERIZES));
    }

    #[test]
    fn collapse_planning_keeps_missing_refs_as_graph_errors() {
        let root_layers = vec![precomp_layer("root_pre", "missing", true)];
        let graph = PrecompGraph::from_nodes(
            "root",
            [CompositionNode {
                id: "root",
                layers: &root_layers,
            }],
        )
        .unwrap();

        let error = graph.render_plan().unwrap_err().to_string();
        assert!(error.contains("references missing composition 'missing'"));
        let collapse_plan = graph
            .collapse_plan_for_precomp_layer("root", "root_pre")
            .unwrap();
        assert_eq!(collapse_plan.mode, CollapseMode::RasterizeFirst);
        assert!(collapse_plan
            .issues
            .contains(&crate::collapse::ISSUE_TARGET_MISSING));
    }

    #[test]
    fn collapse_planning_keeps_cycles_as_graph_errors() {
        let root_layers = vec![precomp_layer("root_pre", "child", true)];
        let child_layers = vec![precomp_layer("child_pre", "root", true)];
        let graph = PrecompGraph::from_nodes(
            "root",
            [
                CompositionNode {
                    id: "root",
                    layers: &root_layers,
                },
                CompositionNode {
                    id: "child",
                    layers: &child_layers,
                },
            ],
        )
        .unwrap();

        let error = graph.render_plan().unwrap_err().to_string();
        assert!(error.contains("precomp composition cycle detected: root -> child -> root"));
        let collapse_plan = graph
            .collapse_plan_for_precomp_layer("root", "root_pre")
            .unwrap();
        assert_eq!(collapse_plan.mode, CollapseMode::RasterizeFirst);
        assert!(collapse_plan
            .issues
            .contains(&crate::collapse::ISSUE_NESTED_PRECOMP_CYCLE));
    }

    fn scene_with_layers(id: &str, layers: Vec<Layer>) -> Scene {
        Scene {
            version: "test".to_string(),
            composition: Composition {
                id: id.to_string(),
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

    fn precomp_layer(id: &str, composition: &str, collapse_transformations: bool) -> Layer {
        Layer::Precomp {
            id: id.to_string(),
            start: 0.0,
            duration: 1.0,
            composition: composition.to_string(),
            collapse_transformations,
            transform: Transform2D::default(),
            effects: Vec::new(),
        }
    }

    fn solid_layer(id: &str) -> Layer {
        Layer::Solid {
            id: id.to_string(),
            start: 0.0,
            duration: 1.0,
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

    fn solid_layer_with_effect(id: &str) -> Layer {
        let mut layer = solid_layer(id);
        if let Layer::Solid { effects, .. } = &mut layer {
            effects.push(EffectSpec {
                match_name: "ADBE Drop Shadow".to_string(),
                params: serde_json::json!({}),
            });
        }
        layer
    }

    fn text_layer(id: &str) -> Layer {
        Layer::Text {
            id: id.to_string(),
            start: 0.0,
            duration: 1.0,
            text: "hello".to_string(),
            font: "default".to_string(),
            fontSize: 12.0,
            fill: [255, 255, 255, 255],
            box_: None,
            transform: Transform2D::default(),
            text_animators: Vec::new(),
            effects: Vec::new(),
        }
    }

    fn footage_layer(id: &str) -> Layer {
        Layer::Footage {
            id: id.to_string(),
            start: 0.0,
            duration: 1.0,
            source: "asset".to_string(),
            source_start: 0.0,
            transform: Transform2D::default(),
            effects: Vec::new(),
        }
    }

    fn adjustment_layer(id: &str) -> Layer {
        Layer::Adjustment {
            id: id.to_string(),
            start: 0.0,
            duration: 1.0,
            effects: Vec::new(),
        }
    }
}
