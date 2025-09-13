use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Mutex;

use shared::windows::{get_jagrenderview, inject, is_input_enabled, open_console, toggle_input};
use simba::target::{SimbaTarget, TARGETS};

mod client;
mod shared;
mod simba;

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
pub extern "system" fn Inject(path: *const c_char, pid: u32) -> bool {
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

    let new_target = SimbaTarget {
        pid,
        hwnd,
        keyboard: [false; 255],
        mouse: [false; 3],
    };

    let mut targets = TARGETS.lock().unwrap();
    targets.insert(pid, new_target);

    unsafe { inject(module_path, pid) }
}

#[no_mangle]
pub extern "system" fn OpenConsole() {
    let hwnd = WINDOW_HWND.lock().unwrap();

    match *hwnd {
        Some(h) => open_console(h),
        None => return,
    };
}

#[no_mangle]
pub extern "system" fn GetInputState() -> bool {
    let hwnd = WINDOW_HWND.lock().unwrap();
    match *hwnd {
        Some(h) => is_input_enabled(h),
        None => false,
    }
}

#[no_mangle]
pub extern "system" fn SetInputState(state: bool) -> bool {
    let hwnd = WINDOW_HWND.lock().unwrap();
    match *hwnd {
        Some(h) => toggle_input(h, state),
        None => false,
    }
}
