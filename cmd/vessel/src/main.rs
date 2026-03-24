use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use api::{Request, Response};

const SOCKET_PATH: &str = "/run/vessel.sock";

#[derive(Parser)]
#[command(name = "vessel")]
#[command(about = "Vessel container runtime CLI")]
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
        id: String,
    },
    Rm {
        id: String,
    },
    Logs {
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let request = match cli.command {
        Commands::Run { rootfs, command } => {
            Request::Run { rootfs, command }
        }
        Commands::Ps => Request::Ps,
        Commands::Stop { id } => Request::Stop { id },
        Commands::Rm { id } => Request::Rm { id },
        Commands::Logs { id } => Request::Logs { id },
    };

    match send_request(request) {
        Ok(response) => handle_response(response),
        Err(e) => {
            eprintln!("Error communicating with daemon: {}", e);
            std::process::exit(1);
        }
    }
}

fn send_request(req: Request) -> Result<Response, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(SOCKET_PATH)?;

    let data = serde_json::to_vec(&req)?;
    stream.write_all(&data)?;

    // Important: shutdown write side so daemon knows request is complete
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer)?;

    let response: Response = serde_json::from_slice(&buffer)?;
    Ok(response)
}

fn handle_response(resp: Response) {
    match resp {
        Response::Ok(msg) => {
            println!("{}", msg);
        }
        Response::List(containers) => {
            println!("{:<40} {:<8} {}", "ID", "PID", "STATE");
            for (id, pid, state) in containers {
                println!("{:<40} {:<8} {}", id, pid, state);
            }
        }
        Response::Error(err) => {
            eprintln!("Daemon error: {}", err);
            std::process::exit(1);
        }
    }
}