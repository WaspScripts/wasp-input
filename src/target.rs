//Target related methods for Simba 2.0
use std::{
    collections::HashMap,
    ffi::CStr,
    os::raw::{c_char, c_int, c_void},
    ptr::null_mut,
    sync::Mutex,
};

use crate::platform::{
    get_jagrenderview, get_window_size, key_down, key_up, lbutton, mbutton, mouse_move, rbutton,
};

#[repr(C)]
pub struct SimbaTarget {
    pub pid: u32,
    pub hwnd: u64,
}

struct Point {
    x: i32,
    y: i32,
}

lazy_static::lazy_static! {
    static ref TARGETS: Mutex<HashMap<u32, Box<SimbaTarget>>> = Mutex::new(HashMap::new());
    static ref MOUSE_POSITION: Mutex<Point> = Mutex::new(Point { x: -1, y: -1 });
}

fn set_mouse_position(hwnd: u64) {
    let mut point = Point { x: -1, y: -1 };
    /*  unsafe {
        if GetCursorPos(&mut point).as_bool() {
            // Lock for writing
            let mut lock = MOUSE_POSITION.write().unwrap();
            *lock = point;
        }
    } */
}

fn get_mouse_position() -> (i32, i32) {
    let lock = MOUSE_POSITION.lock().unwrap();
    (lock.x, lock.y)
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

    let mut clients = TARGETS.lock().unwrap();
    if let Some(target) = clients.get(&pid) {
        // Return the raw pointer to the existing Box
        return &**target as *const SimbaTarget as *mut SimbaTarget;
    } else {
        // Create a new SimbaTarget
        let new_target = Box::new(SimbaTarget { pid, hwnd });

        let ptr = &*new_target as *const SimbaTarget as *mut SimbaTarget;
        clients.insert(pid, new_target);
        return ptr;
    }
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
        println!("[WaspInput]: TODO: SimbaPluginTarget_RequestWithDebugImage\r\n");
    }

    target
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_Release(target: *mut SimbaTarget) {
    if target.is_null() {
        return;
    }

    let target = unsafe { Box::from_raw(target) };
    println!(
        "Releasing Client PID: {} and HWND: {}",
        target.pid, target.hwnd
    );

    let mut clients = TARGETS.lock().unwrap();
    clients.remove(&target.pid);
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

    let target = unsafe { Box::from_raw(target) };

    if let Some((w, h)) = get_window_size(target.hwnd) {
        unsafe {
            *width = w;
            *height = h;
        }
    } else {
        unsafe {
            *width = 0;
            *height = 0;
        }
    }
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_GetImageData(
    target: *mut SimbaTarget,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
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

    //let target = unsafe { Box::from_raw(target) };

    println!("[WaspInput]: TODO: Implement SimbaPluginTarget_GetImageData\r\n");
    false
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

    //let target = unsafe { Box::from_raw(target) };
    println!("[WaspInput]: TODO: Implement SimbaPluginTarget_MousePressed\r\n");
    false
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MousePosition(
    target: *mut SimbaTarget,
    x: *mut c_int,
    y: *mut c_int,
) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    if !x.is_null() {
        unsafe { *x = 0 };
    }
    if !y.is_null() {
        unsafe { *y = 0 };
    }

    //let target = unsafe { Box::from_raw(target) };
    let pos = get_mouse_position();
    unsafe {
        *x = pos.0;
        *y = pos.1;
    };
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseTeleport(target: *mut SimbaTarget, x: c_int, y: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = unsafe { Box::from_raw(target) };
    mouse_move(target.hwnd, x, y);
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseUp(target: *mut SimbaTarget, mouse_button: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = unsafe { Box::from_raw(target) };

    let pos = get_mouse_position();
    match mouse_button {
        0 => lbutton(target.hwnd, false, pos.0, pos.1),
        1 => mbutton(target.hwnd, false, pos.0, pos.1),
        2 => rbutton(target.hwnd, false, pos.0, pos.1),
        _ => println!("[WaspInput]: Unknown mouse button: {}\r\n", mouse_button),
    };
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseDown(target: *mut SimbaTarget, mouse_button: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = unsafe { Box::from_raw(target) };

    let pos = get_mouse_position();
    match mouse_button {
        0 => lbutton(target.hwnd, true, pos.0, pos.1),
        1 => mbutton(target.hwnd, true, pos.0, pos.1),
        2 => rbutton(target.hwnd, true, pos.0, pos.1),
        _ => println!("[WaspInput]: Unknown mouse button: {}\r\n", mouse_button),
    };
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseScroll(target: *mut SimbaTarget, scrolls: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    //let target = unsafe { Box::from_raw(target) };

    println!("[WaspInput]: TODO: Implement SimbaPluginTarget_MouseScroll\r\n");
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyDown(target: *mut SimbaTarget, key: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = unsafe { Box::from_raw(target) };
    key_down(target.hwnd, key);
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyUp(target: *mut SimbaTarget, key: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let target = unsafe { Box::from_raw(target) };
    key_up(target.hwnd, key);
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

    if sleeptimes.is_null() {
        println!("[WaspInput]: sleeptimes is null!\r\n");
        return;
    }

    //let target = unsafe { Box::from_raw(target) };

    println!("[WaspInput]: TODO: Implement SimbaPluginTarget_KeySend\r\n");
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyPressed(target: *mut SimbaTarget, key: c_int) -> bool {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return false;
    }

    //let target = unsafe { Box::from_raw(target) };
    println!("[WaspInput]: TODO: Implement SimbaPluginTarget_KeyPressed\r\n");
    false
}
