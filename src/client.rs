use gl::{
    types::{GLboolean, GLenum, GLint, GLsizei, GLsizeiptr, GLuint, GLvoid},
    TexImage2D, BGRA, CLAMP_TO_EDGE, NEAREST, RGBA, TEXTURE_MAG_FILTER, TEXTURE_MIN_FILTER,
    TEXTURE_RECTANGLE, TEXTURE_WRAP_S, TEXTURE_WRAP_T, UNPACK_ROW_LENGTH, UNSIGNED_BYTE,
};
//Client sided code. Everything on this file is ran on client.
use lazy_static::lazy_static;
use retour::GenericDetour;
use std::{
    collections::HashMap,
    ffi::{c_void, CString},
    mem::transmute,
    ptr::{addr_of, null_mut},
    sync::{Mutex, OnceLock},
};
use windows::{
    core::{BOOL, PCSTR},
    Win32::{
        Graphics::{
            Gdi::{WindowFromDC, HDC},
            OpenGL::{
                glBegin, glBindTexture, glColor4ub, glDeleteTextures, glDisable, glEnable, glEnd,
                glGenTextures, glGetIntegerv, glLoadIdentity, glMatrixMode, glOrtho, glPixelStorei,
                glPopAttrib, glPopMatrix, glPushAttrib, glPushMatrix, glTexCoord2f,
                glTexParameteri, glTexSubImage2D, glVertex2f, glViewport, wglCreateContext,
                wglGetCurrentContext, wglGetProcAddress, wglMakeCurrent, GetPixelFormat,
                GL_ALL_ATTRIB_BITS, GL_DEPTH_TEST, GL_MODELVIEW, GL_PROJECTION, GL_VIEWPORT, HGLRC,
            },
        },
        System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
    },
};

use std::char::from_u32;

use windows::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM},
    System::Console::{AllocConsole, AttachConsole, GetConsoleWindow, ATTACH_PARENT_PROCESS},
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyboardState, MapVirtualKeyA, MapVirtualKeyW, ToUnicode, MAPVK_VK_TO_VSC,
        },
        WindowsAndMessaging::{
            CallWindowProcW, IsWindowVisible, SetWindowLongPtrW, ShowWindow, GWLP_WNDPROC, SW_HIDE,
            SW_SHOWNORMAL, WM_CHAR, WM_IME_NOTIFY, WM_IME_SETCONTEXT, WM_KEYDOWN, WM_KEYUP,
            WM_KILLFOCUS, WNDPROC,
        },
    },
};

