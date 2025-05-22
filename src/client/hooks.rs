use gl::{CURRENT_PROGRAM, VERTEX_ARRAY_BINDING};
use lazy_static::lazy_static;
use retour::GenericDetour;
use std::{
    ffi::c_void,
    ptr::null_mut,
    sync::{Mutex, OnceLock},
};

use std::char::from_u32;

use windows::{
    core::{BOOL, PCSTR},
    Win32::{
        Foundation::{CloseHandle, GetLastError, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::{
            Gdi::HDC,
            OpenGL::{glGetIntegerv, GL_VIEWPORT},
        },
        System::{
            Console::{AllocConsole, AttachConsole, GetConsoleWindow, ATTACH_PARENT_PROCESS},
            LibraryLoader::{GetModuleHandleA, GetProcAddress},
        },
        UI::{
            Input::KeyboardAndMouse::{
                GetKeyboardState, MapVirtualKeyA, MapVirtualKeyW, ToUnicode, MAPVK_VK_TO_VSC,
            },
            WindowsAndMessaging::{
                GetWindowLongPtrW, IsWindowVisible, ShowWindow, GWLP_WNDPROC, SW_HIDE,
                SW_SHOWNORMAL, WM_CHAR, WM_IME_NOTIFY, WM_IME_SETCONTEXT, WM_KEYDOWN, WM_KEYUP,
                WM_KILLFOCUS, WM_MOUSEMOVE,
            },
        },
    },
};

use super::graphics::{
    draw_overlay, draw_point, load_opengl_extensions, read_frame, restore_state,
};
use crate::shared::{
    memory::{MemoryManager, MEMORY_MANAGER},
    windows::{unload_self_dll, WI_CONSOLE, WI_DETACH, WI_MODIFIERS, WI_REMAP},
};

lazy_static! {
    static ref KEYBOARD_MODIFIERS: Mutex<(bool, bool, bool)> = Mutex::new((false, false, false));
    static ref LAST_CHAR: Mutex<i32> = Mutex::new(0);
}

pub unsafe extern "system" fn start_thread(lparam: *mut c_void) -> u32 {
    open_client_console();
    hook_wndproc(lparam as u64);
    hook_wgl_swap_buffers();
    0
}

pub unsafe fn open_client_console() {
    let hwnd = GetConsoleWindow();
    if hwnd.0 != null_mut() {
        if IsWindowVisible(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_HIDE);
            return;
        }

        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
    }
    if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
        let _ = AllocConsole();
    }
}

//WndProc hook
static ORIGINAL_WNDPROC: OnceLock<
    GenericDetour<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>,
> = OnceLock::new();

fn get_modifier_lparam(key: i32, down: bool) -> LPARAM {
    let scancode = unsafe { MapVirtualKeyA(key as u32, MAPVK_VK_TO_VSC) };

    let mut lparam = 1 | (scancode << 16) | (0 << 24);
    if !down {
        lparam |= (1 << 30) | (1 << 31);
    }
    LPARAM(lparam as isize)
}

fn decode_modifiers(wparam: WPARAM) -> (bool, bool, bool) {
    let value = wparam.0;
    let shift = (value & (1 << 0)) != 0;
    let ctrl = (value & (1 << 1)) != 0;
    let alt = (value & (1 << 2)) != 0;
    (shift, ctrl, alt)
}

unsafe fn get_updated_wparam(vkey: u32, shift: bool, ctrl: bool, alt: bool) -> WPARAM {
    let scancode = MapVirtualKeyW(vkey, MAPVK_VK_TO_VSC);

    let mut keyboard_state = [0u8; 256];
    let _ = GetKeyboardState(&mut keyboard_state);

    keyboard_state[vkey as usize] = 0x80;

    if shift {
        keyboard_state[0x10 as usize] = 0x80;
    }

    if ctrl {
        keyboard_state[0x11 as usize] = 0x80;
    }

    if alt {
        keyboard_state[0x12 as usize] = 0x80;
    }

    let mut buff = [0u16; 4];
    let _ = ToUnicode(vkey, scancode, Some(&keyboard_state), &mut buff, 0);

    match from_u32(buff[0] as u32) {
        Some(unicode) => WPARAM(unicode as usize),
        None => WPARAM(0),
    }
}

