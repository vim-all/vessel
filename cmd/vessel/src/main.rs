use clap::Parser;
use runtime::run;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    rootfs: String,

    command: String,
}

fn main() {
    let args: Args = Args::parse();

    run(&args.rootfs, &args.command)
        .expect("Failed to run container");
}
