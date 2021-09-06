use std::mem;

use winapi::um::winuser;

use crate::winapi_functions::{get_async_key_state, send_input, system_parameters_info_a};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_speed_to_millis_min() {
        assert_eq!(raw_speed_to_millis(0), 400u16);
    }

    #[test]
    fn raw_speed_to_millis_max() {
        assert_eq!(raw_speed_to_millis(31), 33u16);
    }
}

/// Create an input struct from the key code and event
fn create_input(key_code: u16, event: u32) -> winuser::INPUT {
    let kb_input_u = unsafe {
        let mut kb_input_u = winuser::INPUT_u::default();
        *kb_input_u.ki_mut() = winuser::KEYBDINPUT {
            wVk: key_code,
            wScan: 0,
            dwFlags: event,
            time: 0,
            dwExtraInfo: 0,
        };
        kb_input_u
    };

    winuser::INPUT {
        type_: winuser::INPUT_KEYBOARD,
        u: kb_input_u,
    }
}

/// Trigger thef list o key events through the Windows api
pub fn trigger_keys(
    key_codes: &[u16],
    events: &[u32],
) -> Result<u32, error_code::ErrorCode<error_code::SystemCategory>> {
    assert_eq!(key_codes.len(), events.len());
    let mut inputs: Vec<_> = key_codes
        .iter()
        .zip(events.iter())
        .map(|(key_code, event)| create_input(*key_code, *event))
        .collect();

    send_input(
        key_codes.len() as u32,
        &mut inputs,
        mem::size_of::<winuser::INPUT>() as i32,
    )
}

/// Get the speed at which the keyboard repeats a keystroke
pub fn get_keyboard_speed() -> Result<u32, error_code::ErrorCode<error_code::SystemCategory>> {
    let mut raw_speed = 0u32;
    unsafe {
        system_parameters_info_a(
            winuser::SPI_GETKEYBOARDSPEED,
            0,
            &mut raw_speed as *mut _ as *mut std::ffi::c_void,
            0,
        )
    }
    .map(|_| raw_speed)
}

/// Based on https://docs.microsoft.com/en-gb/windows/win32/api/winuser/nf-winuser-systemparametersinfoa?redirectedfrom=MSDN
fn raw_speed_to_millis(raw_speed: u8) -> u16 {
    (400 * 31 - raw_speed as u16 * (400 - 33)) / 31
}

/// Return the max delay before we risk a second keypress being trigger by repeating key calls
pub fn get_max_key_delay() -> Result<u16, error_code::ErrorCode<error_code::SystemCategory>> {
    get_keyboard_speed().map(|raw_speed| raw_speed_to_millis(raw_speed as u8) * 8 / 10)
}

pub fn is_key_pressed(
    v_key: i32,
) -> Result<bool, error_code::ErrorCode<error_code::SystemCategory>> {
    // Mask as per https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getasynckeystate
    let mask = 1i16 << 15;
    get_async_key_state(v_key).map(|state| state & mask != 0)
}
