use clap::{Parser, Subcommand};
use runtime::{run, ps, stop, logs};

#[derive(Parser)]
#[command(name = "vessel")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(long)]
        rootfs: String,

        #[arg(required = true, trailing_var_arg = true)]
        command: Vec<String>,
    },

    Ps,

    Stop {
        #[arg(required = true)]
        id: String,
    },

    Logs {
        #[arg(required = true)]
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { rootfs, command } => {
            let id = run(&rootfs, &command)
                .expect("Failed to run container");
            println!("Container started successfully with ID: {}", id);
        }
        Commands::Ps => {
            ps().expect("Failed to list containers");
        }
        Commands::Stop { id } => {
            stop(&id).expect("Failed to stop container");
        }
        Commands::Logs { id } => {
            logs(&id).expect("Failed to read container logs");
        }
    }
}