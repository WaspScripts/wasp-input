//Client sided code. Everything on this file is ran on client.
use lazy_static::lazy_static;
use std::{char::from_u32, ffi::c_void, mem::transmute, ptr::null_mut, sync::Mutex};

use windows::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM},
    System::Console::{AllocConsole, AttachConsole, GetConsoleWindow, ATTACH_PARENT_PROCESS},
    UI::{
        Input::KeyboardAndMouse::{GetKeyboardState, MapVirtualKeyW, ToUnicode, MAPVK_VK_TO_VSC},
        WindowsAndMessaging::{
            CallWindowProcW, IsWindowVisible, SetWindowLongPtrW, ShowWindow, GWLP_WNDPROC, SW_HIDE,
            SW_SHOWNORMAL, WM_CHAR, WM_IME_NOTIFY, WM_IME_SETCONTEXT, WM_KEYDOWN, WM_KEYUP,
            WM_KILLFOCUS, WNDPROC,
        },
    },
};

use crate::windows::{WI_CONSOLE, WI_SHIFTDOWN, WI_SHIFTUP};

lazy_static! {
    static ref SHIFT_KEY_DOWN: Mutex<bool> = Mutex::new(false);
}

pub unsafe extern "system" fn start_thread(lparam: *mut c_void) -> u32 {
    let _success = hook_wndproc(lparam as u64);
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

static mut ORIGINAL_WNDPROC: Option<WNDPROC> = None;

fn message2string(msg: u32) -> &'static str {
    match msg {
        0x0000 => "WM_NULL",
        0x0001 => "WM_CREATE",
        0x0002 => "WM_DESTROY",
        0x0003 => "WM_MOVE",
        0x0005 => "WM_SIZE",
        0x000F => "WM_PAINT",
        0x0010 => "WM_CLOSE",
        0x0012 => "WM_QUIT",
        0x0014 => "WM_ERASEBKGND",
        0x0018 => "WM_SHOWWINDOW",
        0x001C => "WM_ACTIVATEAPP",
        0x0020 => "WM_SETCURSOR",
        0x0021 => "WM_MOUSEACTIVATE",
        0x0024 => "WM_GETMINMAXINFO",
        0x0026 => "WM_PAINTICON",
        0x0027 => "WM_ICONERASEBKGND",
        0x0028 => "WM_NEXTDLGCTL",
        0x002A => "WM_SPOOLERSTATUS",
        0x002B => "WM_DRAWITEM",
        0x002C => "WM_MEASUREITEM",
        0x002D => "WM_DELETEITEM",
        0x002E => "WM_VKEYTOITEM",
        0x002F => "WM_CHARTOITEM",
        0x0030 => "WM_SETFONT",
        0x0031 => "WM_GETFONT",
        0x0032 => "WM_SETHOTKEY",
        0x0033 => "WM_GETHOTKEY",
        0x0037 => "WM_QUERYDRAGICON",
        0x0039 => "WM_COMPAREITEM",
        0x0046 => "WM_WINDOWPOSCHANGING",
        0x0047 => "WM_WINDOWPOSCHANGED",
        0x0200 => "WM_MOUSEMOVE",
        0x0201 => "WM_LBUTTONDOWN",
        0x0202 => "WM_LBUTTONUP",
        0x0204 => "WM_RBUTTONDOWN",
        0x0205 => "WM_RBUTTONUP",
        0x0100 => "WM_KEYDOWN",
        0x0101 => "WM_KEYUP",
        0x0102 => "WM_CHAR",
        0x0007 => "WM_SETFOCUS",
        0x0008 => "WM_KILLFOCUS",
        0x0084 => "WM_NCHITTEST",
        0x0104 => "WM_SYSKEYDOWN",
        0x0105 => "WM_SYSKEYUP",
        0x0215 => "WM_CAPTURECHANGED",
        0x0281 => "WM_IME_SETCONTEXT",
        0x0282 => "WM_IME_NOTIFY",
        WI_CONSOLE => "WI_CONSOLE",
        WI_SHIFTDOWN => "WI_SHIFTDOWN",
        WI_SHIFTUP => "WI_SHIFTUP",
        _ => "UNKNOWN",
    }
}

unsafe fn get_updated_wparam(wparam: WPARAM) -> WPARAM {
    let vkey = wparam.0 as u32;

    if let Some(c) = from_u32(vkey) {
        if c.is_alphabetic() {
            println!(
                "[WaspInput]: IS ALPHABETICAL: {}\r\n",
                c.to_ascii_uppercase()
            );
            return WPARAM(c.to_ascii_uppercase() as usize);
        }
    }

    let scancode = MapVirtualKeyW(vkey, MAPVK_VK_TO_VSC);
    println!("[WaspInput]: VKEY: {}, scancode: {}\r\n", vkey, scancode);

    let mut keyboard_state = [0u8; 256];
    let _ = GetKeyboardState(&mut keyboard_state);

    keyboard_state[0x10 as usize] = 0x80;
    keyboard_state[vkey as usize] = 0x80;

    let mut buff = [0u16; 4];
    let _ = ToUnicode(vkey, scancode, Some(&keyboard_state), &mut buff, 0);

    match from_u32(buff[0] as u32) {
        Some(unicode) => {
            println!("[WaspInput]: unicode: {}\r\n", unicode);
            WPARAM(unicode as usize)
        }
        None => wparam,
    }
}

pub unsafe fn custom_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let original = ORIGINAL_WNDPROC.expect("[WaspInput]: ORIGINAL_WNDPROC is not set!\r\n");
    println!(
        "[WaspInput]: message: {}, wparam: {:?}, lparam: {:?}\r\n",
        message2string(msg),
        wparam,
        lparam
    );

    match msg {
        WI_CONSOLE => {
            open_client_console();
            return LRESULT(0);
        }
        WI_SHIFTDOWN => {
            *SHIFT_KEY_DOWN.lock().unwrap() = true;
            WM_KEYDOWN
        }
        WI_SHIFTUP => {
            *SHIFT_KEY_DOWN.lock().unwrap() = false;
            WM_KEYUP
        }
        WM_CHAR => {
            if *SHIFT_KEY_DOWN.lock().unwrap() {
                return CallWindowProcW(original, hwnd, msg, get_updated_wparam(wparam), lparam);
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

pub unsafe fn hook_wndproc(hwnd: u64) -> bool {
    let w = HWND(hwnd as *mut c_void);

    if ORIGINAL_WNDPROC.is_some() {
        println!("[WaspInput]: WndProc already hooked.\r\n");
        return false;
    }

    let previous = SetWindowLongPtrW(w, GWLP_WNDPROC, custom_wndproc as isize);
    if previous == 0 {
        println!(
            "[WaspInput]: Failed to set new WndProc: {:?}\r\n",
            GetLastError()
        );
        return false;
    }

    ORIGINAL_WNDPROC = Some(transmute(previous));

    println!("[WaspInput]: WndProc successfully hooked.\r\n");
    true
}

pub unsafe fn unhook_wndproc(hwnd: u64) -> bool {
    let original = ORIGINAL_WNDPROC.expect("[WaspInput]: ORIGINAL_WNDPROC is not set!\r\n");

    let hwnd = HWND(hwnd as isize as *mut c_void);

    if let Some(original) = original {
        let result = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, original as isize);
        if result == 0 {
            println!("[WaspInput]: Failed to restore original WndProc.\r\n");
            return false;
        }
        ORIGINAL_WNDPROC = None;
        println!("[WaspInput]: WndProc successfully restored.\r\n");
        return true;
    }

    println!("[WaspInput]: No original WndProc stored.\r\n");
    false
}
