use std::{
    ptr::{copy_nonoverlapping, null_mut},
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

pub unsafe fn is_memory_shared() -> bool {
    let ptr = MAP_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        return false;
    }
    *ptr == 1
}

//random tests below...
pub unsafe fn write_shared_message(message: &str) -> bool {
    println!("write\r\n");
    let ptr = MAP_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        return false;
    }
    println!("write1\r\n");

    let ptr = ptr.add(1);
    let msg_ptr = ptr.add(1);

    let bytes = message.as_bytes();
    if bytes.len() > BUFFER_SIZE - 2 {
        return false;
    }

    println!("write2\r\n");
    copy_nonoverlapping(bytes.as_ptr(), msg_ptr, bytes.len());
    *msg_ptr.add(bytes.len()) = 0;

    *ptr = 1;
    true
}

pub unsafe fn read_shared_message() -> Option<String> {
    println!("read\r\n");
    let ptr = MAP_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }

    println!("read1\r\n");
    let flag_ptr = ptr.add(1);

    if *flag_ptr == 0 {
        return None;
    }

    println!("read2\r\n");
    let msg_ptr = ptr.add(1);
    let slice = std::slice::from_raw_parts(msg_ptr, BUFFER_SIZE - 2);
    let msg = std::str::from_utf8(slice)
        .ok()?
        .trim_end_matches('\0')
        .to_string();

    *ptr = 0;
    Some(msg)
}
