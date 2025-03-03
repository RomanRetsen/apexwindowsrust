use std::ffi::c_void;
use std::ptr;
use windows_sys::w;
use windows_sys::Win32::Foundation::HINSTANCE;
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

#[link(name = "user32")]
extern "system" {
    pub fn MessageBoxW(
        hWnd: *const c_void,
        lpText: *const u16,
        lpCaption: *const u16,
        uType: u32,
    ) -> i32;
}

#[no_mangle]
pub extern "C" fn add(left: usize, right: usize) -> usize {
    left + right
}

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn DllMain(dll_module: HINSTANCE, call_reason: u32, _: *mut ()) -> bool {
    match call_reason {
        DLL_PROCESS_ATTACH => unsafe {
            MessageBoxW(ptr::null_mut(), w!("hello there!"), w!("hi there"), 0);
        },
        DLL_PROCESS_DETACH => (),
        _ => (),
    }

    true
}
