use std::{
    cell::RefCell,
    ffi::{c_char, c_void},
    mem::transmute,
    ptr::null_mut,
};

use windows::{
    core::{s, BOOL, PCSTR},
    Win32::{
        Foundation::{
            CloseHandle, GetLastError, FALSE, HANDLE, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT,
            RECT, TRUE, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM,
        },
        System::{
            Console::{AllocConsole, AttachConsole, GetConsoleWindow, ATTACH_PARENT_PROCESS},
            Diagnostics::Debug::WriteProcessMemory,
            LibraryLoader::{DisableThreadLibraryCalls, GetModuleHandleA, GetProcAddress},
            Memory::{
                CreateFileMappingA, MapViewOfFile, OpenFileMappingA, VirtualAllocEx, VirtualFreeEx,
                FILE_MAP_ALL_ACCESS, FILE_MAP_READ, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE,
                PAGE_READWRITE,
            },
            Threading::{
                CreateRemoteThread, CreateThread, GetCurrentProcessId, OpenProcess,
                WaitForSingleObject, PROCESS_ALL_ACCESS, THREAD_CREATION_FLAGS,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
            WindowsAndMessaging::{
                CallWindowProcW, EnumChildWindows, EnumWindows, GetClassNameW, GetWindowRect,
                GetWindowThreadProcessId, IsWindowVisible, PostMessageW, SetWindowLongPtrW,
                ShowWindow, GWLP_WNDPROC, SW_HIDE, SW_SHOWNORMAL, WM_CHAR, WM_IME_NOTIFY,
                WM_IME_SETCONTEXT, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONDOWN,
                WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETFOCUS, WM_USER,
                WNDPROC,
            },
        },
    },
};

const WI_CONSOLE: u32 = WM_USER + 1;
const WI_MOUSEMOVE: u32 = WM_USER + 2;
const WI_LBUTTONDOWN: u32 = WM_USER + 3;
const WI_LBUTTONUP: u32 = WM_USER + 4;
const WI_RBUTTONDOWN: u32 = WM_USER + 6;
const WI_RBUTTONUP: u32 = WM_USER + 7;
const WI_KEYDOWN: u32 = WM_USER + 8;
const WI_CHAR: u32 = WM_USER + 9;
const WI_KEYUP: u32 = WM_USER + 10;
const WI_SETFOCUS: u32 = WM_USER + 11;
const WI_KILLFOCUS: u32 = WM_USER + 12;

#[no_mangle]
pub static mut MODULE: HMODULE = HMODULE(null_mut());

#[no_mangle]
unsafe extern "system" fn start_thread(lparam: *mut c_void) -> u32 {
    let _success = hook_wndproc(lparam as u64);
    0
}

unsafe fn open_client_console() {
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

#[no_mangle]
pub extern "system" fn DllMain(
    hinst_dll: HINSTANCE,
    fdw_reason: u32, // DWORD is u32 in windows crate
    _lpv_reserved: *mut c_void,
) -> BOOL {
    unsafe { MODULE = HMODULE(hinst_dll.0) };

    let pid = unsafe { GetCurrentProcessId() };
    let hwnd = match get_jagrenderview(pid) {
        Some(hwnd) => hwnd,
        None => return TRUE,
    };

    match fdw_reason {
        1 => unsafe {
            let _ = DisableThreadLibraryCalls(hinst_dll.into());

            if let Ok(hmap) =
                OpenFileMappingA(FILE_MAP_READ.0, false, PCSTR(b"WASPINPUT_FLAG\0".as_ptr()))
            {
                let flag_ptr = MapViewOfFile(hmap, FILE_MAP_READ, 0, 0, 1);
                let is_injected = *(flag_ptr.Value as *const u8);
                if is_injected == 1 {
                    open_client_console();
                    println!("[WaspInput]: Console attached. PID: {:?}\r\n", pid);

                    let _ = CreateThread(
                        Some(null_mut()),
                        0,
                        Some(start_thread),
                        Some(hwnd.0 as *mut c_void),
                        THREAD_CREATION_FLAGS(0),
                        Some(null_mut()),
                    );
                }
            }
        },
        0 => {
            println!("[WaspInput]: Detached.\r\n");
            unsafe { unhook_wndproc(hwnd.0 as u64) };
        }
        _ => (),
    }

    TRUE
}

pub unsafe fn get_proc_address(name: *const c_char) -> *mut c_void {
    let name_str = PCSTR::from_raw(name as *const u8);
    let func_ptr = GetProcAddress(MODULE, name_str);
    transmute(func_ptr)
}

pub struct Injector;

impl Injector {
    pub unsafe fn inject(module_path: &str, pid: u32) -> bool {
        let hmap = CreateFileMappingA(
            HANDLE::default(),
            None,
            PAGE_READWRITE,
            0,
            1,
            PCSTR(b"WASPINPUT_FLAG\0".as_ptr()),
        )
        .expect("[WaspInput]: Cannot initialize mappings.\r\n");

        let flag_ptr = MapViewOfFile(hmap, FILE_MAP_ALL_ACCESS, 0, 0, 1);

        // Correct way to write to the mapped memory
        let flag_ptr_raw = flag_ptr.Value as *mut u8;
        *flag_ptr_raw = 1;

        let process_handle = match unsafe { OpenProcess(PROCESS_ALL_ACCESS, false, pid) } {
            Ok(process) => {
                unsafe {
                    if WaitForSingleObject(process, 0) != WAIT_TIMEOUT {
                        eprintln!("[WaspInput]: Process is not alive.\r\n");
                        CloseHandle(process).ok();
                        return false;
                    }
                }
                process
            }
            Err(_) => {
                eprintln!("[WaspInput]: OpenProcess failed.\r\n");
                return false;
            }
        };

        let kernel32 = match unsafe { GetModuleHandleA(s!("kernel32.dll")) } {
            Ok(h) => h,
            Err(_) => {
                eprintln!("[WaspInput]: GetModuleHandleA failed.\r\n");
                unsafe { CloseHandle(process_handle).ok() };
                return false;
            }
        };

        let size = module_path.len() + 1;
        let remote_address = unsafe {
            VirtualAllocEx(
                process_handle,
                None,
                size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };

        if remote_address.is_null() {
            eprintln!("[WaspInput]: VirtualAllocEx failed.\r\n");
            unsafe { CloseHandle(process_handle).ok() };
            return false;
        }

        let mut buffer = module_path.as_bytes().to_vec();
        buffer.push(0); // null-terminate

        if unsafe {
            WriteProcessMemory(
                process_handle,
                remote_address,
                buffer.as_ptr() as _,
                buffer.len(),
                None,
            )
        }
        .is_err()
        {
            eprintln!("[WaspInput]: WriteProcessMemory failed.\r\n");
            unsafe { CloseHandle(process_handle).ok() };
            return false;
        }

        let load_library = unsafe {
            GetProcAddress(kernel32, s!("LoadLibraryA")).map(|addr| {
                std::mem::transmute::<_, unsafe extern "system" fn(*mut c_void) -> u32>(addr)
            })
        };

        let load_library = match load_library {
            Some(f) => f,
            None => {
                eprintln!("[WaspInput]: GetProcAddress failed.\r\n");
                unsafe { CloseHandle(process_handle).ok() };
                return false;
            }
        };

        let remote_thread = match unsafe {
            CreateRemoteThread(
                process_handle,
                None,
                0,
                Some(load_library),
                Some(remote_address),
                0,
                None,
            )
        } {
            Ok(h) => h,
            Err(_) => {
                eprintln!("[WaspInput]: CreateRemoteThread failed\r\n");
                unsafe { CloseHandle(process_handle).ok() };
                return false;
            }
        };

        let wait_result = unsafe { WaitForSingleObject(remote_thread, 5000) };

        unsafe {
            CloseHandle(remote_thread).ok();
            CloseHandle(process_handle).ok();
            let _ = VirtualFreeEx(process_handle, remote_address, 0, MEM_RELEASE);
        }

        if wait_result != WAIT_OBJECT_0 {
            eprintln!("[WaspInput]: WaitForSingleObject timed out.\r\n");
            return false;
        }

        true
    }
}

pub fn get_jagrenderview(pid: u32) -> Option<HWND> {
    thread_local! {
        static FOUND_HWND: RefCell<Option<HWND>> = RefCell::new(None);
    }

    unsafe extern "system" fn enum_child_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let _ = lparam;
        let mut class_name = [0u16; 256];
        GetClassNameW(hwnd, &mut class_name);
        let class_str = String::from_utf16_lossy(&class_name[..]);
        if class_str.trim_end_matches('\0') == "JagRenderView" {
            FOUND_HWND.with(|cell| {
                *cell.borrow_mut() = Some(hwnd);
            });
            return FALSE; // Stop enumeration
        }
        TRUE
    }

    unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let mut proc_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut proc_id));
        if proc_id != lparam.0 as u32 {
            return TRUE;
        }

        let mut class_name = [0u16; 256];
        GetClassNameW(hwnd, &mut class_name);
        let class_str = String::from_utf16_lossy(&class_name[..]);
        if class_str.trim_end_matches('\0') == "JagWindow" {
            let _ = EnumChildWindows(Some(hwnd), Some(enum_child_proc), LPARAM(0));
            // If found, stop enumerating windows
            let found = FOUND_HWND.with(|cell| cell.borrow().is_some());
            if found {
                return FALSE;
            }
        }

        TRUE
    }

    unsafe {
        let _ = EnumWindows(Some(enum_windows_proc), LPARAM(pid as isize));
        FOUND_HWND.with(|cell| *cell.borrow())
    }
}

