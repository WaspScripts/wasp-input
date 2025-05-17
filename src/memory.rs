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
const BUFFER_SIZE: usize = 33177602; //4k image + 2 bits

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

pub unsafe fn image_buffer(width: usize, height: usize, image_data: *mut c_void) -> *mut u8 {
    (image_data as *mut u8).add(width * height * 4)
}

pub unsafe fn get_img_ptr() -> *mut c_void {
    let ptr = MAP_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        return null_mut();
    }

    ptr.add(1) as *mut c_void
}
