use clap::Args;

#[derive(Args)]
pub struct PerfArgs {}

pub async fn run_perf(_args: PerfArgs) -> anyhow::Result<()> {
    Ok(())
}
