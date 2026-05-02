use media_gst::{FrameRequest, SourcePlan, TimelineMediaPlan};
use render_ir::{Composition, CompositionNode, Layer, Scene};
use std::collections::{BTreeMap, HashMap};

pub fn build_timeline_media_plan(scene: &Scene) -> anyhow::Result<TimelineMediaPlan> {
    anyhow::ensure!(
        scene.composition.fps > 0.0,
        "composition fps must be positive for media planning"
    );
    anyhow::ensure!(
        scene.composition.duration >= 0.0,
        "composition duration must be non-negative for media planning"
    );

    let nodes = scene
        .compositions
        .iter()
        .map(|node| (node.composition.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let frame_count = (scene.composition.duration * scene.composition.fps).ceil() as u32;
    let mut sources = BTreeMap::<String, Vec<FrameRequest>>::new();
    let mut frames_with_requests = 0_usize;
    let mut max_requests_per_frame = 0_usize;

    for frame in 0..frame_count {
        let render_time = frame as f64 / scene.composition.fps;
        let mut stack = vec![scene.composition.id.clone()];
        let requests = collect_layer_requests(
            &scene.composition,
            &scene.layers,
            &nodes,
            render_time,
            frame,
            render_time,
            &mut stack,
            &mut sources,
        )?;
        if requests > 0 {
            frames_with_requests += 1;
            max_requests_per_frame = max_requests_per_frame.max(requests);
        }
    }

    let source_plans = sources
        .into_iter()
        .map(|(asset_id, requests)| SourcePlan::new(asset_id, requests))
        .collect::<Vec<_>>();

    Ok(TimelineMediaPlan::new(
        scene.composition.fps,
        scene.composition.duration,
        frame_count,
        frames_with_requests,
        max_requests_per_frame,
        source_plans,
    ))
}

fn collect_layer_requests(
    composition: &Composition,
    layers: &[Layer],
    nodes: &HashMap<&str, &CompositionNode>,
    comp_time: f64,
    render_frame: u32,
    render_time: f64,
    stack: &mut Vec<String>,
    sources: &mut BTreeMap<String, Vec<FrameRequest>>,
) -> anyhow::Result<usize> {
    let mut requests = 0_usize;

    for layer in layers {
        if !layer.is_active(comp_time) {
            continue;
        }

        match layer {
            Layer::Footage {
                id,
                start,
                source,
                source_start,
                ..
            } => {
                let source_time = (*source_start + (comp_time - *start)).max(0.0);
                sources
                    .entry(source.clone())
                    .or_default()
                    .push(FrameRequest {
                        render_frame,
                        render_time,
                        source_time,
                        composition: composition.id.clone(),
                        layer_id: id.clone(),
                    });
                requests += 1;
            }
            Layer::Precomp {
                start,
                composition: child_id,
                ..
            } => {
                if stack.iter().any(|id| id == child_id) {
                    anyhow::bail!(
                        "precomp cycle detected while planning media: {} -> {}",
                        stack.join(" -> "),
                        child_id
                    );
                }
                let child = nodes.get(child_id.as_str()).ok_or_else(|| {
                    anyhow::anyhow!("precomp composition '{child_id}' was not found")
                })?;
                let child_time = (comp_time - *start).max(0.0);
                stack.push(child_id.clone());
                requests += collect_layer_requests(
                    &child.composition,
                    &child.layers,
                    nodes,
                    child_time,
                    render_frame,
                    render_time,
                    stack,
                    sources,
                )?;
                stack.pop();
            }
            Layer::Solid { .. } | Layer::Text { .. } | Layer::Adjustment { .. } => {}
        }
    }

    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_ir::{Asset, AssetKind, Transform2D};

    #[test]
    fn plans_direct_footage_requests() {
        let scene = Scene {
            version: "test".to_string(),
            composition: comp("main", 2.0, 2.0),
            compositions: Vec::new(),
            assets: vec![video_asset("video_a")],
            layers: vec![footage("fg", 0.0, 2.0, "video_a", 10.0)],
        };

        let plan = build_timeline_media_plan(&scene).unwrap();
        assert_eq!(plan.frames, 4);
        assert_eq!(plan.total_requests, 4);
        assert_eq!(plan.frames_with_requests, 4);
        let source = plan.source_plan("video_a").unwrap();
        assert_eq!(source.requests[0].source_time, 10.0);
        assert_eq!(source.requests[1].source_time, 10.5);
    }

    #[test]
    fn plans_nested_precomp_footage_in_root_frame_order() {
        let scene = Scene {
            version: "test".to_string(),
            composition: comp("main", 2.0, 1.0),
            compositions: vec![CompositionNode {
                composition: comp("child", 2.0, 2.0),
                layers: vec![footage("child_fg", 0.25, 1.0, "video_a", 2.0)],
            }],
            assets: vec![video_asset("video_a")],
            layers: vec![Layer::Precomp {
                id: "pre".to_string(),
                start: 0.0,
                duration: 1.0,
                composition: "child".to_string(),
                collapse_transformations: false,
                transform: Transform2D::default(),
                effects: Vec::new(),
            }],
        };

        let plan = build_timeline_media_plan(&scene).unwrap();
        assert_eq!(plan.frames, 2);
        assert_eq!(plan.total_requests, 1);
        assert_eq!(plan.frames_with_requests, 1);
        let request = &plan.source_plan("video_a").unwrap().requests[0];
        assert_eq!(request.render_frame, 1);
        assert_eq!(request.render_time, 0.5);
        assert_eq!(request.source_time, 2.25);
        assert_eq!(request.composition, "child");
        assert_eq!(request.layer_id, "child_fg");
    }

    #[test]
    fn rejects_precomp_cycles() {
        let scene = Scene {
            version: "test".to_string(),
            composition: comp("main", 1.0, 1.0),
            compositions: vec![CompositionNode {
                composition: comp("a", 1.0, 1.0),
                layers: vec![Layer::Precomp {
                    id: "cycle".to_string(),
                    start: 0.0,
                    duration: 1.0,
                    composition: "a".to_string(),
                    collapse_transformations: false,
                    transform: Transform2D::default(),
                    effects: Vec::new(),
                }],
            }],
            assets: Vec::new(),
            layers: vec![Layer::Precomp {
                id: "pre".to_string(),
                start: 0.0,
                duration: 1.0,
                composition: "a".to_string(),
                collapse_transformations: false,
                transform: Transform2D::default(),
                effects: Vec::new(),
            }],
        };

        let err = build_timeline_media_plan(&scene).unwrap_err();
        assert!(err.to_string().contains("precomp cycle detected"));
    }

    fn comp(id: &str, fps: f64, duration: f64) -> render_ir::Composition {
        render_ir::Composition {
            id: id.to_string(),
            width: 100,
            height: 100,
            fps,
            duration,
            background: [0, 0, 0, 0],
        }
    }

    fn footage(
        id: &str,
        start: f64,
        duration: f64,
        source: &str,
        source_start: f64,
    ) -> Layer {
        Layer::Footage {
            id: id.to_string(),
            start,
            duration,
            source: source.to_string(),
            source_start,
            transform: Transform2D::default(),
            effects: Vec::new(),
        }
    }

    fn video_asset(id: &str) -> Asset {
        Asset {
            id: id.to_string(),
            kind: AssetKind::Video,
            path: format!("{id}.mp4"),
        }
    }
}
