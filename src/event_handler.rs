use clipboard_win::{formats, Clipboard, EnumFormats, Getter};
use std::collections::VecDeque;
use std::sync::Arc;
use std::{thread, time::Duration};
use winapi::um::winuser;
use crate::window::{LParam, MessageType, WParam};

use crate::clipboard_extras::{set_all, ClipboardItem};
use crate::key_utils::trigger_keys;

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

pub struct EventHandler {
    cb_history: VecDeque<Vec<ClipboardItem>>,
    last_internal_update: Option<Vec<ClipboardItem>>,
    skip_clipboard: bool,
    max_history: usize,
}

impl EventHandler {
    pub fn new(max_history: usize) -> Self {
        EventHandler {
            cb_history: VecDeque::new(),
            last_internal_update: None,
            skip_clipboard: false,
            max_history,
        }
    }

    fn handle_clipboard(&mut self) {
        if let Ok(_clip) = Clipboard::new_attempts(10) {
            let cb_data: Vec<_> = EnumFormats::new()
                .filter_map(|format| {
                    let mut clipboard_data = Vec::new();
                    if let Ok(bytes) = formats::RawData(format).read_clipboard(&mut clipboard_data)
                    {
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
                        self.last_internal_update
                            .as_ref()
                            .map(|last_update| {
                                compare_data(&cb_data, last_update, SIMILARITY_THRESHOLD)
                            })
                            .unwrap_or(ComparisonResult::Different)
                    });
                    let current_item_similarity_handle = scope.spawn(|_| {
                        self.cb_history
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
                    if let Some(cb_data) = self.last_internal_update.as_ref() {
                        println!("prev_item: {}", get_cb_text(cb_data));
                    }

                    if let Some(cb_data) = self.cb_history.front() {
                        println!("current_item: {}", get_cb_text(cb_data));
                    }

                    println!("New item: {}", get_cb_text(&cb_data));
                }

                match (prev_item_similarity, current_item_similarity) {
                    (_, ComparisonResult::Same) | (ComparisonResult::Same, _) => {}
                    (_, ComparisonResult::Similar) | (ComparisonResult::Similar, _) => {
                        #[cfg(debug_assertions)]
                        println!("Updating last element: {}", get_cb_text(&cb_data));
                        if let Some(cb_history_front) = self.cb_history.front_mut() {
                            *cb_history_front = Arc::try_unwrap(cb_data).unwrap();
                            self.last_internal_update = None;
                        }
                    }
                    (ComparisonResult::Different, ComparisonResult::Different) => {
                        #[cfg(debug_assertions)]
                        println!("Appending to history: {}", get_cb_text(&cb_data));
                        self.cb_history
                            .push_front(Arc::try_unwrap(cb_data).unwrap());
                        self.cb_history.truncate(self.max_history);
                        self.last_internal_update = None;
                    }
                }
            }
        }
    }

    fn handle_ctrl_shift_v(&mut self) {
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
                self.last_internal_update = self.cb_history.pop_front();
                if let Some(prev_item) = self.cb_history.front() {
                    if let Ok(_clip) = Clipboard::new_attempts(10) {
                        self.skip_clipboard = true;
                        let _ = set_all(prev_item);
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
    }
}

impl FnMut<(MessageType, WParam, LParam)> for EventHandler {
    extern "rust-call" fn call_mut(&mut self, args: (MessageType, WParam, LParam)) {
        let (message_type, w_param, _l_param) = args;
        match message_type {
            winuser::WM_CLIPBOARDUPDATE => {
                if !self.skip_clipboard {
                    self.handle_clipboard();
                }
                self.skip_clipboard = false;
            }
            winuser::WM_HOTKEY => {
                if w_param == 1 {
                    self.handle_ctrl_shift_v();
                }
            }
            _ => {}
        }
    }
}

impl FnOnce<(MessageType, WParam, LParam)> for EventHandler {
    type Output = ();

    extern "rust-call" fn call_once(mut self, args: (MessageType, WParam, LParam)) {
        self.call_mut(args);
    }
}