fn rebuild_key(key: u8, shift: bool, ctrl: bool, alt: bool) -> u16 {
    let mut modifiers: u8 = 0;
    if shift {
        modifiers |= 0x01;
    }
    if ctrl {
        modifiers |= 0x02;
    }
    if alt {
        modifiers |= 0x04;
    }

    ((modifiers as u16) << 8) | (key as u16)
}

unsafe extern "system" fn hooked_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let original = ORIGINAL_WNDPROC.get().unwrap();

    match msg {
        WI_CONSOLE => {
            open_client_console();
            return LRESULT(0);
        }
        WI_REMAP => {
            println!("Restarting map.\r\n");
            let mut mem_manager = MEMORY_MANAGER.lock().unwrap();
            if mem_manager.is_mapped() {
                mem_manager.close_map();
            }
            *mem_manager = MemoryManager::open_map();
            return LRESULT(0);
        }
        WI_MODIFIERS => {
            let mut modifiers = KEYBOARD_MODIFIERS.lock().unwrap();
            let (shift, ctrl, alt) = &mut *modifiers;
            let (nshift, nctrl, nalt) = decode_modifiers(wparam);

            if nshift {
                let msg = if *shift { WM_KEYUP } else { WM_KEYDOWN };
                let lparam = get_modifier_lparam(0x10, !*shift);
                let _ = original.call(hwnd, msg, WPARAM(0x10), lparam);
            }

            if nctrl {
                let msg = if *ctrl { WM_KEYUP } else { WM_KEYDOWN };
                let lparam = get_modifier_lparam(0x11, !*ctrl);
                let _ = original.call(hwnd, msg, WPARAM(0x11), lparam);
            }

            if nalt {
                let msg = if *alt { WM_KEYUP } else { WM_KEYDOWN };
                let lparam = get_modifier_lparam(0x12, !*alt);
                let _ = original.call(hwnd, msg, WPARAM(0x12), lparam);
            }

            if nshift {
                *shift = !*shift;
            }
            if nctrl {
                *ctrl = !*ctrl;
            }
            if nalt {
                *alt = !*alt;
            }

            return LRESULT(0);
        }
        WI_DETACH => {
            let mut mem_manager = MEMORY_MANAGER.lock().unwrap();
            unsafe {
                unhook_wgl_swap_buffers();
                if mem_manager.is_mapped() {
                    mem_manager.close_map();
                }
                unhook_wndproc();
            };
            //unload_self_dll();
            return LRESULT(0);
        }
        WM_KEYDOWN => {
            let mut modifiers = KEYBOARD_MODIFIERS.lock().unwrap();
            let (shift, ctrl, alt) = &mut *modifiers;
            if *shift | *ctrl | *alt {
                *LAST_CHAR.lock().unwrap() = wparam.0 as i32;
            }
            WM_KEYDOWN
        }
        WM_CHAR => {
            let mut modifiers = KEYBOARD_MODIFIERS.lock().unwrap();
            let (shift, ctrl, alt) = &mut *modifiers;
            if *shift | *ctrl | *alt {
                let vkey = LAST_CHAR.lock().unwrap();
                let key = rebuild_key(*vkey as u8, *shift, *ctrl, *alt);
                let new_wparam = get_updated_wparam((key & 0xFF) as u32, *shift, *ctrl, *alt);

                //
                println!(
                    "[WaspInput]: ch: {:?}: key: {:?}, i16Key: {:?}\r\n",
                    wparam,
                    key,
                    key & 0xFF
                );
                println!(
                    "[WaspInput]: updated message: WM_CHAR, wparam: {:?}, lparam: {:?}\r\n",
                    new_wparam, lparam
                );

                return original.call(hwnd, msg, new_wparam, lparam);
            }

            WM_CHAR
        }
        WM_KILLFOCUS => return LRESULT(0),
        WM_IME_SETCONTEXT => return LRESULT(0),
        WM_IME_NOTIFY => return LRESULT(0),
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as u16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i32;

            let mem_manager = MEMORY_MANAGER.lock().unwrap();
            mem_manager.set_mouse_position(x, y);

            WM_MOUSEMOVE
        }
        _ => msg,
    };

    original.call(hwnd, msg, wparam, lparam)
}

