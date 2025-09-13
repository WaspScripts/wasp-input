//Target related methods for Simba 2.0
use lazy_static::lazy_static;
use std::{
    collections::HashMap,
    ffi::CStr,
    os::raw::{c_char, c_int, c_void},
    ptr::null_mut,
    sync::Mutex,
};

use windows::Win32::Foundation::POINT;

use crate::shared::{
    memory::{MemoryManager, MEMORY_MANAGER},
    sync::call_event,
    windows::{
        get_jagrenderview, get_mouse_position, key_down, key_up, keys_send, lbutton, mbutton,
        mouse_move, rbutton, scroll,
    },
};

use super::plugin::PLUGIN_SIMBA_METHODS;

#[repr(C)]
pub struct SimbaTarget {
    pub pid: u32,
    pub hwnd: u64,
    pub keyboard: [bool; 255],
    pub mouse: [bool; 3],
}

lazy_static! {
    pub static ref TARGETS: Mutex<HashMap<u32, SimbaTarget>> = Mutex::new(HashMap::new());
}

pub fn get_mouse_pos(hwnd: u64) -> POINT {
    let mem_manager = MEMORY_MANAGER
        .get()
        .expect("[WaspInput]: Memory manager is not initialized!\r\n")
        .lock()
        .unwrap();
    let (x, y) = unsafe { mem_manager.get_mouse_position() };

    if (x == -1) | (y == -1) {
        match get_mouse_position(hwnd) {
            Some(pt) => unsafe { mem_manager.set_mouse_position(pt.x, pt.y) },
            None => println!("[WaspInput]: Failed to get mouse position!\r\n"),
        };
    }
    POINT { x: x, y: y }
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_Request(args: *const c_char) -> *mut SimbaTarget {
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

    let hwnd = get_jagrenderview(pid).expect("[WaspInput]: Failed to find JagRenderView HWND.\r\n");
    call_event(hwnd.0 as u64);

    let _ = MEMORY_MANAGER.set(Mutex::new(unsafe { MemoryManager::open_map(5000) }));
    let mem_manager = MEMORY_MANAGER
        .get()
        .expect("[WaspInput]: Memory manager is not initialized!\r\n")
        .lock()
        .unwrap();

    if !unsafe { mem_manager.is_mapped() } {}

    let mut targets = TARGETS.lock().unwrap();

    if let Some(target) = targets.get_mut(&pid) {
        return target as *mut SimbaTarget;
    } else {
        panic!("[WaspInput]: The specified target hasn't been injected.\r\n");
    }
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_RequestWithDebugImage(
    args: *const c_char,
    overlay: *mut *mut c_void,
) -> *mut SimbaTarget {
    let target = SimbaPluginTarget_Request(args);

    if target.is_null() {
        return null_mut();
    }

    if !overlay.is_null() {
        let mem_manager = MEMORY_MANAGER
            .get()
            .expect("[WaspInput]: Memory manager is not initialized!\r\n")
            .lock()
            .unwrap();

        unsafe {
            let external_image_create = PLUGIN_SIMBA_METHODS
                .external_image_create
                .expect("external_image_create function pointer is null");

            let external_image_set_memory = PLUGIN_SIMBA_METHODS
                .external_image_set_memory
                .expect("external_image_set_memory function pointer is null");

            let (w, h) = mem_manager.get_dimensions();

            let img = external_image_create(true);
            *overlay = img;
            external_image_set_memory(*overlay, mem_manager.overlay_ptr() as *mut c_void, w, h);
        }
    }

    target
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_Release(target: *mut SimbaTarget) {
    if target.is_null() {
        return;
    }

    let mut targets = TARGETS.lock().unwrap();
    let target = unsafe { &*target };

    if let Some(removed) = targets.remove(&target.pid) {
        println!(
            "Releasing Client PID: {} and HWND: {}\r\n",
            removed.pid, removed.hwnd
        );
    }
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_GetDimensions(
    target: *mut SimbaTarget,
    width: *mut c_int,
    height: *mut c_int,
) {
    if target.is_null() || width.is_null() || height.is_null() {
        return;
    }

    let mem_manager = MEMORY_MANAGER
        .get()
        .expect("[WaspInput]: Memory manager is not initialized!\r\n")
        .lock()
        .unwrap();
    let (w, h) = unsafe { mem_manager.get_dimensions() };

    unsafe {
        *width = w;
        *height = h;
    }
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_GetImageData(
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

    let mem_manager = MEMORY_MANAGER
        .get()
        .expect("[WaspInput]: Memory manager is not initialized!\r\n")
        .lock()
        .unwrap();

    let (w, _h) = unsafe { mem_manager.get_dimensions() };
    unsafe { *data_width = w };

    let img_data = unsafe { mem_manager.image_ptr() };
    let offset = ((y * (w) + x) * 4) as isize;
    unsafe { *bgra = img_data.offset(offset) as *mut c_void };

    true
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_MousePressed(
    target: *mut SimbaTarget,
    mouse_button: c_int,
) -> bool {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return false;
    }

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &*target };
    match mouse_button {
        1 => target.keyboard[0],
        2 | 4 | 5 => target.keyboard[1],
        3 => target.keyboard[2],
        _ => {
            println!("[WaspInput]: Unknown mouse button: {}\r\n", mouse_button);
            false
        }
    }
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_MousePosition(
    target: *mut SimbaTarget,
    x: *mut i32,
    y: *mut i32,
) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &*target };

    let pt = get_mouse_pos(target.hwnd);

    unsafe {
        *x = pt.x;
        *y = pt.y;
    };
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_MouseTeleport(
    target: *mut SimbaTarget,
    x: c_int,
    y: c_int,
) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &*target };

    mouse_move(target.hwnd, x, y);
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_MouseUp(target: *mut SimbaTarget, mouse_button: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &mut *target };

    let pt = get_mouse_pos(target.hwnd);
    match mouse_button {
        1 => {
            lbutton(target.hwnd, false, pt.x, pt.y);
            target.mouse[0] = false;
        }
        2 | 4 | 5 => {
            mbutton(target.hwnd, false, pt.x, pt.y);
            target.mouse[1] = false;
        }
        3 => {
            rbutton(target.hwnd, false, pt.x, pt.y);
            target.mouse[2] = false;
        }
        _ => {
            println!("[WaspInput]: Unknown mouse button: {}\r\n", mouse_button);
            return;
        }
    };
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_MouseDown(target: *mut SimbaTarget, mouse_button: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &mut *target };

    let pt = get_mouse_pos(target.hwnd);

    match mouse_button {
        1 => {
            lbutton(target.hwnd, true, pt.x, pt.y);
            target.mouse[0] = true;
        }
        2 | 4 | 5 => {
            mbutton(target.hwnd, true, pt.x, pt.y);
            target.mouse[1] = true;
        }
        3 => {
            rbutton(target.hwnd, true, pt.x, pt.y);
            target.mouse[2] = true;
        }
        _ => {
            println!("[WaspInput]: Unknown mouse button: {}\r\n", mouse_button);
            return;
        }
    };
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_MouseScroll(target: *mut SimbaTarget, scrolls: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &mut *target };

    let pt = get_mouse_pos(target.hwnd);
    scroll(target.hwnd, true, scrolls, pt.x, pt.y);
    println!("[WaspInput]: TODO: Implement SimbaPluginTarget_MouseScroll\r\n");
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_KeyDown(target: *mut SimbaTarget, key: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &mut *target };

    key_down(target.hwnd, key);
    target.keyboard[key as usize] = true;
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_KeyUp(target: *mut SimbaTarget, key: c_int) {
    if target.is_null() {
        println!("[WaspInput]: target is null!\r\n");
        return;
    }

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &mut *target };

    key_up(target.hwnd, key);
    target.keyboard[key as usize] = false;
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_KeySend(
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

    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &*target };
    keys_send(target.hwnd, text, len, sleeptimes);
}

#[no_mangle]
pub extern "system" fn SimbaPluginTarget_KeyPressed(target: *mut SimbaTarget, key: c_int) -> bool {
    let _lock = TARGETS.lock().unwrap();
    let target = unsafe { &*target };

    target.keyboard[key as usize]
}