use crate::{
    memory::get_debug_image,
    target::get_mouse_pos,
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

static mut GL_GEN_BUFFERS: Option<unsafe extern "system" fn(n: GLsizei, buffers: *mut GLuint)> =
    None;
static mut GL_DELETE_BUFFERS: Option<
    unsafe extern "system" fn(n: GLsizei, buffers: *const GLuint),
> = None;
static mut GL_BIND_BUFFER: Option<unsafe extern "system" fn(target: GLenum, buffer: GLuint)> = None;
static mut GL_BUFFER_DATA: Option<
    unsafe extern "system" fn(target: GLenum, size: GLsizeiptr, data: *const GLvoid, usage: GLenum),
> = None;
static mut GL_MAP_BUFFER: Option<
    unsafe extern "system" fn(target: GLenum, access: GLenum) -> *mut c_void,
> = None;
static mut GL_UNMAP_BUFFER: Option<unsafe extern "system" fn(target: GLenum) -> GLboolean> = None;

unsafe fn load_opengl_extensions() -> bool {
    if GL_GEN_BUFFERS.is_none() {
        let load_fn = |name: &str| -> *const std::ffi::c_void {
            let cname = CString::new(name).unwrap();
            let pcstr = PCSTR(cname.as_ptr() as *const u8);
            if let Some(ptr) = wglGetProcAddress(pcstr) {
                ptr as *const std::ffi::c_void
            } else {
                std::ptr::null()
            }
        };

        GL_GEN_BUFFERS = {
            let ptr = load_fn("glGenBuffers");
            if ptr.is_null() {
                None
            } else {
                Some(std::mem::transmute(ptr))
            }
        };

        GL_DELETE_BUFFERS = {
            let ptr = load_fn("glDeleteBuffers");
            if ptr.is_null() {
                None
            } else {
                Some(std::mem::transmute(ptr))
            }
        };

        GL_BIND_BUFFER = {
            let ptr = load_fn("glBindBuffer");
            if ptr.is_null() {
                None
            } else {
                Some(std::mem::transmute(ptr))
            }
        };

        GL_BUFFER_DATA = {
            let ptr = load_fn("glBufferData");
            if ptr.is_null() {
                None
            } else {
                Some(std::mem::transmute(ptr))
            }
        };

        GL_MAP_BUFFER = {
            let ptr = load_fn("glMapBuffer");
            if ptr.is_null() {
                None
            } else {
                Some(std::mem::transmute(ptr))
            }
        };

        GL_UNMAP_BUFFER = {
            let ptr = load_fn("glUnmapBuffer");
            if ptr.is_null() {
                None
            } else {
                Some(std::mem::transmute(ptr))
            }
        };
    }

    GL_GEN_BUFFERS.is_some()
        && GL_DELETE_BUFFERS.is_some()
        && GL_BIND_BUFFER.is_some()
        && GL_BUFFER_DATA.is_some()
        && GL_MAP_BUFFER.is_some()
        && GL_UNMAP_BUFFER.is_some()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    BgrBgra,
    BGRA,
}

#[repr(C)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

unsafe fn convert(source: *mut Pixel, size: usize, format: ImageFormat) {
    if let ImageFormat::BgrBgra = format {
        for i in 0..size {
            let pixel = source.add(i);
            let value = *(pixel as *const u32);
            (*pixel).a = if value == 0x00 { 0x00 } else { 0xFF };
        }
    }
}

static mut TEXTURE_ID: GLuint = 0;
static mut TEXTURE_WIDTH: i32 = 0;
static mut TEXTURE_HEIGHT: i32 = 0;

pub unsafe fn gl_draw_image(
    _ctx: *mut c_void,
    source_buffer: *mut c_void,
    x: f32,
    y: f32,
    width: i32,
    height: i32,
    _stride: i32,
    format: ImageFormat,
) {
    let gl_format = BGRA;
    println!("HERE0\r\n");

    let size = (width * height) as usize;
    println!("HERE0,1\r\n");
    convert(source_buffer as *mut Pixel, size, format);
    println!("HERE1\r\n");

    let target = TEXTURE_RECTANGLE;
    println!("HERE2\r\n");
    if TEXTURE_ID == 0 || TEXTURE_WIDTH != width || TEXTURE_HEIGHT != height {
        if TEXTURE_ID != 0 {
            glDeleteTextures(1, addr_of!(TEXTURE_ID));
        }

        let mut tex_id = 0;
        glGenTextures(1, &mut tex_id);
        glBindTexture(target, tex_id);

        glPixelStorei(UNPACK_ROW_LENGTH, width);
        TexImage2D(
            target,
            0,
            RGBA as GLint,
            width,
            height,
            0,
            gl_format,
            UNSIGNED_BYTE,
            source_buffer,
        );
        glPixelStorei(UNPACK_ROW_LENGTH, 0);

        glTexParameteri(target, TEXTURE_WRAP_S, CLAMP_TO_EDGE as i32);
        glTexParameteri(target, TEXTURE_WRAP_T, CLAMP_TO_EDGE as i32);
        glTexParameteri(target, TEXTURE_MIN_FILTER, NEAREST as i32);
        glTexParameteri(target, TEXTURE_MAG_FILTER, NEAREST as i32);

        TEXTURE_ID = tex_id;
        TEXTURE_WIDTH = width;
        TEXTURE_HEIGHT = height;
    } else {
        glBindTexture(target, TEXTURE_ID);
        glPixelStorei(UNPACK_ROW_LENGTH, width);
        glTexSubImage2D(
            target,
            0,
            0,
            0,
            width,
            height,
            gl_format,
            UNSIGNED_BYTE,
            source_buffer,
        );
        glPixelStorei(UNPACK_ROW_LENGTH, 0);
        glBindTexture(target, 0);
    }
    println!("HERE3\r\n");
    let (x1, y1, x2, y2) = (x, y, x + width as f32, y + height as f32);
    println!("HERE4\r\n");
    glEnable(target);
    glBindTexture(target, TEXTURE_ID);
    glColor4ub(0xFF, 0xFF, 0xFF, 0xFF);
    println!("HERE5\r\n");
    glBegin(gl::QUADS);
    glTexCoord2f(0.0, height as f32);
    glVertex2f(x1, y1);
    glTexCoord2f(0.0, 0.0);
    glVertex2f(x1, y2);
    glTexCoord2f(width as f32, 0.0);
    glVertex2f(x2, y2);
    glTexCoord2f(width as f32, height as f32);
    glVertex2f(x2, y1);
    glEnd();
    println!("HERE6\r\n");
    glBindTexture(target, 0);
    glDisable(target);
    println!("HERE7\r\n");
}

#[derive(Copy, Clone)]
struct SafeHGLRC(HGLRC);

unsafe impl Send for SafeHGLRC {}
unsafe impl Sync for SafeHGLRC {}

lazy_static! {
    static ref CONTEXTS: Mutex<HashMap<i32, SafeHGLRC>> = Mutex::new(HashMap::new());
}

unsafe fn push_gl_context(hdc: HDC, width: i32, height: i32) {
    let pixelformat = GetPixelFormat(hdc);

    let mut contexts = CONTEXTS.lock().unwrap();
    if !contexts.contains_key(&pixelformat) {
        let ctx = wglCreateContext(hdc).expect("Failed to create OpenGL context");
        contexts.insert(pixelformat, SafeHGLRC(ctx));
    }

    let SafeHGLRC(ctx) = *contexts.get(&pixelformat).unwrap();
    let _ = wglMakeCurrent(hdc, ctx);

    glPushAttrib(GL_ALL_ATTRIB_BITS);

    glPushMatrix();
    glViewport(0, 0, width, height);
    glMatrixMode(GL_PROJECTION);
    glLoadIdentity();
    glOrtho(0.0, width as f64, 0.0, height as f64, -1.0, 1.0);
    glMatrixMode(GL_MODELVIEW);
    glLoadIdentity();
    glDisable(GL_DEPTH_TEST);
}

unsafe fn pop_gl_context(hdc: HDC, ctx: HGLRC) {
    glPopMatrix();
    glPopAttrib();
    let _ = wglMakeCurrent(hdc, ctx);
}

unsafe extern "system" fn hooked_wgl_swap_buffers(hdc: HDC) -> BOOL {
    let hwnd = WindowFromDC(hdc);
    let mouse = get_mouse_pos(hwnd.0 as u64);
    let _ = mouse;

    let mut viewport = [0, 0, 0, 0];

    glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr());

    let width = viewport[2];
    let height = viewport[3];
    //println!("Viewport: width = {}, height = {}", width, height);

    if load_opengl_extensions() {
        let old_ctx = wglGetCurrentContext();
        push_gl_context(hdc, width, height);

        let src = get_debug_image(width as usize, height as usize);
        if !src.is_null() {
            gl_draw_image(
                hdc.0 as *mut c_void,
                src as *mut c_void,
                0.0,
                0.0,
                width as i32,
                height as i32,
                4,
                ImageFormat::BgrBgra,
            );
        }

        //println!("MOUSE: {:?}\r\n", mouse);

        pop_gl_context(hdc, old_ctx);
    }

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