pub fn get_window_size(hwnd: u64) -> Option<(i32, i32)> {
    unsafe {
        // Convert u64 back to a raw pointer (*mut c_void), and then into HWND
        let hwnd = hwnd as *mut c_void; // Convert the u64 back to HWND (pointer)

        let mut rect = RECT::default();
        if GetWindowRect(HWND(hwnd), &mut rect).is_ok() {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            Some((width, height))
        } else {
            None
        }
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
        _ => "UNKNOWN",
    }
}

pub unsafe fn custom_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let original = ORIGINAL_WNDPROC.expect("[WaspInput]: ORIGINAL_WNDPROC is not set!\r\n");

    println!("[WaspInput]: custom_wndproc\r\n");
    let new_msg = match msg {
        x if x == WI_CONSOLE => {
            open_client_console();
            return LRESULT(0);
        }
        x if x == WI_MOUSEMOVE => WM_MOUSEMOVE,
        x if x == WI_LBUTTONDOWN => WM_LBUTTONDOWN,
        x if x == WI_LBUTTONUP => WM_LBUTTONUP,
        x if x == WI_RBUTTONDOWN => WM_RBUTTONDOWN,
        x if x == WI_RBUTTONUP => WM_RBUTTONUP,
        x if x == WI_KEYDOWN => WM_KEYDOWN,
        x if x == WI_CHAR => WM_CHAR,
        x if x == WI_KEYUP => WM_KEYUP,
        x if x == WI_SETFOCUS => WM_SETFOCUS,
        x if x == WI_KILLFOCUS => WM_KILLFOCUS,
        x if x == WM_KILLFOCUS => return LRESULT(0),
        //x if x == WM_IME_SETCONTEXT => return LRESULT(0),
        //x if x == WM_IME_NOTIFY => return LRESULT(0),
        _ => msg,
    };

    println!(
        "[WaspInput]: msg: {:x} name: {}\r\n",
        msg,
        message2string(new_msg)
    );

    CallWindowProcW(original, hwnd, new_msg, wparam, lparam)
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

