use std::{
    ffi::c_void,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};

use windows::{
    core::PCSTR,
    Win32::{
        Foundation::HANDLE,
        System::Memory::{
            CreateFileMappingA, MapViewOfFile, OpenFileMappingA, FILE_MAP_ALL_ACCESS,
            PAGE_READWRITE,
        },
    },
};

const SHARED_MEM_NAME: &[u8] = b"WASPINPUT_DATA\0";
const BUFFER_SIZE: usize = 258;

static MAP_PTR: AtomicPtr<u8> = AtomicPtr::new(null_mut());

pub unsafe fn create_shared_memory() -> bool {
    let hmap = CreateFileMappingA(
        HANDLE::default(),
        None,
        PAGE_READWRITE,
        0,
        BUFFER_SIZE as u32,
        PCSTR(SHARED_MEM_NAME.as_ptr()),
    )
    .expect("[WaspInput]: Cannot initialize mappings.\r\n");

    let view = MapViewOfFile(hmap, FILE_MAP_ALL_ACCESS, 0, 0, BUFFER_SIZE);
    if view.Value.is_null() {
        return false;
    }
    MAP_PTR.store(view.Value as *mut u8, Ordering::Release);
    let flag = view.Value as *mut u8;
    *flag = 1;

    true
}

pub unsafe fn open_shared_memory() -> bool {
    let hmap = match OpenFileMappingA(
        FILE_MAP_ALL_ACCESS.0,
        false,
        PCSTR(SHARED_MEM_NAME.as_ptr()),
    )
    .ok()
    {
        Some(hmap) => hmap,
        None => return false,
    };

    let view = MapViewOfFile(hmap, FILE_MAP_ALL_ACCESS, 0, 0, BUFFER_SIZE);
    if view.Value.is_null() {
        return false;
    }

    MAP_PTR.store(view.Value as *mut u8, Ordering::Release);
    true
}

pub unsafe fn is_mapped() -> bool {
    let ptr = MAP_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        return false;
    }
    *ptr == 1
}

pub unsafe fn image_buffer(image_data: *mut c_void) -> *mut u8 {
    const EIOSDATA_SIZE: usize = 128;
    (image_data as *mut u8).add(EIOSDATA_SIZE)
}

pub unsafe fn get_image_data() -> *mut c_void {
    let ptr = MAP_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        return null_mut();
    }

    let flag_ptr = ptr.add(1);
    if *flag_ptr == 0 {
        return null_mut();
    }

    flag_ptr as *mut c_void
}

unsafe fn debug_image_buffer(
    image_data: *mut c_void,
    image_width: usize,
    image_height: usize,
) -> *mut u8 {
    let base = image_buffer(image_data);
    base.add(image_width * image_height * 4)
}

pub unsafe fn get_debug_image(width: usize, height: usize) -> *mut u8 {
    let img_ptr = get_image_data();
    if img_ptr.is_null() {
        null_mut()
    } else {
        debug_image_buffer(img_ptr, width, height)
    }
}

pub fn get_target_dimensions(width: &mut i32, height: &mut i32) {
    if unsafe { !is_mapped() } {
        *width = 100; //placeholder
        *height = 100; //placeholder
        return;
    }
    *width = -1;
    *height = -1;
}
