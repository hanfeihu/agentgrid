use std::{path::PathBuf, process::ExitCode};

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use scheduler_core::{run_due_tasks_once, Action, NewTask, Schedule, Store};

#[derive(Debug, Parser)]
#[command(name = "ai-task")]
#[command(about = "Command-line task scheduler for AI clients")]
struct Cli {
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        at: String,
        #[arg(long)]
        program: String,
        #[arg(long = "arg")]
        args: Vec<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long, default_value_t = 300)]
        timeout_seconds: u64,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    Cancel {
        id: String,
    },
    Runs {
        id: String,
        #[arg(long)]
        json: bool,
    },
    RunDue {
        #[arg(long)]
        json: bool,
    },
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
        Commands::Add {
            name,
            at,
            program,
            args,
            cwd,
            timeout_seconds,
            json,
        } => {
            let run_at = DateTime::parse_from_rfc3339(&at)?.with_timezone(&Utc);
            let task = store.create_task(NewTask {
                title: name,
                description: None,
                schedule: Schedule::Once { run_at },
                action: Action::Command {
                    program,
                    args,
                    working_dir: cwd,
                    timeout_seconds,
                },
            })?;

            if json {
                println!("{}", serde_json::to_string_pretty(&task)?);
            } else {
                println!("created {}", task.id);
            }
        }
        Commands::List { json } => {
            let tasks = store.list_tasks()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&tasks)?);
            } else {
                for task in tasks {
                    println!(
                        "{}\t{:?}\t{}\t{}",
                        task.id,
                        task.status,
                        task.next_run_at
                            .map(|time| time.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        task.title
                    );
                }
            }
        }
        Commands::Get { id, json } => {
            let task = store.get_task(&id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&task)?);
            } else {
                println!("id: {}", task.id);
                println!("title: {}", task.title);
                println!("status: {:?}", task.status);
                println!(
                    "next run: {}",
                    task.next_run_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
        }
        Commands::Cancel { id } => {
            store.cancel_task(&id)?;
            println!("cancelled {id}");
        }
        Commands::Runs { id, json } => {
            let runs = store.list_runs(&id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&runs)?);
            } else {
                for run in runs {
                    println!(
                        "{}\t{:?}\t{}\texit={:?}",
                        run.id,
                        run.status,
                        run.finished_at.to_rfc3339(),
                        run.exit_code
                    );
                }
            }
        }
        Commands::RunDue { json } => {
            let runs = run_due_tasks_once(&store)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&runs)?);
            } else {
                println!("ran {} due task(s)", runs.len());
            }
        }
    }

    Ok(())
}
