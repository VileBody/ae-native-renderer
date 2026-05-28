use clap::Parser;
use std::path::PathBuf;

#[allow(dead_code)]
#[path = "../conformance_pack.rs"]
mod conformance_pack;

#[derive(Parser, Debug)]
#[command(name = "p2-text-journal")]
#[command(about = "Lightweight P2 text raster journal runner")]
struct Args {
    #[arg(long, default_value = "fixtures/ae_conformance_pack")]
    pack: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long = "case")]
    cases: Vec<String>,
    #[arg(long)]
    full_events: bool,
    #[arg(long)]
    ae_ref_root: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    conformance_pack::run_p2_text_journal(conformance_pack::P2TextJournalOptions {
        pack: args.pack,
        out: args.out,
        cases: args.cases,
        full_events: args.full_events,
        ae_ref_root: args.ae_ref_root,
    })?;
    Ok(())
}
