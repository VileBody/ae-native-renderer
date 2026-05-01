use render_ir::Scene;

pub fn validate_graph_stub(_scene: &Scene) -> anyhow::Result<()> {
    // TODO: validate precomp cycles, missing assets, unsupported feature flags.
    Ok(())
}
