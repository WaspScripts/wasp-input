//Target related methods for Simba 2.0
use std::{
    collections::HashMap,
    ffi::CStr,
    os::raw::{c_char, c_int, c_void},
    ptr::null_mut,
    sync::Mutex,
};

use crate::platform::{get_jagrenderview, get_window_size, key_down, key_up, unhook_wndproc};

#[repr(C)]
pub struct SimbaTarget {
    pub pid: u32,
    pub hwnd: u64,
}

lazy_static::lazy_static! {
    static ref TARGETS: Mutex<HashMap<u32, Box<SimbaTarget>>> = Mutex::new(HashMap::new());
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
            println!("Couldn't find JagRenderView HWND\r\n");
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
        println!("TODO: SimbaPluginTarget_RequestWithDebugImage\r\n");
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

    unsafe { unhook_wndproc(target.hwnd) };

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
    println!("TODO: Implement SimbaPluginTarget_GetImageData\r\n");
    false
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MousePressed(
    target: *mut SimbaTarget,
    mouse_button: c_int,
) -> bool {
    println!("TODO: Implement SimbaPluginTarget_MousePressed\r\n");
    false
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MousePosition(
    target: *mut SimbaTarget,
    x: *mut c_int,
    y: *mut c_int,
) {
    println!("TODO: Implement SimbaPluginTarget_MousePosition\r\n");
    if !x.is_null() {
        unsafe { *x = 0 };
    }
    if !y.is_null() {
        unsafe { *y = 0 };
    }
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseTeleport(target: *mut SimbaTarget, x: c_int, y: c_int) {
    println!("TODO: Implement SimbaPluginTarget_MouseTeleport\r\n");
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseUp(target: *mut SimbaTarget, mouse_button: c_int) {
    println!("TODO: Implement SimbaPluginTarget_MouseUp\r\n");
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseDown(target: *mut SimbaTarget, mouse_button: c_int) {
    println!("TODO: Implement SimbaPluginTarget_MouseDown\r\n");
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_MouseScroll(target: *mut SimbaTarget, scrolls: c_int) {
    println!("TODO: Implement SimbaPluginTarget_MouseScroll\r\n");
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyDown(target: *mut SimbaTarget, key: c_int) {
    if target.is_null() {
        print!("target is null!\r\n");
        return;
    }

    let target = unsafe { Box::from_raw(target) };
    key_down(target.hwnd, key);
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyUp(target: *mut SimbaTarget, key: c_int) {
    if target.is_null() {
        print!("target is null!\r\n");
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
    println!("TODO: Implement SimbaPluginTarget_KeySend\r\n");
}

#[no_mangle]
pub extern "C" fn SimbaPluginTarget_KeyPressed(target: *mut SimbaTarget, key: c_int) -> bool {
    println!("TODO: Implement SimbaPluginTarget_KeyPressed\r\n");
    false
}
