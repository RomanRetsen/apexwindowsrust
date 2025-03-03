use dllloading::dll_function_finder::*;
use dllloading::win_api_functions::load_library::load_dll;
use dllloading::win_api_functions::{FnMessageBox, IntoNullTerminatedU16, ToResult};
use obfuscated_strings::*;
use std::mem::transmute;
use std::ptr;
use strobfuscate::obfuscate_str;

fn main() {
    let r = load_dll("user32.dll");
    let user32 = r.unwrap();
    let parsed = ParsedDLL::parse_dll(&user32).unwrap();
    // todo - make obfuscate str work by completing the obfuscation and deobfuscation functions in obfuscated_strings
    let fname = obfuscate_str!("MessageBoxW");
    let p = parsed.get_function_by_name(&fname).unwrap();
    let messagebox = unsafe { transmute::<_, FnMessageBox>(p) };
    _ = messagebox(
        ptr::null(),
        "caption".to_nullterminated_u16().as_ptr(),
        "title".to_nullterminated_u16().as_ptr(),
        0,
    )
    .to_result()
}
