use std::ffi::CString;
use std::mem::{offset_of, zeroed};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::{self, addr_of_mut, copy_nonoverlapping, null, null_mut, read};

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

//Simba Information
#[repr(C, packed)]
pub struct TSimbaInfomation {
    pub simba_version: i32,
    pub simba_major: i32,
    pub file_name: *const c_char,
    pub compiler: *mut core::ffi::c_void,
}

#[repr(C, packed)]
pub struct TSimbaMethods {
    pub run_on_main_thread: Option<
        unsafe extern "C" fn(
            method: extern "C" fn(*mut core::ffi::c_void),
            data: *mut core::ffi::c_void,
        ),
    >,
    pub get_mem: Option<unsafe extern "C" fn(size: usize) -> *mut core::ffi::c_void>,
    pub free_mem: Option<unsafe extern "C" fn(ptr: *mut core::ffi::c_void)>,
    pub alloc_mem: Option<unsafe extern "C" fn(size: usize) -> *mut core::ffi::c_void>,
    pub realloc_mem: Option<
        unsafe extern "C" fn(
            ptr: *mut *mut core::ffi::c_void,
            size: usize,
        ) -> *mut core::ffi::c_void,
    >,
    pub mem_size: Option<unsafe extern "C" fn(ptr: *mut core::ffi::c_void) -> usize>,

    pub raise_exception: Option<unsafe extern "C" fn(message: *const c_char)>,

    pub get_type_info: Option<
        unsafe extern "C" fn(
            compiler: *mut core::ffi::c_void,
            typ: *const c_char,
        ) -> *mut core::ffi::c_void,
    >,
    pub get_type_info_size: Option<unsafe extern "C" fn(typeinfo: *mut core::ffi::c_void) -> isize>,
    pub get_type_info_field_offset: Option<
        unsafe extern "C" fn(typeinfo: *mut core::ffi::c_void, field: *const c_char) -> isize,
    >,

    pub allocate_raw_array:
        Option<unsafe extern "C" fn(element_size: usize, len: usize) -> *mut core::ffi::c_void>,
    pub reallocate_raw_array: Option<
        unsafe extern "C" fn(
            array: *mut *mut core::ffi::c_void,
            element_size: usize,
            new_len: usize,
        ),
    >,

    pub allocate_array: Option<
        unsafe extern "C" fn(
            type_info: *mut core::ffi::c_void,
            len: usize,
        ) -> *mut core::ffi::c_void,
    >,
    pub allocate_string:
        Option<unsafe extern "C" fn(data: *const c_char) -> *mut core::ffi::c_void>,
    pub allocate_unicode_string:
        Option<unsafe extern "C" fn(data: *const u16) -> *mut core::ffi::c_void>,

    pub set_array_length: Option<
        unsafe extern "C" fn(
            type_info: *mut core::ffi::c_void,
            var: *mut *mut core::ffi::c_void,
            new_len: usize,
        ),
    >,
    pub get_array_length: Option<unsafe extern "C" fn(var: *mut core::ffi::c_void) -> usize>,

    pub external_image_create:
        Option<unsafe extern "C" fn(auto_resize: bool) -> *mut core::ffi::c_void>,
    pub external_image_set_memory: Option<
        unsafe extern "C" fn(
            img: *mut core::ffi::c_void,
            data: *mut core::ffi::c_void,
            width: i32,
            height: i32,
        ),
    >,
    pub external_image_resize:
        Option<unsafe extern "C" fn(img: *mut core::ffi::c_void, new_width: i32, new_height: i32)>,
    pub external_image_set_user_data: Option<
        unsafe extern "C" fn(img: *mut core::ffi::c_void, user_data: *mut core::ffi::c_void),
    >,
    pub external_image_get_user_data:
        Option<unsafe extern "C" fn(img: *mut core::ffi::c_void) -> *mut core::ffi::c_void>,
}

#[no_mangle]
pub static mut PLUGIN_SIMBA_INFO: TSimbaInfomation = TSimbaInfomation {
    simba_version: 0,
    simba_major: 0,
    file_name: null(),
    compiler: null_mut(),
};

#[no_mangle]
pub static mut PLUGIN_SIMBA_METHODS: TSimbaMethods = unsafe { zeroed() };

// Optional memory management helpers
#[repr(C)]
pub struct TSimbaMemoryAllocators {
    pub get_mem: Option<extern "C" fn(size: usize) -> *mut c_void>,
    pub free_mem: Option<extern "C" fn(p: *mut c_void) -> usize>,
}

#[repr(C)]
pub struct TMemoryManager {
    pub get_mem: Option<extern "C" fn(size: usize) -> *mut c_void>,
    pub free_mem: Option<extern "C" fn(p: *mut c_void) -> usize>,
}

#[no_mangle]
pub unsafe extern "C" fn SetPluginMemManager(mem_mgr: TMemoryManager) {
    let _ = mem_mgr;
    // Implement if needed
}

#[no_mangle]
pub unsafe extern "C" fn SetPluginSimbaMethods(methods: TSimbaMethods) {
    PLUGIN_SIMBA_METHODS = methods;
}

#[no_mangle]
pub unsafe extern "C" fn SetPluginSimbaMemoryAllocators(_allocators: TSimbaMemoryAllocators) {
    // Implement if needed
}

#[no_mangle]
pub unsafe extern "C" fn RegisterSimbaPlugin(
    info: *const TSimbaInfomation,
    methods: *const TSimbaMethods,
) {
    if info.is_null() || methods.is_null() {
        return;
    }

    let major = (*info).simba_major;

    if major < 2000 {
        let dst_info = addr_of_mut!(PLUGIN_SIMBA_INFO) as *mut _ as *mut u8;
        let src_info = info as *const u8;
        copy_nonoverlapping(
            src_info,
            dst_info,
            size_of::<i32>() * 2 + size_of::<*const c_char>(),
        );

        let dst_methods = addr_of_mut!(PLUGIN_SIMBA_METHODS) as *mut _ as *mut u8;
        let src_methods = methods as *const u8;
        let size_to_copy = offset_of!(TSimbaMethods, raise_exception);
        ptr::copy_nonoverlapping(src_methods, dst_methods, size_to_copy);
    } else {
        PLUGIN_SIMBA_INFO = read(info);
        PLUGIN_SIMBA_METHODS = read(methods);
    }
}
