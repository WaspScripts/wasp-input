use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Mutex;

mod memory;
mod windows;
use target::{SimbaTarget, TARGET};
use windows::{get_jagrenderview, inject, is_input_enabled, open_console, toggle_input};

// Pascal types as tuples (name, definition)
const PASCAL_TYPES: &[(&str, &str)] = &[("PHelloChar", "^Char;"), ("PTestInt", "^Int32;")];

// Pascal exports as (name, declaration)
//name as to match the dll function name exactly
const PASCAL_EXPORTS: &[(&str, &str)] = &[
    (
        "Inject",
        "function Inject(dll: String; pid: UInt32): Boolean;",
    ),
    ("OpenConsole", "procedure OpenConsole();"),
    ("GetInputState", "function GetInputState(): Boolean;"),
    (
        "SetInputState",
        "function SetInputState(state: Boolean): Boolean;",
    ),
];

lazy_static::lazy_static! {
    static ref PROCESS_PID: Mutex<Option<u32>> = Mutex::new(None);
    static ref WINDOW_HWND: Mutex<Option<u64>> = Mutex::new(None);
}

// dll functions
#[no_mangle]
pub extern "C" fn Inject(path: *const c_char, pid: u32) -> bool {
    if path.is_null() {
        println!("[WaspInput]: Invalid string\n");
        return false;
    }

    let module_path = unsafe {
        match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => {
                println!("[WaspInput]: Invalid UTF-8\n");
                return false;
            }
        }
    };

    let hwnd = match get_jagrenderview(pid) {
        Some(h) => h.0 as u64,
        None => {
            println!("[WaspInput]: Couldn't find JagRenderView HWND\n");
            return false;
        }
    };

    let mut target = TARGET.lock().unwrap();
    *target = SimbaTarget { pid, hwnd };

    unsafe { inject(module_path, pid) }
}

#[no_mangle]
pub extern "C" fn OpenConsole() {
    let hwnd = WINDOW_HWND.lock().unwrap();

    match *hwnd {
        Some(h) => open_console(h),
        None => return,
    };
}

#[no_mangle]
pub extern "C" fn GetInputState() -> bool {
    let hwnd = WINDOW_HWND.lock().unwrap();
    match *hwnd {
        Some(h) => is_input_enabled(h),
        None => false,
    }
}

#[no_mangle]
pub extern "C" fn SetInputState(state: bool) -> bool {
    let hwnd = WINDOW_HWND.lock().unwrap();
    match *hwnd {
        Some(h) => toggle_input(h, state),
        None => false,
    }
}

mod client;
mod graphics;
mod plugin;
mod target;
