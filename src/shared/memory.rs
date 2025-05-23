use std::{
    ptr::{copy_nonoverlapping, null_mut, write_bytes},
    sync::{Mutex, OnceLock},
    thread::sleep,
    time::{Duration, Instant},
};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Memory::{
            CreateFileMappingA, MapViewOfFile, OpenFileMappingA, UnmapViewOfFile,
            FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
        },
    },
};

const VERSION: const VERSION: &str = "e7167fc";str = "b441ae8";
const SHARED_MEM_NAME: &[u8] = b"WASPINPUT_DATA\0";
const IMAGE_DATA_SIZE: usize = 33177602;

#[repr(C, packed)]
pub struct SharedMemory {
    pub flag: u8,
    pub version: [u8; 7],
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub width: i32,
    pub height: i32,
    pub img: [u8; IMAGE_DATA_SIZE],
    pub overlay: [u8; IMAGE_DATA_SIZE],
}

const BUFFER_SIZE: usize = std::mem::size_of::<SharedMemory>();

pub struct MemoryManager {
    ptr: *mut SharedMemory,
    hmap: HANDLE,
}

unsafe impl Send for MemoryManager {}
unsafe impl Sync for MemoryManager {}

impl MemoryManager {
    pub unsafe fn create_map() -> Self {
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
        assert!(!view.Value.is_null(), "[WaspInput]: Cannot map memory.\r\n");

        let ptr = view.Value as *mut SharedMemory;

        // Initialize default values
        (*ptr).flag = 1;
        (*ptr).mouse_x = -1;
        (*ptr).mouse_y = -1;
        (*ptr).width = -1;
        (*ptr).height = -1;
        copy_nonoverlapping(VERSION.as_ptr(), (*ptr).version.as_mut_ptr(), 7);

        Self { ptr, hmap }
    }

    pub unsafe fn open_map(time: u64) -> Self {
        let start = Instant::now();
        let timeout = Duration::from_millis(time);

        let hmap = loop {
            let handle = OpenFileMappingA(
                FILE_MAP_ALL_ACCESS.0,
                false,
                PCSTR(SHARED_MEM_NAME.as_ptr()),
            );

            if let Ok(h) = handle {
                if h.0 != null_mut() {
                    break h;
                }
            }

            if start.elapsed() >= timeout {
                panic!("[WaspInput]: Cannot open shared memory.\r\n");
            }

            sleep(Duration::from_millis(100));
        };

        let view = MapViewOfFile(hmap, FILE_MAP_ALL_ACCESS, 0, 0, BUFFER_SIZE);
        assert!(!view.Value.is_null(), "[WaspInput]: Cannot map memory.\r\n");

        let ptr = view.Value as *mut SharedMemory;

        let version = &(*ptr).version;
        assert!(
            version == VERSION.as_bytes(),
            "[WaspInput]: Simba and Client are using different versions of WaspInput, please restart the client.\r\n"
        );

        Self { ptr, hmap }
    }

    pub unsafe fn close_map(&mut self) {
        if !self.ptr.is_null() {
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.ptr as _,
            })
            .expect("[WaspInput]: Failed to unmap memory map.\r\n");
            self.ptr = null_mut();
        }

        if !self.hmap.is_invalid() {
            let _ = CloseHandle(self.hmap)
                .expect("[WaspInput]: Failed to close memory map handle.\r\n");
            self.hmap = HANDLE::default();
        }
    }

    pub unsafe fn is_mapped(&self) -> bool {
        !self.ptr.is_null() && (*self.ptr).flag == 1
    }

    pub unsafe fn image_ptr(&self) -> *mut u8 {
        (*self.ptr).img.as_ptr() as *mut u8
    }

    pub unsafe fn overlay_ptr(&self) -> *mut u8 {
        (*self.ptr).overlay.as_ptr() as *mut u8
    }

    pub unsafe fn clear_overlay(&self) {
        if !self.ptr.is_null() {
            let overlay_ptr = &mut (*self.ptr).overlay as *mut [u8; IMAGE_DATA_SIZE] as *mut u8;
            write_bytes(overlay_ptr, 0, IMAGE_DATA_SIZE);
        }
    }

    pub unsafe fn get_mouse_position(&self) -> (i32, i32) {
        ((*self.ptr).mouse_x, (*self.ptr).mouse_y)
    }

    pub unsafe fn set_mouse_position(&self, x: i32, y: i32) {
        (*self.ptr).mouse_x = x;
        (*self.ptr).mouse_y = y;
    }

    pub unsafe fn get_dimensions(&self) -> (i32, i32) {
        ((*self.ptr).width, (*self.ptr).height)
    }

    pub unsafe fn set_dimensions(&self, width: i32, height: i32) {
        (*self.ptr).width = width;
        (*self.ptr).height = height;
    }
}

pub static MEMORY_MANAGER: OnceLock<Mutex<MemoryManager>> = OnceLock::new();
