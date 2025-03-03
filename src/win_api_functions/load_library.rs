use crate::win_api_functions::*;
use std::ffi::c_void;

pub struct LoadedDLL {
    pub dll_handle: *const c_void,
}

pub fn load_dll(name: &str) -> Result<LoadedDLL, DLLError> {
    // this is unsafe since rust can't enforce type usage while loading the windows API.
    // Note that the same problem applies in C/C++ there's just no way to check it (e.g. nothing is safe)
    let h = unsafe { LoadLibraryA(name.to_nullterminated_u8().as_ptr()).to_result()? };
    if h.is_null() {
        return Err(DLLError::InvalidDLL);
    }
    Ok(LoadedDLL { dll_handle: h })
}

impl Drop for LoadedDLL {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.dll_handle);
        }
    }
}
