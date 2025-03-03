use crate::dll_function_finder::LoadedDLL;
use crate::errors::DLLError;
use crate::win_api_functions::{GetLastError, GetProcAddress, LoadLibraryA};
use core::ffi::{c_char, c_void, CStr};
use core::{mem, ptr};
use log::{debug, trace};
use modular_bitfield::bitfield;
use modular_bitfield::prelude::*;
use std::fs;

use windows_sys::Win32::Foundation::{HINSTANCE, HMODULE};
use windows_sys::Win32::System::Diagnostics::Debug::IMAGE_DIRECTORY_ENTRY_TLS;
use windows_sys::Win32::System::SystemServices::{
    DLL_PROCESS_ATTACH, IMAGE_IMPORT_BY_NAME, IMAGE_ORDINAL_FLAG64,
};
use windows_sys::Win32::System::{
    Diagnostics::Debug::{
        IMAGE_DATA_DIRECTORY, IMAGE_DIRECTORY_ENTRY_BASERELOC, IMAGE_DIRECTORY_ENTRY_IMPORT,
        IMAGE_FILE_DLL, IMAGE_NT_HEADERS64, IMAGE_NT_OPTIONAL_HDR_MAGIC, IMAGE_SECTION_HEADER,
    },
    Memory::{VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE},
    SystemServices::{
        IMAGE_BASE_RELOCATION, IMAGE_DOS_HEADER, IMAGE_IMPORT_DESCRIPTOR, IMAGE_NT_SIGNATURE,
        IMAGE_REL_BASED_DIR64, IMAGE_REL_BASED_HIGH, IMAGE_REL_BASED_HIGHADJ,
        IMAGE_REL_BASED_HIGHLOW, IMAGE_REL_BASED_LOW,
    },
    WindowsProgramming::IMAGE_THUNK_DATA64,
};

#[bitfield]
pub struct RelocData {
    offset: B12,
    reloc_type: B4,
}

type DLLMainFn = extern "stdcall" fn(HINSTANCE, u32, *const c_void) -> bool;

/// will probably fail if the dll is already loaded in the process!
/// @todo check for that and return existing handle if so
pub unsafe fn load_dll_manually(name: &str) -> Result<LoadedDLL, DLLError> {
    let hdl = load_and_relocate_dll(name)?;
    let loaded = init_dll(hdl)?;
    Ok(loaded)
}

