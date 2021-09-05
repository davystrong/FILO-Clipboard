use clap::{AppSettings, Clap};

/// This program provides a FILO queue from values copies to the clipboard,
/// which can be used with Ctrl+Shift+V
#[derive(Clap)]
#[clap(version = "1.0", author = "David A. <github.com/davystrong>")]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    /// The maximum number of items to keep in the clipboard history
    #[clap(long, default_value = "50")]
    pub max_history: usize,
}
