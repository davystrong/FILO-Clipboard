use crate::winapi_functions::{
    add_clipboard_format_listener, register_hotkey, remove_clipboard_format_listener,
    unregister_hotkey,
};

pub struct ClipboardListener<'a> {
    h_wnd: &'a mut winapi::shared::windef::HWND__,
}

impl<'a> ClipboardListener<'a> {
    pub fn add(h_wnd: &'a mut winapi::shared::windef::HWND__) -> Self {
        add_clipboard_format_listener(h_wnd).unwrap();
        Self { h_wnd }
    }
}

impl Drop for ClipboardListener<'_> {
    fn drop(&mut self) {
        let _ = remove_clipboard_format_listener(self.h_wnd);
    }
}

pub struct HotkeyListener<'a> {
    h_wnd: &'a mut winapi::shared::windef::HWND__,
    id: i32,
}

impl<'a> HotkeyListener<'a> {
    pub fn add(
        h_wnd: &'a mut winapi::shared::windef::HWND__,
        id: i32,
        fs_modifiers: u32,
        key_code: u32,
    ) -> Self {
        register_hotkey(h_wnd, id, fs_modifiers, key_code).unwrap();
        Self { h_wnd, id }
    }
}

impl Drop for HotkeyListener<'_> {
    fn drop(&mut self) {
        let _ = unregister_hotkey(self.h_wnd, self.id);
    }
}
