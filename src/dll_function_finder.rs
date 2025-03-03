use crate::errors::DLLError;
pub use crate::win_api_functions::load_library::LoadedDLL;
use alloc::format;
use core::ffi::{c_char, c_void, CStr};
use core::fmt::{Debug, Formatter};
use log::debug;
use windows_sys::Win32::System::Diagnostics::Debug::{
    IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_NT_HEADERS64, IMAGE_NT_OPTIONAL_HDR_MAGIC,
};
use windows_sys::Win32::System::SystemServices::{
    IMAGE_DOS_HEADER, IMAGE_DOS_SIGNATURE, IMAGE_EXPORT_DIRECTORY,
};
// instead of calling getProcAddress to find the message box function let's just search our loaded dll instead!
// pe structure in rust here https://itehax.com/blog/portable-executable-explained-throught-rust-code
// hiding API calls here https://trikkss.github.io/posts/hiding_windows_api_calls_part1/

/* steps:
   1. Find export table from dll
   2. Browse the name list to find function name
   3. Get ordinal of function name
   4. Get address from ordinal

*/
// this is valid as long as the parsed DLL is valid (which is probably static)
pub struct ParsedDLL<'dll> {
    pub(crate) dll: &'dll LoadedDLL,
    pub(crate) pe_header: &'dll IMAGE_DOS_HEADER,
    pub(crate) nt_header: &'dll IMAGE_NT_HEADERS64,
    pub(crate) export_table: &'dll IMAGE_EXPORT_DIRECTORY,
    pub(crate) exports: ParsedExportTable,
}

impl Debug for ParsedDLL<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> alloc::fmt::Result {
        f.debug_struct("DLL")
            .field("dll", &format!("{:p}", &self.dll))
            .field("pe_header", &format!("{:p}", &self.pe_header))
            .field("nt_header", &format!("{:p}", &self.nt_header))
            .field("export_dir", &format!("{:p}", &self.export_table))
            .finish()
    }
}
impl<'dll> ParsedDLL<'dll> {
    pub fn parse_dll(dll: &LoadedDLL) -> Result<ParsedDLL, DLLError> {
        let handl = dll.dll_handle;
        if handl.is_null() {
            return Err(DLLError::InvalidDLL);
        };

        unsafe {
            // a dll is a PE and the dll handle represents where it's been read into memory
            // &* dereferences the raw pointer then makes a reference to the object it was pointing to
            let pe_header = &*(handl as *const IMAGE_DOS_HEADER);
            // the nt header is at an offset stored in the dos header as e_lfanew (Logical File Address)
            let nt_header =
                &*(handl.offset(pe_header.e_lfanew as isize) as *const IMAGE_NT_HEADERS64);
            let export_table = &*(handl.offset(
                nt_header.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize]
                    .VirtualAddress as isize,
            ) as *const IMAGE_EXPORT_DIRECTORY);
            // check that the pe magic byte is right
            if pe_header.e_magic != IMAGE_DOS_SIGNATURE
                || nt_header.OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR_MAGIC
            {
                return Err(DLLError::InvalidDLL);
            }

            let exports = ParsedExportTable::parse_table(handl, export_table)?;
            Ok(ParsedDLL {
                dll,
                pe_header,
                nt_header,
                export_table,
                exports,
            })
        }
    }
    pub fn get_function_by_name(&self, name: &CStr) -> Option<*const c_void> {
        // todo - complete this function
        None
    }
    fn get_ordinal_for_string(&self, n: isize) -> Option<usize> {
        if n < self.exports.f_count as isize {
            unsafe {
                return Some(*self.exports.f_name_ordinals.offset(n) as usize);
            }
        }
        None
    }

    pub fn get_function_index_by_name(&self, name: &CStr) -> Option<usize> {
        for n in 0..self.exports.name_count {
            if self.get_function_string(n) == name {
                debug!("found function {:?}", self.get_function_string(n));
                return self.get_ordinal_for_string(n as isize);
            }
        }
        None
    }
    // n must be within the number of exported functions!
    unsafe fn get_function_pointer(&self, n: usize) -> *const c_void {
        let p = self.exports.f_addresses.add(n);
        let fp = self.dll.dll_handle.add(*p as usize);
        debug!("function pointer is {:p}", fp);
        fp
    }
    fn get_function_string(&self, n: usize) -> &CStr {
        unsafe {
            CStr::from_ptr(
                self.dll
                    .dll_handle
                    .add(*self.exports.f_names.add(n) as usize) as *const c_char,
            )
        }
    }
}
// https://dev.to/wireless90/exploring-the-export-table-windows-pe-internals-4l47
pub struct ParsedExportTable {
    f_count: usize,
    name_count: usize,
    // addresses and names are both pointers to an array of offsets from the base address
    f_addresses: *const u32,
    f_names: *const u32,
    f_name_ordinals: *const u16,
}

impl Debug for ParsedExportTable {
    fn fmt(&self, f: &mut Formatter) -> alloc::fmt::Result {
        f.debug_struct("ExportTable")
            .field("func_count", &self.f_count)
            .field("name_count", &self.name_count)
            .field("name address", &format!("{:p}", self.f_names))
            .finish()
    }
}

impl<'dll> ParsedExportTable {
    pub fn parse_table(
        base: *const c_void,
        export_table: &IMAGE_EXPORT_DIRECTORY,
    ) -> Result<ParsedExportTable, DLLError> {
        // if the parsed DLL is valid this is all safe to do and will have the same lifetime
        unsafe {
            debug!("base address {:p}", base);
            let f_a = base.offset(export_table.AddressOfFunctions as isize) as *const u32;
            let f_n = base.offset(export_table.AddressOfNames as isize) as *const u32;
            debug!(
                "name offset is {:x} and location is {:p}",
                export_table.AddressOfNames, f_n
            );
            let f_name_ordinals =
                base.offset(export_table.AddressOfNameOrdinals as isize) as *const u16;
            let f_c = export_table.NumberOfFunctions as usize;
            let n_c = export_table.NumberOfNames as usize;
            Ok(ParsedExportTable {
                f_count: f_c,
                name_count: n_c,
                f_addresses: f_a,
                f_names: f_n,
                f_name_ordinals,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win_api_functions::load_library::load_dll;
    use crate::win_api_functions::FnMessageBox;
    use core::ffi::c_void;

    #[test]
    fn test_dll_parse() {
        let user32 = load_dll("user32.dll").unwrap();
        let parsed = ParsedDLL::parse_dll(&user32).unwrap();
        debug!("{:?}", parsed);
        assert_eq!(
            parsed.pe_header as *const IMAGE_DOS_HEADER as *const c_void,
            user32.dll_handle
        );
        // check that the pe magic byte is right
        assert_eq!(parsed.pe_header.e_magic, IMAGE_DOS_SIGNATURE);
        // check that the nt magic byte is right
        assert_eq!(
            parsed.nt_header.OptionalHeader.Magic,
            IMAGE_NT_OPTIONAL_HDR_MAGIC
        );
    }
    #[test]
    fn test_table_parse() {
        let user32 = load_dll("user32").unwrap();
        let parsed = ParsedDLL::parse_dll(&user32).unwrap();

        debug!("{:?}", parsed.exports);
    }

    #[test]
    fn test_message_box() {
        let user32 = load_dll("user32").unwrap();
        let parsed = ParsedDLL::parse_dll(&user32).unwrap();
        let p = parsed.get_function_by_name(&c"MessageBoxW").unwrap();
        let _ = unsafe { core::mem::transmute::<_, FnMessageBox>(p) };
    }
}
