use std::{ffi::CString, ptr};
use winapi::um::winuser;

pub type SystemError = error_code::ErrorCode<error_code::SystemCategory>;

pub fn register_class_ex_a(
    lp_wnd_class: &winuser::WNDCLASSEXA,
) -> Result<u16, error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::RegisterClassExA(lp_wnd_class) } {
        0 => Err(SystemError::last()),
        atom => Ok(atom),
    }
}

pub fn create_window_ex_a<'a>(
    dw_ex_style: u32,
    lp_class_name: &str,
    lp_window_name: &str,
    dw_style: u32,
    x: i32,
    y: i32,
    n_width: i32,
    n_height: i32,
    h_wnd_parent: &'a mut winapi::shared::windef::HWND__,
    h_menu: Option<&'a mut winapi::shared::windef::HMENU__>,
    h_instance: Option<&'a mut winapi::shared::minwindef::HINSTANCE__>,
    lp_param: Option<&'a mut std::ffi::c_void>,
) -> Result<&'a mut winapi::shared::windef::HWND__, error_code::ErrorCode<error_code::SystemCategory>>
{
    //Lifetimes assuming worst case scenario
    let class_name = CString::new(lp_class_name).unwrap();
    let window_name = CString::new(lp_window_name).unwrap();
    match unsafe {
        winuser::CreateWindowExA(
            dw_ex_style,
            class_name.as_ptr(),
            window_name.as_ptr(),
            dw_style,
            x,
            y,
            n_width,
            n_height,
            h_wnd_parent,
            h_menu.map(|x| x as *mut _).unwrap_or(ptr::null_mut()),
            h_instance.map(|x| x as *mut _).unwrap_or(ptr::null_mut()),
            lp_param.map(|x| x as *mut _).unwrap_or(ptr::null_mut()),
        )
    } {
        h_wnd if h_wnd.is_null() => Err(SystemError::last()),
        h_wnd => Ok(unsafe { &mut *h_wnd }),
    }
}

pub fn send_input(
    c_inputs: u32,
    p_inputs: &mut [winuser::INPUT],
    cb_size: i32,
) -> Result<u32, error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::SendInput(c_inputs, p_inputs.as_mut_ptr(), cb_size) } {
        0 => Err(SystemError::last()),
        events => Ok(events),
    }
}

pub fn add_clipboard_format_listener(
    h_wnd: &mut winapi::shared::windef::HWND__,
) -> Result<(), error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::AddClipboardFormatListener(h_wnd) } {
        0 => Err(SystemError::last()),
        _ => Ok(()),
    }
}

pub fn remove_clipboard_format_listener(
    h_wnd: &mut winapi::shared::windef::HWND__,
) -> Result<(), error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::RemoveClipboardFormatListener(h_wnd) } {
        0 => Err(SystemError::last()),
        _ => Ok(()),
    }
}

pub fn is_clipboard_format_available(format: u32) -> bool {
    (unsafe { winuser::IsClipboardFormatAvailable(format) } != 0)
}

pub fn register_clipboard_format(
    lpsz_format: &str,
) -> Result<u32, error_code::ErrorCode<error_code::SystemCategory>> {
    let lpsz_format = CString::new(lpsz_format).unwrap();
    match unsafe { winuser::RegisterClipboardFormatA(lpsz_format.as_ptr()) } {
        0 => Err(SystemError::last()),
        id => Ok(id),
    }
}

pub fn register_hotkey(
    h_wnd: &mut winapi::shared::windef::HWND__,
    id: i32,
    fs_modifiers: u32,
    key_code: u32,
) -> Result<(), error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::RegisterHotKey(h_wnd, id, fs_modifiers, key_code) } {
        0 => Err(SystemError::last()),
        _ => Ok(()),
    }
}

pub fn unregister_hotkey(
    h_wnd: &mut winapi::shared::windef::HWND__,
    id: i32,
) -> Result<(), error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::UnregisterHotKey(h_wnd, id) } {
        0 => Err(SystemError::last()),
        _ => Ok(()),
    }
}

pub unsafe fn system_parameters_info_a(
    ui_action: u32,
    ui_param: u32,
    pv_param: *mut std::ffi::c_void,
    f_win_ini: u32,
) -> Result<(), error_code::ErrorCode<error_code::SystemCategory>> {
    match winuser::SystemParametersInfoA(ui_action, ui_param, pv_param, f_win_ini) {
        0 => Err(SystemError::last()),
        _ => Ok(()),
    }
}

pub fn get_async_key_state(
    v_key: i32,
) -> Result<i16, error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::GetAsyncKeyState(v_key) } {
        0 => Err(SystemError::last()),
        state => Ok(state),
    }
}

pub fn open_clipboard(
    h_wnd: &mut winapi::shared::windef::HWND__,
) -> Result<(), error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::OpenClipboard(h_wnd) } {
        0 => Err(SystemError::last()),
        _ => Ok(()),
    }
}

pub fn close_clipboard() -> Result<(), error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::CloseClipboard() } {
        0 => Err(SystemError::last()),
        _ => Ok(()),
    }
}

pub fn get_clipboard_data(
    u_format: u32,
) -> Result<*mut std::ffi::c_void, error_code::ErrorCode<error_code::SystemCategory>> {
    match unsafe { winuser::GetClipboardData(u_format) } {
        handle if handle.is_null() => Err(SystemError::last()),
        handle => Ok(handle),
    }
}
