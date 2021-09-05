#![windows_subsystem = "windows"]
use clap::Clap;
use filo_clipboard::{cli::Opts, run};

fn main() {
    let opts = Opts::parse();

    run(opts);
}