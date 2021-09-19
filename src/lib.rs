pub mod cli;
pub mod clipboard_extras;
pub mod key_utils;
pub mod winapi_functions;
pub mod window;

use crate::window::Window;
use cli::Opts;

pub fn run(opts: Opts) {
    // Create a window and event handler
    let mut window = Window::new(opts.max_history);
    window.run_event_loop();
}
