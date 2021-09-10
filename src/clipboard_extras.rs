use clipboard_win::{empty, SysResult};
use winapi::um::winuser::SetClipboardData;

use core::{mem, ptr};

use winapi::ctypes::c_void;

const GHND: winapi::ctypes::c_uint = 0x42;

const BYTES_LAYOUT: std::alloc::Layout = std::alloc::Layout::new::<u8>();

#[inline]
fn noop(_: *mut c_void) {}

#[inline]
fn free_rust_mem(data: *mut c_void) {
    unsafe { std::alloc::dealloc(data as _, BYTES_LAYOUT) }
}

#[inline]
fn unlock_data(data: *mut c_void) {
    unsafe {
        winapi::um::winbase::GlobalUnlock(data);
    }
}

#[inline]
fn free_global_mem(data: *mut c_void) {
    unsafe {
        winapi::um::winbase::GlobalFree(data);
    }
}

pub struct Scope<T: Copy>(pub T, pub fn(T));

impl<T: Copy> Drop for Scope<T> {
    #[inline(always)]
    fn drop(&mut self) {
        (self.1)(self.0)
    }
}

pub struct RawMem(Scope<*mut c_void>);

impl RawMem {
    #[inline(always)]
    pub fn new_rust_mem(size: usize) -> Self {
        let mem = unsafe {
            std::alloc::alloc_zeroed(
                std::alloc::Layout::array::<u8>(size).expect("To create layout for bytes"),
            )
        };
        debug_assert!(!mem.is_null());
        Self(Scope(mem as _, free_rust_mem))
    }

    #[inline(always)]
    pub fn new_global_mem(size: usize) -> SysResult<Self> {
        unsafe {
            let mem = winapi::um::winbase::GlobalAlloc(GHND, size as _);
            if mem.is_null() {
                Err(error_code::SystemError::last())
            } else {
                Ok(Self(Scope(mem, free_global_mem)))
            }
        }
    }

    #[inline(always)]
    pub fn from_borrowed(ptr: ptr::NonNull<c_void>) -> Self {
        Self(Scope(ptr.as_ptr(), noop))
    }

    #[inline(always)]
    pub fn get(&self) -> *mut c_void {
        (self.0).0
    }

    #[inline(always)]
    pub fn release(self) {
        mem::forget(self)
    }

    pub fn lock(&self) -> SysResult<(ptr::NonNull<c_void>, Scope<*mut c_void>)> {
        let ptr = unsafe { winapi::um::winbase::GlobalLock(self.get()) };

        match ptr::NonNull::new(ptr) {
            Some(ptr) => Ok((ptr, Scope(self.get(), unlock_data))),
            None => Err(error_code::SystemError::last()),
        }
    }
}

#[derive(PartialEq, Debug, Default)]
pub struct ClipboardItem {
    pub format: u32,
    pub content: Vec<u8>,
}

///Copies raw bytes onto clipboard with specified `format`, returning whether it was successful.
pub fn set_all(clipbard_items: &[ClipboardItem]) -> Vec<SysResult<()>> {
    let _ = empty();

    clipbard_items
        .iter()
        .map(|item| {
            let data = &item.content;
            let format = item.format;

            let size = data.len();
            debug_assert!(size > 0);

            let mem = RawMem::new_global_mem(size)?;

            {
                let (ptr, _lock) = mem.lock()?;
                unsafe { ptr::copy_nonoverlapping(data.as_ptr(), ptr.as_ptr() as _, size) };
            }

            if unsafe { !SetClipboardData(format, mem.get()).is_null() } {
                //SetClipboardData takes ownership
                mem.release();
                return Ok(());
            }

            Err(error_code::SystemError::last())
        })
        .collect()
}
