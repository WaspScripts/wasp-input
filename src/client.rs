use gl::{
    types::{GLboolean, GLenum, GLint, GLsizei, GLsizeiptr, GLubyte, GLuint, GLvoid},
    BGRA, PIXEL_PACK_BUFFER, STREAM_READ, UNPACK_ROW_LENGTH, UNSIGNED_BYTE,
};
//Client sided code. Everything on this file is ran on client.
use lazy_static::lazy_static;
use retour::GenericDetour;
use std::{
    collections::HashMap,
    ffi::{c_void, CString},
    mem::transmute,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicI32, AtomicUsize, Ordering},
        Mutex, OnceLock,
    },
};

use std::char::from_u32;

use windows::{
    core::{BOOL, PCSTR},
    Win32::{
        Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::{
            Gdi::{WindowFromDC, HDC},
            OpenGL::{
                glDisable, glGetIntegerv, glLoadIdentity, glMatrixMode, glOrtho, glPixelStorei,
                glPopAttrib, glPopMatrix, glPushAttrib, glPushMatrix, glReadPixels, glViewport,
                wglCreateContext, wglGetCurrentContext, wglGetProcAddress, wglMakeCurrent,
                GetPixelFormat, GL_ALL_ATTRIB_BITS, GL_DEPTH_TEST, GL_MODELVIEW, GL_PROJECTION,
                GL_VIEWPORT, HGLRC,
            },
        },
        System::Console::{AllocConsole, AttachConsole, GetConsoleWindow, ATTACH_PARENT_PROCESS},
        System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
        UI::{
            Input::KeyboardAndMouse::{
                GetKeyboardState, MapVirtualKeyA, MapVirtualKeyW, ToUnicode, MAPVK_VK_TO_VSC,
            },
            WindowsAndMessaging::{
                CallWindowProcW, IsWindowVisible, SetWindowLongPtrW, ShowWindow, GWLP_WNDPROC,
                SW_HIDE, SW_SHOWNORMAL, WM_CHAR, WM_IME_NOTIFY, WM_IME_SETCONTEXT, WM_KEYDOWN,
                WM_KEYUP, WM_KILLFOCUS, WNDPROC,
            },
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

type GlGenBuffersFn = unsafe extern "system" fn(n: GLsizei, buffers: *mut GLuint);
type GlDeleteBuffersFn = unsafe extern "system" fn(n: GLsizei, buffers: *const GLuint);
type GlBindBufferFn = unsafe extern "system" fn(target: GLenum, buffer: GLuint);
type GlBufferDataFn =
    unsafe extern "system" fn(target: GLenum, size: GLsizeiptr, data: *const GLvoid, usage: GLenum);
type GlMapBufferFn = unsafe extern "system" fn(target: GLenum, access: GLenum) -> *mut c_void;
type GlUnmapBufferFn = unsafe extern "system" fn(target: GLenum) -> GLboolean;

static GL_GEN_BUFFERS: OnceLock<GlGenBuffersFn> = OnceLock::new();
static GL_DELETE_BUFFERS: OnceLock<GlDeleteBuffersFn> = OnceLock::new();
static GL_BIND_BUFFER: OnceLock<GlBindBufferFn> = OnceLock::new();
static GL_BUFFER_DATA: OnceLock<GlBufferDataFn> = OnceLock::new();
static GL_MAP_BUFFER: OnceLock<GlMapBufferFn> = OnceLock::new();
static GL_UNMAP_BUFFER: OnceLock<GlUnmapBufferFn> = OnceLock::new();

pub unsafe fn load_opengl_extensions() -> bool {
    let load_fn = |name: &str| -> *const c_void {
        let cname = CString::new(name).unwrap();
        let pcstr = PCSTR(cname.as_ptr() as *const u8);
        match wglGetProcAddress(pcstr) {
            Some(f) => f as *const c_void,
            None => null(),
        }
    };

    macro_rules! load {
        ($sym:ident, $type:ty, $name:literal) => {{
            let ptr = load_fn($name);
            if ptr.is_null() {
                return false;
            }
            $sym.set(std::mem::transmute::<*const c_void, $type>(ptr))
                .is_ok()
        }};
    }

    load!(GL_GEN_BUFFERS, GlGenBuffersFn, "glGenBuffers")
        && load!(GL_DELETE_BUFFERS, GlDeleteBuffersFn, "glDeleteBuffers")
        && load!(GL_BIND_BUFFER, GlBindBufferFn, "glBindBuffer")
        && load!(GL_BUFFER_DATA, GlBufferDataFn, "glBufferData")
        && load!(GL_MAP_BUFFER, GlMapBufferFn, "glMapBuffer")
        && load!(GL_UNMAP_BUFFER, GlUnmapBufferFn, "glUnmapBuffer")
}

#[derive(Copy, Clone)]
struct SafeHGLRC(HGLRC);

unsafe impl Send for SafeHGLRC {}
unsafe impl Sync for SafeHGLRC {}

lazy_static! {
    static ref CONTEXTS: Mutex<HashMap<i32, SafeHGLRC>> = Mutex::new(HashMap::new());
}

static WIDTH: AtomicI32 = AtomicI32::new(0);
static HEIGHT: AtomicI32 = AtomicI32::new(0);

pub unsafe fn generate_pixel_buffers(
    pbo: &mut [GLuint; 2],
    width: GLint,
    height: GLint,
    stride: GLint,
) {
    let w = WIDTH.load(Ordering::Relaxed);
    let h = HEIGHT.load(Ordering::Relaxed);

    if (w != width) || (h != height) {
        let gl_gen_buffers = *GL_GEN_BUFFERS.get().unwrap();
        let gl_buffer_data = *GL_BUFFER_DATA.get().unwrap();
        let gl_bind_buffer = *GL_BIND_BUFFER.get().unwrap();
        let gl_delete_buffers = *GL_DELETE_BUFFERS.get().unwrap();

        WIDTH.store(width, Ordering::Relaxed);
        HEIGHT.store(height, Ordering::Relaxed);

        if pbo[1] != 0 {
            gl_delete_buffers(2, pbo.as_ptr());
            pbo[0] = 0;
            pbo[1] = 0;
        }

        gl_gen_buffers(2, pbo.as_mut_ptr());

        let buffer_size = (width * height * stride) as GLsizeiptr;

        gl_bind_buffer(PIXEL_PACK_BUFFER, pbo[0]);
        gl_buffer_data(PIXEL_PACK_BUFFER, buffer_size, null(), STREAM_READ);
        gl_bind_buffer(gl::PIXEL_PACK_BUFFER, 0);

        gl_bind_buffer(gl::PIXEL_PACK_BUFFER, pbo[1]);
        gl_buffer_data(PIXEL_PACK_BUFFER, buffer_size, null(), STREAM_READ);
        gl_bind_buffer(PIXEL_PACK_BUFFER, 0);
    }
}

static INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn flip_image_bytes(input: *const u8, output: *mut u8, width: i32, height: i32, bpp: u32) {
    let chunk = if bpp > 24 {
        (width as usize) * 4
    } else {
        (width as usize) * 3 + (width as usize) % 4
    };

    unsafe {
        let mut source = input.add(chunk * (height as usize - 1));
        let mut destination = output;

        while source != input {
            std::ptr::copy_nonoverlapping(source, destination, chunk);
            destination = destination.add(chunk);
            source = source.sub(chunk);
        }
    }
}

pub unsafe fn read_pixel_buffers(
    dest: *mut GLubyte,
    pbo: &mut [GLuint; 2],
    width: GLint,
    height: GLint,
) {
    let index_cell = INDEX.load(Ordering::Relaxed);
    let gl_bind_buffer = *GL_BIND_BUFFER.get().unwrap();
    let gl_unmap_buffer = *GL_UNMAP_BUFFER.get().unwrap();
    let gl_map_buffer = *GL_MAP_BUFFER.get().unwrap();

    let gl_format = BGRA;
    // Swap indices
    let index = (index_cell + 1) % 2;
    let next_index = (index + 1) % 2;
    INDEX.store(index, Ordering::Relaxed);

    glPixelStorei(UNPACK_ROW_LENGTH, width);
    gl_bind_buffer(PIXEL_PACK_BUFFER, pbo[index]);
    glReadPixels(0, 0, width, height, gl_format, UNSIGNED_BYTE, null_mut());
    gl_bind_buffer(PIXEL_PACK_BUFFER, pbo[next_index]);

    let data = gl_map_buffer(gl::PIXEL_PACK_BUFFER, gl::READ_ONLY);

    if !data.is_null() {
        flip_image_bytes(data as *const u8, dest.cast(), width, height, 32);
        gl_unmap_buffer(PIXEL_PACK_BUFFER);
    } else {
        glReadPixels(
            0,
            0,
            width,
            height,
            gl_format,
            gl::UNSIGNED_BYTE,
            dest as *mut _,
        );
    }

    gl_bind_buffer(PIXEL_PACK_BUFFER, 0);
    glPixelStorei(UNPACK_ROW_LENGTH, 0);
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

lazy_static! {
    static ref PBO: Mutex<[GLuint; 2]> = Mutex::new([0; 2]);
}

unsafe extern "system" fn hooked_wgl_swap_buffers(hdc: HDC) -> BOOL {
    let hwnd = WindowFromDC(hdc);
    let mouse = get_mouse_pos(hwnd.0 as u64);
    let _ = mouse;

    let mut viewport = [0, 0, 0, 0];

    glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr());

    let width = viewport[2];
    let height = viewport[3];

    if load_opengl_extensions() {
        let dest = get_debug_image(width as usize, height as usize);
        if !dest.is_null() {
            let mut pbo = PBO.lock().unwrap();
            generate_pixel_buffers(&mut *pbo, width, height, 4);
            read_pixel_buffers(dest, &mut *pbo, width, height);
        }

        let old_ctx = wglGetCurrentContext();

        push_gl_context(hdc, width, height);

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
