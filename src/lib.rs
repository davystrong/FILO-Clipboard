#![feature(unboxed_closures)]
#![feature(fn_traits)]

pub mod cli;
pub mod clipboard_extras;
pub mod key_utils;
pub mod winapi_functions;
pub mod window;
pub mod event_handler;

use cli::Opts;
use crate::window::Window;
use crate::event_handler::EventHandler;

pub fn run(opts: Opts) {
    // Create a window and event handler
    let event_loop_callback = EventHandler::new(opts.max_history);
    let mut window = Window::new(event_loop_callback);
    window.run_event_loop();
}
