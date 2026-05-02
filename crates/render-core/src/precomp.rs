//! Precomp evaluation skeleton.
//!
//! v0: render nested composition into offscreen canvas, then composite as a normal layer.
//! v1: cache precomp frames.
//! v2: implement controlled collapse transformations.

pub use crate::collapse::CollapseMode;
use crate::collapse::{
    plan_precomp_collapse, plan_target_collapse, PrecompCollapsePlan, ISSUE_TARGET_MISSING,
};
use render_ir::{Layer, Scene};
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
        Self::from_nodes(scene.composition.id.as_str(), std::iter::once(root).chain(nested))
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
