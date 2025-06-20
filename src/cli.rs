use clap::{Parser, Subcommand, ArgAction};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Install(InstallArgs),
}

#[derive(Parser)]
pub struct InstallArgs {
    #[arg(required = true)]
    pub packages: Vec<String>,

    #[arg(short, long, action = ArgAction::Append)]
    pub flags: Vec<String>,

    #[arg(short, long)]
    pub yes: bool,
}
