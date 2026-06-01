use std::{path::PathBuf, process::ExitCode, thread, time::Duration};

use clap::{Parser, Subcommand};
use scheduler_core::{run_due_tasks_once, Store};

#[derive(Debug, Parser)]
#[command(name = "ai-taskd")]
#[command(about = "Local task scheduler daemon")]
struct Cli {
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Run {
        #[arg(long, default_value_t = 1)]
        interval_seconds: u64,
    },
    RunOnce,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> scheduler_core::Result<()> {
    let cli = Cli::parse();
    let store = match cli.db {
        Some(path) => Store::open(path)?,
        None => Store::open_default()?,
    };

    match cli.command {
        Commands::Run { interval_seconds } => loop {
            let runs = run_due_tasks_once(&store)?;
            for run in runs {
                println!(
                    "{} finished with {:?}, exit={:?}",
                    run.task_id, run.status, run.exit_code
                );
            }
            thread::sleep(Duration::from_secs(interval_seconds.max(1)));
        },
        Commands::RunOnce => {
            let runs = run_due_tasks_once(&store)?;
            println!("ran {} due task(s)", runs.len());
        }
    }

    Ok(())
}
