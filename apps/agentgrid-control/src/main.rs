use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "agentgrid-control")]
#[command(about = "AgentGrid control plane placeholder")]
struct Cli {
    #[arg(long, default_value = "agentgrid-control is not yet wired to HTTP")]
    message: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    println!("{}", cli.message);
    Ok(())
}
