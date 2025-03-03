use dllloading::dll_function_finder::*;
use dllloading::win_api_functions::load_library::load_dll;
use dllloading::win_api_functions::{FnMessageBox, IntoNullTerminatedU16, ToResult};
use std::mem::transmute;
use std::ptr;

fn main() {
    let r = load_dll("user32.dll");
    let user32 = r.unwrap();
    let parsed = ParsedDLL::parse_dll(&user32).unwrap();

    // todo - get function by name must be completed to work properly
    let p = parsed.get_function_by_name(&c"MessageBoxW").unwrap();
    let messagebox = unsafe { transmute::<_, FnMessageBox>(p) };
    _ = messagebox(
        ptr::null(),
        "caption".to_nullterminated_u16().as_ptr(),
        "title".to_nullterminated_u16().as_ptr(),
        0,
    )
    .to_result()
}