unsafe fn load_and_relocate_dll(name: &str) -> Result<HMODULE, DLLError> {
    // this is unsafe since rust can't enforce type usage while loading the windows API.
    // Note that the same problem applies in C/C++ there's just no way to check it (e.g. nothing is safe)
    let mut dll_file = "C:\\Windows\\system32\\".to_owned();
    dll_file.push_str(name);
    debug!("Loading DLL manually...{0}", dll_file);
    let dll_contents = fs::read(dll_file)?;
    trace!(
        "Content start: {:x}, end {:x}",
        dll_contents.as_ptr().addr(),
        dll_contents.as_ptr().addr() + dll_contents.len()
    );

    let dos_header = &*(dll_contents.as_ptr() as *const IMAGE_DOS_HEADER);
    // e_lfanew is logical file address of the new exe header (dos being the old one)
    let nt_header =
        &*(dll_contents.as_ptr().offset(dos_header.e_lfanew as isize) as *const IMAGE_NT_HEADERS64);
    // next thing after the nt headers is the section headers array
    let n_sections = nt_header.FileHeader.NumberOfSections as usize;
    const HDR_SIZE: isize = size_of::<IMAGE_NT_HEADERS64>() as isize; // 264

    let section_headers_start = (nt_header as *const IMAGE_NT_HEADERS64 as *const c_void)
        .offset(HDR_SIZE) as *const IMAGE_SECTION_HEADER;
    let _section_headers = alloc::slice::from_raw_parts(section_headers_start, n_sections);
    let image_size = nt_header.OptionalHeader.SizeOfImage as usize;
    check_nt_data(nt_header)?;

    debug!("Image size: {}", image_size);

    let new_memory = VirtualAlloc(
        ptr::null(),
        image_size,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_EXECUTE_READWRITE,
    );
    trace!(
        "Memory start {:x} end {:x}",
        new_memory.addr(),
        new_memory.addr() + image_size
    );

    if new_memory.is_null() {
        return Err(DLLError::AllocationFailure);
    }

    trace!("New section headers: {:?}", n_sections);

    let header_size = nt_header.OptionalHeader.SizeOfHeaders as usize;
    trace!(
        "copying {:x} bytes from {:x} to {:x}",
        header_size,
        dll_contents.as_ptr().addr(),
        new_memory.addr()
    );

    ptr::copy(dll_contents.as_ptr(), new_memory as *mut u8, header_size);

    let relocation_data: IMAGE_DATA_DIRECTORY =
        (*nt_header).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_BASERELOC as usize];
    let imports_data: IMAGE_DATA_DIRECTORY =
        (*nt_header).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT as usize];

    let new_dos_header = *(new_memory as *mut IMAGE_DOS_HEADER);
    let new_nt_header =
        new_memory.offset(new_dos_header.e_lfanew as isize) as *mut IMAGE_NT_HEADERS64;
    (*new_nt_header).OptionalHeader.ImageBase = new_memory.addr() as u64;
    let new_section_headers_start = new_nt_header.offset(1) as *mut IMAGE_SECTION_HEADER;
    let new_section_headers = core::slice::from_raw_parts(new_section_headers_start, n_sections);

    let (reloc_section, _import_section) = load_sections_and_find_reloc_import_data(
        &dll_contents,
        new_memory,
        new_section_headers,
        &relocation_data,
        &imports_data,
    )?;

    // now that we have the relocation data, use it to update the addresses
    // https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#the-reloc-section-image-only
    // DLLs are position dependent and need to be relocated on load
    // We need to relocate them by a delta of the actual address and the image base
    let preferred_base = nt_header.OptionalHeader.ImageBase as isize;
    let address_delta = new_memory.addr() as isize - preferred_base;
    apply_relocation_data(
        &dll_contents,
        new_memory,
        &relocation_data,
        reloc_section,
        preferred_base,
        address_delta,
    );
    trace!("relocation done");
    // now we have to load dependencies. For simplicity we'll just load them normally

    load_imports_apply_thunks(new_memory, &imports_data)?;

    let callbacks = nt_header.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_TLS as usize]
        .VirtualAddress
        != 0;
    if callbacks {
        debug!("must apply callbacks");
    }

    Ok(new_memory as HMODULE)
}

unsafe fn init_dll(handle: HINSTANCE) -> Result<LoadedDLL, DLLError> {
    let dos_header = handle as *const IMAGE_DOS_HEADER;
    // e_lfanew is logical file address of the new exe header (dos being the old one)
    let nt_header = *(handle.offset((*dos_header).e_lfanew as isize) as *const IMAGE_NT_HEADERS64);
    let main = handle.add(nt_header.OptionalHeader.AddressOfEntryPoint as usize);
    let mainfn = mem::transmute::<_, DLLMainFn>(main);
    let ret = mainfn(handle, DLL_PROCESS_ATTACH, ptr::null());
    if ret == false {
        let e = GetLastError();
        return Err(DLLError::WindowsError(e));
    }
    Ok(LoadedDLL { dll_handle: handle })
}

