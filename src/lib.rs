pub mod cli;
pub mod key_utils;
pub mod winapi_functions;

use cli::Opts;
use clipboard_win::{formats, get_clipboard, set_clipboard};
use core::ptr;
use key_utils::is_key_pressed;
use lazy_static::lazy_static;
use std::collections::VecDeque;
use std::ffi::CString;
use std::mem;
use std::sync::{atomic, RwLock};
use winapi::um::winuser;

use crate::{
    key_utils::trigger_keys,
    winapi_functions::{
        add_clipboard_format_listener, create_window_ex_a, register_class_ex_a, register_hotkey,
        remove_clipboard_format_listener, sleep, unregister_hotkey,
    },
};

const MAX_RETRIES: u8 = 10;

// Have to use global variables to allow access from C callback
static INTERNAL_UPDATE: atomic::AtomicBool = atomic::AtomicBool::new(false);
static MAX_HISTORY: atomic::AtomicUsize = atomic::AtomicUsize::new(10);

lazy_static! {
    static ref CB_HISTORY: RwLock<VecDeque<String>> = RwLock::new(VecDeque::new());
}

extern "system" fn message_watcher_proc(
    h_wnd: *mut winapi::shared::windef::HWND__,
    u_msg: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    let h_wnd = unsafe { &mut *h_wnd };
    match u_msg {
        winuser::WM_CREATE => {
            add_clipboard_format_listener(h_wnd).unwrap();
            register_hotkey(
                h_wnd,
                1,
                (winuser::MOD_CONTROL | winuser::MOD_SHIFT) as u32,
                'V' as u32,
            )
            .unwrap();
            0
        }
        winuser::WM_CLIPBOARDUPDATE => {
            if !INTERNAL_UPDATE.load(atomic::Ordering::Relaxed) {
                if let Ok(clipboard_data) = get_clipboard::<String, _>(formats::Unicode) {
                    let mut history = CB_HISTORY.write().unwrap();
                    history.push_front(clipboard_data);
                    history.truncate(MAX_HISTORY.load(atomic::Ordering::Relaxed));
                }
            } else {
                INTERNAL_UPDATE.store(false, atomic::Ordering::Relaxed);
            }
            0
        }
        winuser::WM_HOTKEY => {
            if w_param == 1
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
                        CB_HISTORY.write().unwrap().pop_front();
                        if let Some(last_addition) = CB_HISTORY.read().unwrap().front() {
                            INTERNAL_UPDATE.store(true, atomic::Ordering::Relaxed);
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
            0
        }
        winuser::WM_DESTROY => {
            let clipboard_listener_removed = remove_clipboard_format_listener(h_wnd);
            unregister_hotkey(h_wnd, 1).unwrap();
            clipboard_listener_removed.unwrap();
            0
        }
        _ => unsafe { winuser::DefWindowProcA(h_wnd, u_msg, w_param, l_param) },
    }
}

pub fn run(opts: Opts) {
    // Move options to global variables
    MAX_HISTORY.store(opts.max_history, atomic::Ordering::Relaxed);

    // Create and register a class
    let class_name = "filo-clipboard_class";
    let window_name = "filo-clipboard";

    let class_name_c_string = CString::new(class_name).unwrap();
    let lp_wnd_class = winuser::WNDCLASSEXA {
        cbSize: mem::size_of::<winuser::WNDCLASSEXA>() as u32,
        lpfnWndProc: Some(message_watcher_proc),
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

    // Create the invisible window
    create_window_ex_a(
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

    // Event loop
    let mut lp_msg = winuser::MSG::default();
    // println!("Ready");
    unsafe {
        while winuser::GetMessageA(&mut lp_msg, ptr::null_mut(), 0, 0) != 0 {
            winuser::TranslateMessage(&lp_msg);
            winuser::DispatchMessageA(&lp_msg);
        }
    };
}
