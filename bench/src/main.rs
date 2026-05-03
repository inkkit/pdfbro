use clap::{Parser, Subcommand};

mod driver;
mod perf;
mod preflight;
mod quality;
mod report;
mod rss;
mod stats;
mod workload;

#[derive(Parser)]
#[command(name = "bench", about = "pdfbro benchmarking tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Perf(perf::PerfArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Perf(args) => perf::run_perf(args).await,
    }
}