unsafe fn load_imports_apply_thunks(
    new_memory: *mut c_void,
    imports_data: &IMAGE_DATA_DIRECTORY,
) -> Result<(), DLLError> {
    if imports_data.VirtualAddress == 0 {
        return Err(DLLError::LoadError("Failed to find imports"));
    }
    let num_imports = (imports_data.Size as usize) / size_of::<IMAGE_IMPORT_DESCRIPTOR>();
    let import_descriptor_table: *const IMAGE_IMPORT_DESCRIPTOR =
        new_memory.offset(imports_data.VirtualAddress as isize) as *const IMAGE_IMPORT_DESCRIPTOR;

    debug!(
        "Found {} imports in table: {:?}",
        num_imports, import_descriptor_table
    );

    let imports = core::slice::from_raw_parts(import_descriptor_table, num_imports);
    trace!("made them into a table");

    for import in imports {
        // the last import is a marker, it's name is 0
        if import.Name != 0 {
            load_imported_module(new_memory, import);
        }
    }
    Ok(())
}

unsafe fn load_imported_module(new_memory: *mut c_void, import: &IMAGE_IMPORT_DESCRIPTOR) {
    let name: *const u8 = new_memory.offset(import.Name as isize) as *const u8;

    let module: HMODULE = LoadLibraryA(name) as HMODULE;
    if module.is_null() {
        debug!("Failed to load {:?}", CStr::from_ptr(name as *const c_char));
        return;
    }
    debug!(
        "Loading module: {:?}",
        CStr::from_ptr(name as *const c_char)
    );
    // now we have to apply the "thunks", which is updating the dll's list of addresses where the function exports are
    let mut thunk: *mut IMAGE_THUNK_DATA64 =
        new_memory.offset(import.FirstThunk as isize) as *mut IMAGE_THUNK_DATA64;
    let mut n = 0;
    while (*thunk).u1.AddressOfData != 0 {
        // check if it's an ordinal or named export
        let _og = (*thunk).u1.Function;
        if (*thunk).u1.Ordinal & IMAGE_ORDINAL_FLAG64 != 0 {
            let ordinal = (*thunk).u1.Ordinal & 0xFFFF;
            let resource = ordinal as u16 as usize as *const u8;
            (*thunk).u1.Function = GetProcAddress(module, resource) as u64;
        } else {
            let data: *const IMAGE_IMPORT_BY_NAME = new_memory
                .offset((*thunk).u1.AddressOfData as isize)
                as *const IMAGE_IMPORT_BY_NAME;
            // the data structure is actually a null terminated VLA as described in this msdn article
            // https://learn.microsoft.com/en-us/archive/msdn-magazine/2002/march/inside-windows-an-in-depth-look-into-the-win32-portable-executable-file-format-part-2
            // to get it from the struct we take the address of the name and then cast it to a u8 pointer
            let name = (*data).Name.as_ptr() as *const u8;
            let new_addr = GetProcAddress(module, name);
            (*thunk).u1.Function = new_addr as u64;
            trace!(
                "applied thunk from {:x} to {:x} for {:?}",
                _og,
                (*thunk).u1.Function,
                CStr::from_ptr(name as *const c_char)
            )
        }
        n = n + 1;
        thunk = new_memory
            .offset(import.FirstThunk as isize + n * size_of::<IMAGE_THUNK_DATA64>() as isize)
            as *mut IMAGE_THUNK_DATA64;
    }
}

//noinspection ALL
unsafe fn apply_relocation_data(
    dll_contents: &Vec<u8>,
    new_memory: *mut c_void,
    relocation_data: &IMAGE_DATA_DIRECTORY,
    reloc_section: &IMAGE_SECTION_HEADER,
    _preferred_base: isize,
    address_delta: isize,
) {
    // the relocation data is structured as a relocation image base followed by a variable length
    // array of relocation entries. The relocation base specifies the length of the array.
    // We'll walk through that array and parse as we go, when we've seen the right number of
    // entries the next bytes are another relocation base

    let mut reloc_offset = 0;
    while reloc_offset < relocation_data.Size {
        let reloc_base = &*(dll_contents
            .as_ptr()
            .offset((reloc_section.PointerToRawData + reloc_offset) as isize)
            as *const IMAGE_BASE_RELOCATION);
        reloc_offset += size_of::<IMAGE_BASE_RELOCATION>() as u32;
        let num_relocs = (reloc_base.SizeOfBlock as usize - size_of::<IMAGE_BASE_RELOCATION>())
            / size_of::<RelocData>();
        for _ in 0..num_relocs {
            let reloc_entry = &*(dll_contents
                .as_ptr()
                .offset((reloc_section.PointerToRawData + reloc_offset) as isize)
                as *const RelocData);

            reloc_offset += size_of::<RelocData>() as u32;
            let relo_type = reloc_entry.reloc_type() as u32;
            let relo_offset = reloc_entry.offset() as u32;
            let reloc_base_address = reloc_base.VirtualAddress;
            let reloc_address = (reloc_base_address + relo_offset) as isize;
            apply_reloc(relo_type, new_memory, reloc_address, address_delta);
        }
    }
}

