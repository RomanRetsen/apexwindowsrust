use crate::errors::DLLError;
use std::ffi::c_void;
use std::ptr;

pub mod load_library;
pub mod load_library_manually;

pub type WindowsGlobalError = u32;
pub type FnMessageBox = extern "stdcall" fn(
    hWnd: *const c_void,
    lpText: *const u16,
    lpCaption: *const u16,
    uType: u32,
) -> i32;

pub trait IntoNullTerminatedU16 {
    fn to_nullterminated_u16(&self) -> Vec<u16>;
}

pub trait IntoNullTerminatedU8 {
    fn to_nullterminated_u8(&self) -> Vec<u8>;
}

//rust strings are utf8 with a length attribute, windows ABI strings are UTF16 with a null terminator
impl IntoNullTerminatedU16 for str {
    fn to_nullterminated_u16(&self) -> Vec<u16> {
        self.encode_utf16().chain(Some(0)).collect()
    }
}
impl IntoNullTerminatedU8 for str {
    fn to_nullterminated_u8(&self) -> Vec<u8> {
        self.bytes().chain(Some(0)).collect()
    }
}

pub trait ToResult: Sized {
    fn to_result(&self) -> Result<Self, DLLError>;
}

impl ToResult for i32 {
    fn to_result(&self) -> Result<i32, DLLError> {
        if *self == 0 {
            unsafe { Err(DLLError::WindowsError(GetLastError())) }
        } else {
            Ok(*self)
        }
    }
}
impl ToResult for *const c_void {
    fn to_result(&self) -> Result<*const c_void, DLLError> {
        if *self == ptr::null() {
            unsafe { Err(DLLError::WindowsError(GetLastError())) }
        } else {
            Ok(*self)
        }
    }
}

// the functions we want from kernel32. Link against the kernel32 header; extern "stdcall" says its using the windows ABI and not standard C ABI
#[link(name = "kernel32")]
extern "system" {
    // windows structered error handling uses a global error object.
    //   On an error the function returns nullptr and we retrieve the actual error using getLastError
    pub fn GetLastError() -> WindowsGlobalError;
    // this function accepts a dll name, a nullptr (reserved for future use), and a set of flags
    //https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-loadlibraryexw
    pub fn LoadLibraryA(lpLibFileName: *const u8) -> *const c_void;
    // free library tells the OS it's no longer in use
    pub fn FreeLibrary(hLibModule: *const c_void) -> i32;
    pub fn GetProcAddress(hModule: *const c_void, lpProcName: *const u8) -> *const c_void;

}