//Input
pub fn is_input_enabled(hwnd: u64) -> bool {
    unsafe { IsWindowEnabled(HWND(hwnd as *mut c_void)).as_bool() }
}

pub fn toggle_input(hwnd: u64, state: bool) -> bool {
    unsafe { EnableWindow(HWND(hwnd as *mut c_void), state).as_bool() }
}

pub fn open_console(hwnd: u64) {
    let hwnd = Some(HWND(hwnd as *mut c_void));
    let _ = unsafe { PostMessageW(hwnd, WI_CONSOLE, WPARAM(0), LPARAM(0)) };
}

pub fn mouse_move(hwnd: u64, x: i32, y: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    let lparam = (x << 16) | y;
    unsafe {
        let _ = PostMessageW(Some(hwnd), WI_MOUSEMOVE, WPARAM(0), LPARAM(lparam as isize));
    }
}

pub fn lbutton(hwnd: u64, down: bool, x: i32, y: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    let lparam = (x << 16) | y;
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            if down { WI_LBUTTONDOWN } else { WI_LBUTTONUP },
            WPARAM(0),
            LPARAM(lparam as isize),
        );
    }
}

pub fn mbutton(hwnd: u64, down: bool, x: i32, y: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    let lparam = (x << 16) | y;
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            if down { WI_RBUTTONDOWN } else { WI_RBUTTONUP },
            WPARAM(0),
            LPARAM(lparam as isize),
        );
    }
}

pub fn rbutton(hwnd: u64, down: bool, x: i32, y: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    let lparam = (x << 16) | y;
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            if down { WI_RBUTTONDOWN } else { WI_RBUTTONUP },
            WPARAM(0),
            LPARAM(lparam as isize),
        );
    }
}

pub fn scroll(hwnd: u64, down: bool, x: i32, y: i32) {
    //let hwnd = HWND(hwnd as *mut c_void);
    let lparam = (x << 16) | y;
    print!(
        "[WaspInput]: TODO: scroll direction: {}, hwnd: {}, lparam: {}\r\n",
        down, hwnd, lparam
    );
}

pub fn key_down(hwnd: u64, vkey: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WI_KEYDOWN,
            WPARAM(vkey as usize),
            LPARAM(0x00000001),
        );
        let _ = PostMessageW(
            Some(hwnd),
            WI_CHAR,
            WPARAM(vkey as usize),
            LPARAM(0x00000001),
        );
    }
}

pub fn key_up(hwnd: u64, vkey: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WI_KEYUP,
            WPARAM(vkey as usize),
            LPARAM(0xc0000001),
        );
    }
}
