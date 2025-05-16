//Target related methods for Simba 2.0
use lazy_static::lazy_static;
use std::{
    ffi::CStr,
    os::raw::{c_char, c_int, c_void},
    ptr::null_mut,
    sync::Mutex,
};

use windows::Win32::Foundation::POINT;

use crate::{
    memory::{get_img_ptr, image_buffer},
    windows::{
        get_jagrenderview, get_mouse_position, get_window_size, key_down, key_up, keys_send,
        lbutton, mbutton, mouse_move, rbutton, scroll,
    },
};

#[repr(C)]
pub struct SimbaTarget {
    pub pid: u32,
    pub hwnd: u64,
}

lazy_static! {
    pub static ref TARGET: Mutex<SimbaTarget> = Mutex::new(SimbaTarget { pid: 0, hwnd: 0 });
    static ref MOUSE_POSITION: Mutex<POINT> = Mutex::new(POINT { x: -1, y: -1 });
    static ref KEYBOARD_STATE: Mutex<[bool; 255]> = Mutex::new([false; 255]);
    static ref MOUSE_STATE: Mutex<[bool; 2]> = Mutex::new([false; 2]);
}

pub fn get_mouse_pos(hwnd: u64) -> POINT {
    let mut lock = MOUSE_POSITION.lock().unwrap();
    if (lock.x == -1) | (lock.y == -1) {
        match get_mouse_position(hwnd) {
            Some(pt) => *lock = pt,
            None => println!("[WaspInput]: Failed to get mouse position!\r\n"),
        };
    }
    *lock
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_Request(args: *const c_char) -> *mut SimbaTarget {
    if args.is_null() {
        return null_mut();
    }

    let pid = match unsafe { CStr::from_ptr(args) }.to_str() {
        Ok(s) => match s.parse::<u32>() {
            Ok(pid) => pid,
            Err(_) => return null_mut(),
        },
        Err(_) => return null_mut(),
    };

    let hwnd = match get_jagrenderview(pid) {
        Some(hwnd) => hwnd.0 as u64,
        None => {
            println!("[WaspInput]: Couldn't find JagRenderView HWND\r\n");
            return null_mut();
        }
    };

    let mut target = TARGET.lock().unwrap();

    if (target.pid == pid) && (target.hwnd != 0) {
        return &mut *target as *mut SimbaTarget;
    }

    *target = SimbaTarget { pid, hwnd };
    return &mut *target as *mut SimbaTarget;
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_RequestWithDebugImage(
    args: *const c_char,
    image: *mut *mut c_void,
) -> *mut SimbaTarget {
    let target = SimbaPluginTarget_Request(args);

    if target.is_null() {
        return null_mut();
    }

    if !image.is_null() {

        /* unsafe {
            let external_image_create = PLUGIN_SIMBA_METHODS
                .external_image_create
                .expect("external_image_create function pointer is null");

            let external_image_set_memory = PLUGIN_SIMBA_METHODS
                .external_image_set_memory
                .expect("external_image_set_memory function pointer is null");

            let img = external_image_create(true);
            *image = img;
            external_image_set_memory(
                img,
                get_debug_image(width as usize, height as usize) as *mut c_void,
                width,
                height,
            );
        } */
    }

    target
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_Release(target: *mut SimbaTarget) {
    if target.is_null() {
        return;
    }

    let target = TARGET.lock().unwrap();
    println!(
        "Releasing Client PID: {} and HWND: {}\r\n",
        target.pid, target.hwnd
    );

    let mut target = TARGET.lock().unwrap();
    *target = SimbaTarget { pid: 0, hwnd: 0 };
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_GetDimensions(
    target: *mut SimbaTarget,
    width: *mut c_int,
    height: *mut c_int,
) {
    if target.is_null() || width.is_null() || height.is_null() {
        return;
    }

    let target = TARGET.lock().unwrap();
    let (w, h) = get_window_size(target.hwnd);

    unsafe {
        *width = w;
        *height = h;
    }
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_GetImageData(
    target: *mut SimbaTarget,
    x: c_int,
    y: c_int,
    _width: c_int,
    _height: c_int,
    bgra: *mut *mut c_void,
    data_width: *mut c_int,
) -> bool {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return false;
    }

    if bgra.is_null() {
        println!("[WaspInput]: bgra is null!\r\n");
        return false;
    }

    if data_width.is_null() {
        println!("[WaspInput]: data_width is null!\r\n");
        return false;
    }

    let target = TARGET.lock().unwrap();
    let (w, h) = get_window_size(target.hwnd);

    let ptr = unsafe { get_img_ptr() };
    if ptr.is_null() {
        return false;
    }

    unsafe { *data_width = w };

    let img_data = unsafe { image_buffer(w as usize, h as usize, ptr) };
    let offset = ((y * (w) + x) * 4) as isize;
    unsafe { *bgra = img_data.offset(offset) as *mut c_void };

    true
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MousePressed(
    target: *mut SimbaTarget,
    mouse_button: c_int,
) -> bool {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return false;
    }

    let state = MOUSE_STATE.lock().unwrap();
    match mouse_button {
        1 => state[0],
        2 | 4 | 5 => state[1],
        3 => state[2],
        _ => {
            println!("[WaspInput]: Unknown mouse button: {}\r\n", mouse_button);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MousePosition(
    target: *mut SimbaTarget,
    x: *mut i32,
    y: *mut i32,
) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = TARGET.lock().unwrap();
    let pt = get_mouse_pos(target.hwnd);

    unsafe {
        *x = pt.x;
        *y = pt.y;
    };
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseTeleport(target: *mut SimbaTarget, x: c_int, y: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = TARGET.lock().unwrap();
    mouse_move(target.hwnd, x, y);
    let mut lock = MOUSE_POSITION.lock().unwrap();
    *lock = POINT { x: x, y: y };
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseUp(target: *mut SimbaTarget, mouse_button: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = TARGET.lock().unwrap();

    let pt = get_mouse_pos(target.hwnd);
    let mut state = MOUSE_STATE.lock().unwrap();
    match mouse_button {
        1 => {
            lbutton(target.hwnd, false, pt.x, pt.y);
            state[0] = false;
        }
        2 | 4 | 5 => {
            mbutton(target.hwnd, false, pt.x, pt.y);
            state[1] = false;
        }
        3 => {
            rbutton(target.hwnd, false, pt.x, pt.y);
            state[2] = false;
        }
        _ => {
            println!("[WaspInput]: Unknown mouse button: {}\r\n", mouse_button);
            return;
        }
    };
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseDown(target: *mut SimbaTarget, mouse_button: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = TARGET.lock().unwrap();

    let pt = get_mouse_pos(target.hwnd);
    let mut state = MOUSE_STATE.lock().unwrap();
    match mouse_button {
        1 => {
            lbutton(target.hwnd, true, pt.x, pt.y);
            state[0] = true;
        }
        2 | 4 | 5 => {
            mbutton(target.hwnd, true, pt.x, pt.y);
            state[1] = true;
        }
        3 => {
            rbutton(target.hwnd, true, pt.x, pt.y);
            state[2] = true;
        }
        _ => {
            println!("[WaspInput]: Unknown mouse button: {}\r\n", mouse_button);
            return;
        }
    };
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseScroll(target: *mut SimbaTarget, scrolls: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = TARGET.lock().unwrap();
    let pt = get_mouse_pos(target.hwnd);
    scroll(target.hwnd, true, scrolls, pt.x, pt.y);
    println!("[WaspInput]: TODO: Implement SimbaPluginTarget_MouseScroll\r\n");
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyDown(target: *mut SimbaTarget, key: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = TARGET.lock().unwrap();
    key_down(target.hwnd, key);

    let mut state = KEYBOARD_STATE.lock().unwrap();
    state[key as usize] = true;
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyUp(target: *mut SimbaTarget, key: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = TARGET.lock().unwrap();
    key_up(target.hwnd, key);
    let mut state = KEYBOARD_STATE.lock().unwrap();
    state[key as usize] = false;
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeySend(
    target: *mut SimbaTarget,
    text: *mut c_char,
    len: c_int,
    sleeptimes: *mut c_int,
) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    if text.is_null() {
        println!("[WaspInput]: text is null!\r\n");
        return;
    }

    if sleeptimes.is_null() {
        println!("[WaspInput]: sleeptimes is null!\r\n");
        return;
    }

    let target = TARGET.lock().unwrap();
    keys_send(target.hwnd, text, len, sleeptimes);
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyPressed(_target: *mut SimbaTarget, key: c_int) -> bool {
    let state = KEYBOARD_STATE.lock().unwrap();
    state[key as usize]
}
