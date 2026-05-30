use crate::collapse::CollapseMode;
use crate::precomp::{DeferredPrimitiveKind, PrecompDeferredRasterPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeeTextCarrierRouteStatus {
    PrecompRasterizeFirst,
    BeeTextCarrierRequired,
    BeeTextCarrierRouted,
    BeeTextCarrierUnsupported,
}

impl BeeTextCarrierRouteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrecompRasterizeFirst => "precomp_rasterize_first",
            Self::BeeTextCarrierRequired => "bee_text_carrier_required",
            Self::BeeTextCarrierRouted => "bee_text_carrier_routed",
            Self::BeeTextCarrierUnsupported => "bee_text_carrier_unsupported",
        }
    }

    pub fn counts_as_direct_p6_success(self) -> bool {
        false
    }

    pub fn counts_as_p5_green(self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeeTextCarrierRouteRecord<'a> {
    pub parent_composition: &'a str,
    pub precomp_layer_id: &'a str,
    pub target_composition: &'a str,
    pub source_composition: Option<&'a str>,
    pub source_layer_id: Option<&'a str>,
    pub status: BeeTextCarrierRouteStatus,
    pub reason: &'static str,
    pub transform_step_count: usize,
    pub source_time_step_count: usize,
}

pub fn classify_bee_text_carrier_routes<'a>(
    plan: &'a PrecompDeferredRasterPlan<'a>,
) -> Vec<BeeTextCarrierRouteRecord<'a>> {
    if !plan.collapse_requested {
        return vec![plan_level_record(
            plan,
            BeeTextCarrierRouteStatus::PrecompRasterizeFirst,
            "collapse_not_requested",
        )];
    }

    if plan.mode != CollapseMode::CollapseSupportedVectors {
        return vec![plan_level_record(
            plan,
            BeeTextCarrierRouteStatus::PrecompRasterizeFirst,
            if plan.raster_barriers.is_empty() {
                "collapse_mode_rasterize_first"
            } else {
                "raster_barrier_present"
            },
        )];
    }

    let mut records = Vec::new();
    for primitive in &plan.deferred_primitives {
        if primitive.kind != DeferredPrimitiveKind::TextVector {
            continue;
        }
        records.push(BeeTextCarrierRouteRecord {
            parent_composition: plan.parent_composition,
            precomp_layer_id: plan.layer_id,
            target_composition: plan.target_composition,
            source_composition: Some(primitive.source_composition),
            source_layer_id: Some(primitive.source_layer_id),
            status: BeeTextCarrierRouteStatus::BeeTextCarrierRequired,
            reason: "collapsed_text_vector_requires_bee_carrier_payload",
            transform_step_count: primitive.transform_steps.len(),
            source_time_step_count: primitive.source_time_steps.len(),
        });
    }

    records
}

fn plan_level_record<'a>(
    plan: &'a PrecompDeferredRasterPlan<'a>,
    status: BeeTextCarrierRouteStatus,
    reason: &'static str,
) -> BeeTextCarrierRouteRecord<'a> {
    BeeTextCarrierRouteRecord {
        parent_composition: plan.parent_composition,
        precomp_layer_id: plan.layer_id,
        target_composition: plan.target_composition,
        source_composition: None,
        source_layer_id: None,
        status,
        reason,
        transform_step_count: 0,
        source_time_step_count: 0,
    }
}
