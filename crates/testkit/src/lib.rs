pub mod conformance;
pub mod fixtures;
pub mod golden;
pub mod hypothesis;
pub mod image_diff;
pub mod interpolation;
pub mod obligations;
pub mod operator;
pub mod phase1;
pub mod phase2;
pub mod phase3;
pub mod phase4;
pub mod phase5;
pub mod telemetry;
pub mod temporal;

pub const REQUIRE_AE_PNG_GOLDENS_ENV: &str = "AE_NATIVE_REQUIRE_AE_PNG_GOLDENS";

pub fn ae_png_goldens_required() -> bool {
    std::env::var(REQUIRE_AE_PNG_GOLDENS_ENV)
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "required" | "strict"
            )
        })
}

pub use conformance::*;
pub use fixtures::*;
pub use hypothesis::*;
pub use image_diff::*;
pub use interpolation::*;
pub use obligations::*;
pub use operator::*;
pub use phase1::*;
pub use phase2::*;
pub use phase4::*;
pub use telemetry::*;
pub use temporal::*;
