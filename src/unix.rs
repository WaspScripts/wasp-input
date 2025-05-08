use libc::{dlsym, RTLD_DEFAULT};

pub unsafe fn get_proc_address(name: *const c_char) -> *mut c_void {
    dlsym(RTLD_DEFAULT, name)
}

pub struct Injector;

impl Injector {
    pub fn inject(module_path: &str, pid: u32) -> bool {
        //TODO...
        false
    }
}

pub fn get_window_size(hwnd: HWND) -> Option<(i32, i32)> {
    None
}

//input
pub fn is_input_enabled(hwnd: u64) -> bool {
    false
}

pub fn enable_input(hwnd: u64) -> bool {
    false
}

pub fn disable_input(hwnd: u64) -> bool {
    false
}

pub fn key_down(hwnd: u64, virtual_key: u16) {}

pub fn key_up(hwnd: u64, virtual_key: u16) {}
