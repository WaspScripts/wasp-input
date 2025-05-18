use lazy_static::lazy_static;
use std::sync::Mutex;
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
const IMAGE_DATA_SIZE: usize = 33177602;
const BUFFER_SIZE: usize = std::mem::size_of::<SharedMemory>();

#[repr(C, packed)]
pub struct SharedMemory {
    pub flag: u8,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub width: i32,
    pub height: i32,
    pub img_size: i32,
    pub img: [u8; IMAGE_DATA_SIZE],
}

pub struct MemoryManager {
    ptr: *mut SharedMemory,
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

        Self { ptr }
    }

    pub unsafe fn open_map() -> Self {
        let hmap = OpenFileMappingA(
            FILE_MAP_ALL_ACCESS.0,
            false,
            PCSTR(SHARED_MEM_NAME.as_ptr()),
        )
        .expect("[WaspInput]: Cannot open shared memory.\r\n");

        let view = MapViewOfFile(hmap, FILE_MAP_ALL_ACCESS, 0, 0, BUFFER_SIZE);
        assert!(!view.Value.is_null(), "[WaspInput]: Cannot map memory.\r\n");

        let ptr = view.Value as *mut SharedMemory;
        Self { ptr }
    }

    pub unsafe fn is_mapped(&self) -> bool {
        !self.ptr.is_null() && (*self.ptr).flag == 1
    }

    pub unsafe fn image_ptr(&self) -> *mut u8 {
        (*self.ptr).img.as_ptr() as *mut u8
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

    pub unsafe fn set_dimensions(&self, width: i32, height: i32, frame_size: i32) {
        (*self.ptr).width = width;
        (*self.ptr).height = height;
        (*self.ptr).img_size = frame_size;
    }

    /* pub unsafe fn get_img_size(&self) -> i32 {
        (*self.ptr).img_size
    } */
}

lazy_static! {
    pub static ref MEMORY_MANAGER: Mutex<MemoryManager> =
        Mutex::new(unsafe { MemoryManager::open_map() });
}
