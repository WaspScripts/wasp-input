use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use crate::windows::get_proc_address;
use crate::{PASCAL_EXPORTS, PASCAL_TYPES}; // bring in the constants

#[no_mangle]
pub extern "C" fn GetFunctionInfo(
    index: c_int,
    address: *mut *mut c_void,
    definition: *mut *mut c_char,
) -> c_int {
    if index >= GetFunctionCount() {
        return -1;
    }

    let (name, def) = PASCAL_EXPORTS[index as usize];
    let name = CString::new(name).unwrap();
    unsafe {
        *address = get_proc_address(name.as_ptr());
        let def = CString::new(def).unwrap();
        ptr::copy(def.as_ptr(), *definition, def.as_bytes_with_nul().len());
    }
    index
}

#[no_mangle]
pub extern "C" fn GetFunctionCount() -> c_int {
    PASCAL_EXPORTS.len() as c_int
}

#[no_mangle]
pub extern "C" fn GetTypeInfo(
    index: c_int,
    typ: *mut *mut c_char,
    definition: *mut *mut c_char,
) -> c_int {
    if index >= GetTypeCount() {
        return -1;
    }

    let (name, def) = PASCAL_TYPES[index as usize];
    let name = CString::new(name).unwrap();
    let def = CString::new(def).unwrap();

    unsafe {
        ptr::copy(name.as_ptr(), *typ, name.as_bytes_with_nul().len());
        ptr::copy(def.as_ptr(), *definition, def.as_bytes_with_nul().len());
    }
    index
}

#[no_mangle]
pub extern "C" fn GetTypeCount() -> c_int {
    PASCAL_TYPES.len() as c_int
}
