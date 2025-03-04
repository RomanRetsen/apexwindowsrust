use dllloading::win_api_functions::load_library::load_dll;
use dllloading::win_api_functions::ToResult;
use std::ffi::c_void;
use std::mem::transmute;
use std::ptr;
use windows_sys::w; //macro to make windows wide strings

// the functions we want from kernel32. Link against the kernel32 header; extern "stdcall" says its using the windows ABI and not standard C ABI
#[link(name = "kernel32")]
extern "system" {
    pub fn GetProcAddress(hModule: *const c_void, lpProcName: *const u8) -> *const c_void;
    pub fn LoadLibraryA(lpLibFileName: *const u8) -> *const c_void;
    // free library tells the OS it's no longer in use
    pub fn FreeLibrary(hLibModule: *const c_void) -> i32;
}
pub(crate) type FnMessageBox = extern "stdcall" fn(
    hWnd: *const c_void,
    lpText: *const u16,
    lpCaption: *const u16,
    uType: u32,
) -> i32;

fn main() {

    unsafe {
        let user32 = LoadLibraryA("user32.dll\0".as_ptr() as *const u8);
        if user32.is_null() {
            eprintln!("Failed to load user32.dll");
            return;
        }else {
            println!("Use32.dll was loaded properly");
        }
        let message_box_w: FnMessageBox = transmute(GetProcAddress(
            user32,
            "MessageBoxW\0".as_ptr() as *const u8,
        ));
        message_box_w(
            ptr::null(),
            w!("Your message here"),
            w!("Title"),
            0x00000000, // MB_OK
        );

        // Free the library when done
        FreeLibrary(user32);
    }

    //todo - load user32 at runtime and call messagebox!
}