unsafe fn hook_wndproc(hwnd: u64) {
    let original_proc = GetWindowLongPtrW(HWND(hwnd as *mut c_void), GWLP_WNDPROC) as *const ();
    if original_proc.is_null() {
        panic!("Failed to get WndProc: {:?}", GetLastError());
    }

    let detour =
        GenericDetour::<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>::new(
            std::mem::transmute(original_proc),
            hooked_wndproc,
        )
        .expect("[WaspInput]: Failed to create WndProc hook.\r\n");

    detour
        .enable()
        .expect("[WaspInput]: Failed to enable WndProc hook.\r\n");

    ORIGINAL_WNDPROC
        .set(detour)
        .expect("[WaspInput]: Failed to save original WndProc function.\r\n");

    println!("[WaspInput]: WndProc successfully hooked.\r\n");
}

pub unsafe fn unhook_wndproc() {
    let detour = ORIGINAL_WNDPROC
        .get()
        .expect("[WaspInput]: WndProc hook not found.\r\n");

    detour
        .disable()
        .expect("[WaspInput]: Failed to disable WndProc hook\r\n");

    println!("[WaspInput]: WndProc successfully unhooked.\r\n");
}

//OpenGL Hook
static ORIGINAL_WGL_SWAPBUFFERS: OnceLock<GenericDetour<unsafe extern "system" fn(HDC) -> BOOL>> =
    OnceLock::new();

unsafe extern "system" fn hooked_wgl_swap_buffers(hdc: HDC) -> BOOL {
    let mem_manager = MEMORY_MANAGER.lock().unwrap();
    let mut viewport = [0, 0, 0, 0];
    let mouse = mem_manager.get_mouse_position();

    glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr());
    // Save current state
    let mut prev_program: i32 = 0;
    let mut prev_vao: i32 = 0;
    glGetIntegerv(CURRENT_PROGRAM, &mut prev_program);
    glGetIntegerv(VERTEX_ARRAY_BINDING, &mut prev_vao);

    let width = viewport[2];
    let height = viewport[3];
    let frame_size = width * height * 4;

    mem_manager.set_dimensions(width, height);

    if load_opengl_extensions() {
        let dest = mem_manager.image_ptr();
        read_frame(width, height, frame_size, dest);

        let overlay = mem_manager.overlay_ptr();
        draw_overlay(width, height, overlay);

        if (mouse.0 > -1) && (mouse.1 > -1) && (mouse.0 < width) && (mouse.1 < height) {
            draw_point(mouse.0, mouse.1, width, height);
        }
    }

    restore_state(prev_program, prev_vao);

    let original = ORIGINAL_WGL_SWAPBUFFERS.get().unwrap();
    original.call(hdc)
}

unsafe fn hook_wgl_swap_buffers() {
    let module = GetModuleHandleA(PCSTR(b"opengl32.dll\0".as_ptr()))
        .expect("[WaspInput]: opengl32.dll module not found.\r\n");

    let addr = GetProcAddress(module, PCSTR(b"wglSwapBuffers\0".as_ptr()))
        .expect("[WaspInput]: wglSwapBuffers function not found.\r\n");

    let original_fn: unsafe extern "system" fn(HDC) -> BOOL = std::mem::transmute(addr);
    let detour = GenericDetour::new(original_fn, hooked_wgl_swap_buffers)
        .expect("[WaspInput]: Failed to create wglSwapBuffers hook.\r\n");

    detour
        .enable()
        .expect("[WaspInput]: Failed to enable wglSwapBuffers hook.\r\n");

    ORIGINAL_WGL_SWAPBUFFERS
        .set(detour)
        .expect("[WaspInput]: Failed to save original wglSwapBuffers function.\r\n");
    println!("[WaspInput]: wglSwapBuffers successfully hooked.\r\n");
}

pub unsafe fn unhook_wgl_swap_buffers() {
    let detour = ORIGINAL_WGL_SWAPBUFFERS
        .get()
        .expect("[WaspInput]: wglSwapBuffers hook not found\r\n");

    detour
        .disable()
        .expect("[WaspInput]: Failed to disable wglSwapBuffers hook\r\n");

    println!("[WaspInput]: wglSwapBuffers successfully unhooked.\r\n");
}
