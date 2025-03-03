use std::ffi::c_void;
use std::ptr;
use windows_sys::w; //macro to make windows strings
                    // most basic way to use the windows api is to just link against it and call it as you'd call a normal function
                    // as a result it will be listed as a dependency in the exe and windows will load it when our program starts into an address written
                    // into the processes "import address table"
                    // a malware analyst can then see that we're using a function because it appears in our import address table
#[link(name = "user32")]
extern "system" {
    pub fn MessageBoxW(
        hWnd: *const c_void,
        lpText: *const u16,
        lpCaption: *const u16,
        uType: u32,
    ) -> i32;
}
fn main() {
    // todo - call messagebox!
}
