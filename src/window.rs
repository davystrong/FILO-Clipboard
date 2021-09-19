use std::{ffi::CString, mem, ptr};

use winapi::um::winuser;

use crate::winapi_functions::{
    add_clipboard_format_listener, create_window_ex_a, register_class_ex_a, register_hotkey,
    remove_clipboard_format_listener, unregister_hotkey,
};

type MessageType = u32;
type WParam = usize;
type LParam = isize;

pub struct Window<'a, T>
where
    T: FnMut(MessageType, WParam, LParam),
{
    h_wnd: &'a mut winapi::shared::windef::HWND__,
    event_loop_callback: T,
}

impl<T> Window<'_, T>
where
    T: FnMut(MessageType, WParam, LParam),
{
    pub fn new(event_loop_callback: T) -> Self {
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
        // let _clipboard_listener = ClipboardListener::add(h_wnd);

        // Register the hotkey listener to the message window
        register_hotkey(
            h_wnd,
            1,
            (winuser::MOD_CONTROL | winuser::MOD_SHIFT) as u32,
            'V' as u32,
        )
        .unwrap();

        Self {
            h_wnd,
            event_loop_callback,
        }
    }

    pub fn run_event_loop(&mut self) {
        let mut lp_msg = winuser::MSG::default();
        #[cfg(debug_assertions)]
        println!("Ready");
        while unsafe { winuser::GetMessageA(&mut lp_msg, self.h_wnd, 0, 0) != 0 } {
            (self.event_loop_callback)(lp_msg.message, lp_msg.wParam, lp_msg.lParam);
        }
    }
}

impl<T> Drop for Window<'_, T>
where
    T: FnMut(MessageType, WParam, LParam),
{
    fn drop(&mut self) {
        let _ = remove_clipboard_format_listener(&mut self.h_wnd);
        let _ = unregister_hotkey(self.h_wnd, 1);
    }
}
