//Client sided code. Everything on this file is ran on client.

use gl::{types::GLuint, CURRENT_PROGRAM, VERTEX_ARRAY_BINDING};
use lazy_static::lazy_static;
use retour::GenericDetour;
use std::{
    ffi::c_void,
    mem::transmute,
    ptr::null_mut,
    sync::{Mutex, OnceLock},
};

use std::char::from_u32;

use windows::{
    core::{BOOL, PCSTR},
    Win32::{
        Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM},
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
                CallWindowProcW, IsWindowVisible, SetWindowLongPtrW, ShowWindow, GWLP_WNDPROC,
                SW_HIDE, SW_SHOWNORMAL, WM_CHAR, WM_IME_NOTIFY, WM_IME_SETCONTEXT, WM_KEYDOWN,
                WM_KEYUP, WM_KILLFOCUS, WM_MOUSEMOVE, WNDPROC,
            },
        },
    },
};

use crate::{
    graphics::{
        draw_point, generate_pixel_buffers, load_opengl_extensions, read_pixel_buffers,
        restore_state,
    },
    memory::MEMORY_MANAGER,
    windows::{WI_CONSOLE, WI_MODIFIERS},
};

lazy_static! {
    static ref KEYBOARD_MODIFIERS: Mutex<(bool, bool, bool)> = Mutex::new((false, false, false));
    static ref LAST_CHAR: Mutex<i32> = Mutex::new(0);
}

pub unsafe extern "system" fn start_thread(lparam: *mut c_void) -> u32 {
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
static mut ORIGINAL_WNDPROC: Option<WNDPROC> = None;

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

unsafe fn hooked_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let original = ORIGINAL_WNDPROC.expect("[WaspInput]: ORIGINAL_WNDPROC is not set!\r\n");

    match msg {
        WI_CONSOLE => {
            open_client_console();
            return LRESULT(0);
        }
        WI_MODIFIERS => {
            let mut modifiers = KEYBOARD_MODIFIERS.lock().unwrap();
            let (shift, ctrl, alt) = &mut *modifiers;
            let (nshift, nctrl, nalt) = decode_modifiers(wparam);

            if nshift {
                let msg = if *shift { WM_KEYUP } else { WM_KEYDOWN };
                let lparam = get_modifier_lparam(0x10, !*shift);
                let _ = CallWindowProcW(original, hwnd, msg, WPARAM(0x10), lparam);
            }

            if nctrl {
                let msg = if *ctrl { WM_KEYUP } else { WM_KEYDOWN };
                let lparam = get_modifier_lparam(0x11, !*ctrl);
                let _ = CallWindowProcW(original, hwnd, msg, WPARAM(0x11), lparam);
            }

            if nalt {
                let msg = if *alt { WM_KEYUP } else { WM_KEYDOWN };
                let lparam = get_modifier_lparam(0x12, !*alt);
                let _ = CallWindowProcW(original, hwnd, msg, WPARAM(0x12), lparam);
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

                return CallWindowProcW(original, hwnd, msg, new_wparam, lparam);
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

    CallWindowProcW(original, hwnd, msg, wparam, lparam)
}

unsafe fn hook_wndproc(hwnd: u64) {
    let hwnd = HWND(hwnd as *mut c_void);

    if ORIGINAL_WNDPROC.is_some() {
        println!("[WaspInput]: WndProc already hooked.\r\n");
        return;
    }

    let previous = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, hooked_wndproc as isize);
    if previous == 0 {
        panic!(
            "[WaspInput]: Failed to set new WndProc: {:?}\r\n",
            GetLastError()
        );
    }

    ORIGINAL_WNDPROC = Some(transmute(previous));

    println!("[WaspInput]: WndProc successfully hooked.\r\n");
}

pub unsafe fn unhook_wndproc(hwnd: u64) {
    let original = ORIGINAL_WNDPROC.expect("[WaspInput]: ORIGINAL_WNDPROC is not set!\r\n");
    let hwnd = HWND(hwnd as isize as *mut c_void);

    if let Some(original) = original {
        let result = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, original as isize);
        if result == 0 {
            panic!("[WaspInput]: Failed to restore original WndProc.\r\n");
        }
        ORIGINAL_WNDPROC = None;
        println!("[WaspInput]: WndProc successfully restored.\r\n");
        return;
    }

    println!("[WaspInput]: No original WndProc stored.\r\n");
}

//OpenGL Hook
static ORIGINAL_WGL_SWAPBUFFERS: OnceLock<GenericDetour<unsafe extern "system" fn(HDC) -> BOOL>> =
    OnceLock::new();

lazy_static! {
    static ref PBO: Mutex<[GLuint; 2]> = Mutex::new([0; 2]);
}

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

    mem_manager.set_dimensions(width, height);

    if load_opengl_extensions() {
        let dest = mem_manager.image_ptr();
        if !dest.is_null() {
            let mut pbo = PBO.lock().unwrap();
            generate_pixel_buffers(&mut *pbo, width, height, 4);
            read_pixel_buffers(dest as *mut u8, &mut *pbo, width, height);
        }

        if (mouse.0 > -1) && (mouse.1 > -1) && (mouse.0 < width) && (mouse.1 < height) {
            draw_point(mouse.0, mouse.1, width, height);
        }
    }

    restore_state(prev_program, prev_vao);

    let detour = ORIGINAL_WGL_SWAPBUFFERS.get().unwrap();
    detour.call(hdc)
}

unsafe fn hook_wgl_swap_buffers() {
    let module = GetModuleHandleA(PCSTR(b"opengl32.dll\0".as_ptr())).unwrap();
    let addr = GetProcAddress(module, PCSTR(b"wglSwapBuffers\0".as_ptr()))
        .expect("[WaspInput]: wglSwapBuffers not found\r\n");

    let original_fn: unsafe extern "system" fn(HDC) -> BOOL = std::mem::transmute(addr);
    let detour = GenericDetour::new(original_fn, hooked_wgl_swap_buffers)
        .expect("[WaspInput]: Failed to create wglSwapBuffers hook\r\n");

    detour
        .enable()
        .expect("[WaspInput]: Failed to enable wglSwapBuffers hook\r\n");

    ORIGINAL_WGL_SWAPBUFFERS.set(detour).unwrap();
    println!("[WaspInput]: wglSwapBuffers successfully hooked.\r\n");
}
