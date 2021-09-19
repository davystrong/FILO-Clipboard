use std::{collections::VecDeque, ffi::CString, mem, ptr, thread, time::Duration};

use winapi::um::winuser;

use crate::winapi_functions::{
    add_clipboard_format_listener, create_window_ex_a, is_clipboard_format_available,
    register_class_ex_a, register_clipboard_format, register_hotkey,
    remove_clipboard_format_listener, unregister_hotkey,
};

use clipboard_win::{formats, Clipboard, EnumFormats, Getter};

use crate::clipboard_extras::{set_all, ClipboardItem};
use crate::key_utils::trigger_keys;

pub type MessageType = u32;
pub type WParam = usize;
pub type LParam = isize;

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

#[cfg(debug_assertions)]
fn get_cb_text(cb_data: &[ClipboardItem]) -> String {
    cb_data
        .iter()
        .find(|item| item.format == winuser::CF_TEXT)
        .map(|res| String::from_utf8(res.content.clone()).unwrap_or_default())
        .unwrap_or_default()
}

pub struct Window<'a> {
    h_wnd: &'a mut winapi::shared::windef::HWND__,
    cb_history: VecDeque<Vec<ClipboardItem>>,
    last_internal_update: Option<Vec<ClipboardItem>>,
    skip_clipboard: bool,
    max_history: usize,
    ignore_format_id: Option<u32>,
}

impl Window<'_> {
    pub fn new(max_history: usize) -> Self {
        //http://www.clipboardextender.com/developing-clipboard-aware-programs-for-windows/ignoring-clipboard-updates-with-the-cf_clipboard_viewer_ignore-clipboard-format
        let ignore_format_id = match register_clipboard_format("Clipboard Viewer Ignore") {
            Ok(format_id) => Some(format_id),
            Err(_) => {
                println!("Failed to register ignore format. This shouldn't cause a problem as it's only used in very specific clipboard programs");
                None
            }
        };

        // Create and register a class
        let class_name = "filo-clipboard_class";
        let window_name = "filo-clipboard";

        let class_name_c_string = CString::new(class_name).unwrap();
        let lp_wnd_class = winuser::WNDCLASSEXA {
            cbSize: mem::size_of::<winuser::WNDCLASSEXA>() as u32,
            lpfnWndProc: Some(winuser::DefWindowProcA),
            hInstance: ptr::null_mut(),
            lpszClassName: class_name_c_string.as_ptr(),
            style: 0,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null_mut(),
            hIconSm: ptr::null_mut(),
        };

        register_class_ex_a(&lp_wnd_class).unwrap();

        // Create the message window
        let h_wnd = create_window_ex_a(
            winuser::WS_EX_LEFT,
            class_name,
            window_name,
            0,
            0,
            0,
            0,
            0,
            unsafe { &mut *winuser::HWND_MESSAGE },
            None,
            None,
            None,
        )
        .unwrap();

        // Register the clipboard listener to the message window
        add_clipboard_format_listener(h_wnd).unwrap();

        // Register the hotkey listener to the message window
        register_hotkey(
            h_wnd,
            1,
            (winuser::MOD_CONTROL | winuser::MOD_SHIFT) as u32,
            'V' as u32,
        )
        .expect("Could not register hotkey. Is an instance already running?");

        Self {
            h_wnd,
            cb_history: VecDeque::new(),
            last_internal_update: None,
            skip_clipboard: false,
            max_history,
            ignore_format_id,
        }
    }

    pub fn run_event_loop(&mut self) {
        let mut lp_msg = winuser::MSG::default();
        #[cfg(debug_assertions)]
        println!("Ready");
        while unsafe { winuser::GetMessageA(&mut lp_msg, self.h_wnd, 0, 0) != 0 } {
            match lp_msg.message {
                winuser::WM_CLIPBOARDUPDATE => {
                    if !self.skip_clipboard
                        && !self
                            .ignore_format_id
                            .map(is_clipboard_format_available)
                            .unwrap_or(false)
                    {
                        self.handle_clipboard();
                    }
                    self.skip_clipboard = false;
                }
                winuser::WM_HOTKEY => {
                    if lp_msg.wParam == 1 {
                        self.handle_ctrl_shift_v();
                    }
                }
                _ => {}
            }
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
                            *cb_history_front = cb_data;
                            self.last_internal_update = None;
                        }
                    }
                    (ComparisonResult::Different, ComparisonResult::Different) => {
                        #[cfg(debug_assertions)]
                        println!("Appending to history: {}", get_cb_text(&cb_data));
                        self.cb_history.push_front(cb_data);
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

impl Drop for Window<'_> {
    fn drop(&mut self) {
        let _ = remove_clipboard_format_listener(&mut self.h_wnd);
        let _ = unregister_hotkey(self.h_wnd, 1);
    }
}
