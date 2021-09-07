pub mod cli;
pub mod key_utils;
pub mod winapi_functions;

use cli::Opts;
use clipboard_win::{formats, get_clipboard, set_clipboard};
use core::ptr;
use key_utils::is_key_pressed;
use std::collections::VecDeque;
use std::ffi::CString;
use std::mem;
use winapi::um::winuser;

use crate::{
    key_utils::trigger_keys,
    winapi_functions::{
        add_clipboard_format_listener, create_window_ex_a, register_class_ex_a, register_hotkey,
        remove_clipboard_format_listener, sleep, unregister_hotkey,
    },
};

const MAX_RETRIES: u8 = 10;

pub fn run(opts: Opts) {
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
    .unwrap();

    // Event loop
    let mut cb_history = VecDeque::<String>::new();
    let mut internal_update = false;

    let mut lp_msg = winuser::MSG::default();
    #[cfg(debug_assertions)]
    println!("Ready");
    while unsafe { winuser::GetMessageA(&mut lp_msg, h_wnd, 0, 0) != 0 } {
        // unsafe { winuser::TranslateMessage(&lp_msg) };

        match lp_msg.message {
            winuser::WM_CLIPBOARDUPDATE => {
                if !internal_update {
                    if let Ok(clipboard_data) = get_clipboard::<String, _>(formats::Unicode) {
                        cb_history.push_front(clipboard_data);
                        cb_history.truncate(opts.max_history);
                    }
                } else {
                    internal_update = false;
                }
            }
            winuser::WM_HOTKEY => {
                if lp_msg.wParam == 1
                /*Ctrl + Shift + V*/
                {
                    fn old_state(v_key: i32) -> u32 {
                        match is_key_pressed(v_key) {
                            Ok(false) => winuser::KEYEVENTF_KEYUP,
                            _ => 0,
                        }
                    }

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
                            old_state(winuser::VK_CONTROL),
                            old_state('V' as i32),
                            old_state(winuser::VK_SHIFT),
                        ],
                    ) {
                        Ok(_) => {
                            // Sleep for less time than the lowest possible automatic keystroke repeat ((1000ms / 30) * 0.8)
                            sleep(25);
                            cb_history.pop_front();
                            if let Some(last_addition) = cb_history.front() {
                                internal_update = true;
                                let _ = set_clipboard(formats::Unicode, last_addition);
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
                                sleep(25);
                            }
                        }
                    }
                }
            }
            _ => unsafe {
                winuser::DefWindowProcA(lp_msg.hwnd, lp_msg.message, lp_msg.wParam, lp_msg.lParam);
            },
        }
    }

    let _ = unregister_hotkey(h_wnd, 1);
    let _ = remove_clipboard_format_listener(h_wnd);
}