/// reloc_address is relative to image base, address delta is difference between real and preferred base
unsafe fn apply_reloc(
    relo_type: u32,
    image_base: *mut c_void,
    reloc_address: isize,
    address_delta: isize,
) {
    if relo_type == 0 {
        // this means skip the relocation
    } else if IMAGE_REL_BASED_DIR64 == relo_type {
        // 64 bit - there's only one type so this is easy
        let addr_to_reloc = image_base.offset(reloc_address) as *mut u64;
        let address = *addr_to_reloc;
        let new_address = address + address_delta as u64;
        *addr_to_reloc = new_address;
        if *(image_base.offset(reloc_address) as *mut u64) != new_address {
            panic!("relocating {address:x} to {new_address:x} at {addr_to_reloc:?} failed")
        }
    } else {
        // 32 bit relocations
        let addr_to_reloc = image_base.offset(reloc_address) as *mut u32;
        let address = *addr_to_reloc;
        let new_address: u32 = match relo_type {
            IMAGE_REL_BASED_HIGH => address + (address_delta as u32 >> 16), // upper 16 only
            IMAGE_REL_BASED_LOW => address + address_delta as u16 as u32,   // lower 16 only
            IMAGE_REL_BASED_HIGHLOW => (address as isize + address_delta) as u32,
            IMAGE_REL_BASED_HIGHADJ => address,
            _ => address,
        };

        *addr_to_reloc = new_address;
        if *(image_base.offset(reloc_address) as *mut u32) != new_address {
            panic!("relocating 32 bit {address:x} to {new_address:x} at {addr_to_reloc:?} failed")
        }
    }
}

unsafe fn load_sections_and_find_reloc_import_data<'a>(
    dll_contents: &Vec<u8>,
    new_memory: *mut c_void,
    new_section_headers: &'a [IMAGE_SECTION_HEADER],
    relocation_data: &IMAGE_DATA_DIRECTORY,
    imports_data: &IMAGE_DATA_DIRECTORY,
) -> Result<(&'a IMAGE_SECTION_HEADER, &'a IMAGE_SECTION_HEADER), DLLError> {
    let mut reloc_section: Option<&IMAGE_SECTION_HEADER> = None;
    let mut import_section: Option<&IMAGE_SECTION_HEADER> = None;
    for section in new_section_headers {
        if (section.VirtualAddress..(section.VirtualAddress + section.Misc.VirtualSize))
            .contains(&imports_data.VirtualAddress)
        {
            reloc_section = Some(section);
        }
        if (section.VirtualAddress..(section.VirtualAddress + section.Misc.VirtualSize))
            .contains(&relocation_data.VirtualAddress)
        {
            import_section = Some(section);
        }
        let source = dll_contents
            .as_ptr()
            .offset(section.PointerToRawData as isize);
        let dst = new_memory.offset(section.VirtualAddress as isize) as *mut u8;

        ptr::copy_nonoverlapping(source, dst, section.SizeOfRawData as usize);
        debug!(
            "Loaded section {:?} from address {:?} to address {:?}",
            CStr::from_bytes_until_nul(&section.Name),
            section.PointerToRawData,
            section.VirtualAddress
        );
    }
    if let Some(reloc) = reloc_section {
        if let Some(import) = import_section {
            return Ok((import, reloc));
        } else {
            return Err(DLLError::LoadError("Failed to find import section"));
        }
    }
    Err(DLLError::LoadError("Failed to find reloc data"))
}

