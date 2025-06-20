mod cli;
mod commands;
mod utils;

use clap::Parser;
use cli::Cli;
use cli::Commands;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install(args) => commands::install::install(&args.packages, &args.flags, args.yes),
    }
}
