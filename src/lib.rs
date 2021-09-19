pub mod cli;
pub mod clipboard_extras;
pub mod key_utils;
pub mod winapi_functions;
pub mod window;

use cli::Opts;
use clipboard_win::{formats, Clipboard, EnumFormats, Getter};
use std::collections::VecDeque;
use std::sync::Arc;
use std::{thread, time::Duration};
use winapi::um::winuser;

use crate::clipboard_extras::{set_all, ClipboardItem};
use crate::key_utils::trigger_keys;
use crate::window::Window;

const MAX_RETRIES: u8 = 10;
const SIMILARITY_THRESHOLD: u8 = 230;

#[derive(Debug, PartialEq)]
enum ComparisonResult {
    Same,
    Similar,
    Different,
}

fn compare_data(
    cb_data: &[ClipboardItem],
    prev_cb_data: &[ClipboardItem],
    threshold: u8,
) -> ComparisonResult {
    match (cb_data.len(), prev_cb_data.len()) {
        (0, 0) => ComparisonResult::Same,
        (0, _) | (_, 0) => ComparisonResult::Different,
        _ => {
            let count_eq = cb_data
                .iter()
                .filter(
                    |x| match prev_cb_data.iter().find(|y| x.format == y.format) {
                        Some(y) => **x == *y,
                        None => false,
                    },
                )
                .count();

            let max_eq = *[cb_data.len(), prev_cb_data.len()].iter().max().unwrap();

            if count_eq == max_eq {
                ComparisonResult::Same
            } else if count_eq * 255 >= max_eq * threshold as usize {
                ComparisonResult::Similar
            } else {
                ComparisonResult::Different
            }
        }
    }
}

fn get_cb_text(cb_data: &[ClipboardItem]) -> String {
    cb_data
        .iter()
        .find(|item| item.format == winuser::CF_TEXT)
        .map(|res| String::from_utf8(res.content.clone()).unwrap_or_default())
        .unwrap_or_default()
}

fn handle_clipboard(
    cb_history: &mut VecDeque<Vec<ClipboardItem>>,
    last_internal_update: &mut Option<Vec<ClipboardItem>>,
    max_history: usize,
) {
    if let Ok(_clip) = Clipboard::new_attempts(10) {
        let cb_data: Vec<_> = EnumFormats::new()
            .filter_map(|format| {
                let mut clipboard_data = Vec::new();
                if let Ok(bytes) = formats::RawData(format).read_clipboard(&mut clipboard_data) {
                    if bytes != 0 {
                        return Some(ClipboardItem {
                            format,
                            content: clipboard_data,
                        });
                    }
                }
                None
            })
            .collect();
        let cb_data = Arc::new(cb_data);

        if !cb_data.is_empty() {
            let (prev_item_similarity, current_item_similarity) = crossbeam::scope(|scope| {
                //If let chains would do this far more neatly
                let prev_item_similarity_handle = scope.spawn(|_| {
                    last_internal_update
                        .as_ref()
                        .map(|last_update| {
                            compare_data(&cb_data, last_update, SIMILARITY_THRESHOLD)
                        })
                        .unwrap_or(ComparisonResult::Different)
                });
                let current_item_similarity_handle = scope.spawn(|_| {
                    cb_history
                        .front()
                        .map(|last_update| {
                            compare_data(&cb_data, last_update, SIMILARITY_THRESHOLD)
                        })
                        .unwrap_or(ComparisonResult::Different)
                });

                (
                    prev_item_similarity_handle.join().unwrap(),
                    current_item_similarity_handle.join().unwrap(),
                )
            })
            .unwrap();

            #[cfg(debug_assertions)]
            {
                if let Some(cb_data) = last_internal_update.as_ref() {
                    println!("prev_item: {}", get_cb_text(cb_data));
                }

                if let Some(cb_data) = cb_history.front() {
                    println!("current_item: {}", get_cb_text(cb_data));
                }

                println!("New item: {}", get_cb_text(&cb_data));
            }

            match (prev_item_similarity, current_item_similarity) {
                (_, ComparisonResult::Same) | (ComparisonResult::Same, _) => {}
                (_, ComparisonResult::Similar) | (ComparisonResult::Similar, _) => {
                    #[cfg(debug_assertions)]
                    println!("Updating last element: {}", get_cb_text(&cb_data));
                    if let Some(cb_history_front) = cb_history.front_mut() {
                        *cb_history_front = Arc::try_unwrap(cb_data).unwrap();
                        *last_internal_update = None;
                    }
                }
                (ComparisonResult::Different, ComparisonResult::Different) => {
                    #[cfg(debug_assertions)]
                    println!("Appending to history: {}", get_cb_text(&cb_data));
                    cb_history.push_front(Arc::try_unwrap(cb_data).unwrap());
                    cb_history.truncate(max_history);
                    *last_internal_update = None;
                }
            }
        }
    }
}

fn handle_hotkey(
    cb_history: &mut VecDeque<Vec<ClipboardItem>>,
    last_internal_update: &mut Option<Vec<ClipboardItem>>,
) -> bool {
    #[cfg(debug_assertions)]
    dbg!("Ctrl+Shift+V");

    match trigger_keys(
        &[
            winuser::VK_SHIFT as u16,
            winuser::VK_CONTROL as u16,
            'V' as u16,
            winuser::VK_CONTROL as u16,
            'V' as u16,
            winuser::VK_SHIFT as u16,
        ],
        &[
            winuser::KEYEVENTF_KEYUP,
            winuser::KEYEVENTF_KEYUP,
            winuser::KEYEVENTF_KEYUP,
            0,
            0,
            0,
        ],
    ) {
        Ok(_) => {
            // Sleep for less time than the lowest possible automatic keystroke repeat ((1000ms / 30) * 0.8)
            thread::sleep(Duration::from_millis(25));
            *last_internal_update = cb_history.pop_front();
            if let Some(prev_item) = cb_history.front() {
                if let Ok(_clip) = Clipboard::new_attempts(10) {
                    let _ = set_all(prev_item);
                    return true;
                }
            }
        }
        Err(_) => {
            let mut retries = 0u8;
            while let Err(error) = trigger_keys(
                &[
                    winuser::VK_SHIFT as u16,
                    winuser::VK_CONTROL as u16,
                    'V' as u16,
                ],
                &[
                    winuser::KEYEVENTF_KEYUP,
                    winuser::KEYEVENTF_KEYUP,
                    winuser::KEYEVENTF_KEYUP,
                ],
            ) {
                if retries >= MAX_RETRIES {
                    panic!("Could not release keys after {} attemps. Something has gone badly wrong: {}", MAX_RETRIES, error)
                }
                retries += 1;
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
    false
}

pub fn run(opts: Opts) {
    // Define callbacks
    let mut cb_history = VecDeque::<Vec<ClipboardItem>>::new();
    let mut last_internal_update = Option::<Vec<ClipboardItem>>::None;
    let mut skip_clipboard = false;

    let event_loop_callback = |message_type, w_param, _l_param| match message_type {
        winuser::WM_CLIPBOARDUPDATE => {
            if !skip_clipboard {
                handle_clipboard(&mut cb_history, &mut last_internal_update, opts.max_history);
            }
            skip_clipboard = false;
        }
        winuser::WM_HOTKEY => {
            if w_param == 1 {
                skip_clipboard = handle_hotkey(&mut cb_history, &mut last_internal_update);
            }
        }
        _ => {}
    };

    // Create a Window and register callbacks
    let mut window = Window::new(event_loop_callback);
    window.run_event_loop();
}