fn check_nt_data(nt_hdr: &IMAGE_NT_HEADERS64) -> Result<(), DLLError> {
    if nt_hdr.Signature != IMAGE_NT_SIGNATURE {
        // it's not a PE file
        return Err(DLLError::NotAPE);
    }
    if (nt_hdr.FileHeader.Characteristics & IMAGE_FILE_DLL) == 0 {
        // it's not a DLL
        return Err(DLLError::NotADLL);
    }
    if nt_hdr.OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR_MAGIC {
        // It has the wrong magic byte
        return Err(DLLError::InvalidDLL);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::dll_function_finder::ParsedDLL;
    use crate::win_api_functions::load_library::load_dll;
    use std::iter::zip;
    use std::slice;
    #[test]
    fn test_size() {
        assert_eq!(size_of::<RelocData>(), 2);
    }
    #[test]
    fn test_fns() {
        let user32 = unsafe { load_and_relocate_dll("user32.dll") }.unwrap();
        let loaded_not_init = LoadedDLL { dll_handle: user32 };
        let parsed = ParsedDLL::parse_dll(&loaded_not_init).unwrap();
        let index = parsed.get_function_index_by_name(c"MessageBoxW");
        let user32_normal = load_dll("user32").unwrap();
        let parsed2 = ParsedDLL::parse_dll(&user32_normal).unwrap();
        let index2 = parsed2.get_function_index_by_name(c"MessageBoxW");

        assert_eq!(index, index2);

        let man_base = user32.addr();
        let r_base = user32_normal.dll_handle.addr();

        let man_messagebox = parsed.get_function_by_name(c"MessageBoxW").unwrap();
        let messagebox = parsed2.get_function_by_name(c"MessageBoxW").unwrap();

        assert_eq!(
            man_messagebox.addr() - parsed.dll.dll_handle.addr(),
            messagebox.addr() - parsed2.dll.dll_handle.addr()
        );
        let man_ep = parsed.nt_header.OptionalHeader.AddressOfEntryPoint as usize;
        let real_ep = parsed2.nt_header.OptionalHeader.AddressOfEntryPoint as usize;
        debug!(
            "manual entrypoint: \t {:016x}\nreal entrypoint: \t {:016x}",
            man_ep + man_base,
            real_ep + r_base
        );
        assert_eq!(
            parsed.nt_header.OptionalHeader.AddressOfEntryPoint as usize,
            parsed2.nt_header.OptionalHeader.AddressOfEntryPoint as usize
        );

        let man_mb_bytes = unsafe { slice::from_raw_parts(man_messagebox as *const u8, 100) };
        let mb_bytes = unsafe { slice::from_raw_parts(messagebox as *const u8, 100) };
        assert_eq!(man_mb_bytes, mb_bytes);

        let nt_header_manual = parsed.nt_header as *const IMAGE_NT_HEADERS64;
        let nt_bytes_m = unsafe {
            slice::from_raw_parts(
                nt_header_manual as *const usize,
                size_of::<IMAGE_NT_HEADERS64>() / 4,
            )
        };
        let nt_header_real = parsed2.nt_header as *const IMAGE_NT_HEADERS64;
        let nt_bytes_r = unsafe {
            slice::from_raw_parts(
                nt_header_real as *const usize,
                size_of::<IMAGE_NT_HEADERS64>() / 4,
            )
        };
        // all bytes in the headers must either be the same or a relocated address
        for (man, real) in zip(nt_bytes_m, nt_bytes_r) {
            if man != real {
                assert_eq!(*man - man_base, real - r_base);
            }
        }
    }
    #[test]
    fn test_load() {
        let _ = unsafe { load_dll_manually("browcli.dll") }.unwrap();
    }
}
