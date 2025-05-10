use std::{
    cell::RefCell,
    ffi::{c_char, c_void},
    mem::transmute,
    ptr::null_mut,
    sync::Mutex,
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
                CallWindowProcW, DefWindowProcW, EnumChildWindows, EnumWindows, GetClassNameW,
                GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
                SetWindowLongPtrW, ShowWindow, GWLP_WNDPROC, SW_HIDE, SW_SHOWNORMAL, WM_CHAR,
                WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN,
                WM_RBUTTONUP, WM_SETFOCUS, WM_USER, WNDPROC,
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
    let pid = lparam as usize as u32;
    let hwnd = get_jagrenderview(pid)
        .expect("Can't find JagRenderView HWND!\r\n")
        .0 as u64;

    let _success = hook_wndproc(hwnd);
    0
}

#[no_mangle]
pub extern "system" fn DllMain(
    hinst_dll: HINSTANCE,
    fdw_reason: u32, // DWORD is u32 in windows crate
    _lpv_reserved: *mut c_void,
) -> BOOL {
    unsafe { MODULE = HMODULE(hinst_dll.0) };

    print!("HANDLE: {:?} FDW_REASON: {}\r\n", hinst_dll, fdw_reason);

    match fdw_reason {
        1 => unsafe {
            let _ = DisableThreadLibraryCalls(hinst_dll.into());

            if let Ok(hmap) =
                OpenFileMappingA(FILE_MAP_READ.0, false, PCSTR(b"WASPINPUT_FLAG\0".as_ptr()))
            {
                let flag_ptr = MapViewOfFile(hmap, FILE_MAP_READ, 0, 0, 1);
                let is_injected = *(flag_ptr.Value as *const u8);
                if is_injected == 1 {
                    let hwnd = GetConsoleWindow();
                    if hwnd.0 != null_mut() {
                        if IsWindowVisible(hwnd).as_bool() {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                        } else {
                            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
                        }
                    }

                    if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
                        let _ = AllocConsole();
                    }

                    let pid = GetCurrentProcessId();
                    println!("[WaspInput]: Console attached. PID: {:?}\r\n", pid);

                    let _ = CreateThread(
                        Some(null_mut()),
                        0,
                        Some(start_thread),
                        Some(pid as usize as *mut c_void),
                        THREAD_CREATION_FLAGS(0),
                        Some(null_mut()),
                    );
                }
            }
        },
        0 => println!("[WaspInput]: Detached.\r\n"),
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

static ORIGINAL_WNDPROC: Mutex<Option<WNDPROC>> = Mutex::new(None); //temporarily here, add to clients map later.

pub unsafe extern "system" fn custom_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let new_msg = match msg {
        x if x == WI_CONSOLE => {
            let hwnd = GetConsoleWindow();
            if hwnd.0 != null_mut() {
                if IsWindowVisible(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                } else {
                    let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
                }
            }
            if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
                let _ = AllocConsole();
            }
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
        x if x == WI_KILLFOCUS => return LRESULT(0),
        _ => msg,
    };

    println!("[WaspInput]: msg: {}, new_msg: {}\r\n", msg, new_msg);

    let guard = ORIGINAL_WNDPROC.lock().unwrap();
    if let Some(original) = *guard {
        CallWindowProcW(original, hwnd, new_msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, new_msg, wparam, lparam)
    }
}

#[no_mangle]
pub unsafe extern "system" fn hook_wndproc(hwnd: u64) -> bool {
    let w = HWND(hwnd as *mut c_void);

    let mut guard = ORIGINAL_WNDPROC.lock().unwrap();
    if guard.is_some() {
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

    *guard = Some(transmute(previous));

    println!("[WaspInput]: WndProc successfully hooked.\r\n");
    true
}

#[no_mangle]
pub unsafe extern "system" fn unhook_wndproc(hwnd: u64) -> bool {
    let w = HWND(hwnd as isize as *mut c_void);

    let mut guard = ORIGINAL_WNDPROC.lock().unwrap();
    if let Some(original) = *guard {
        if let Some(original) = original {
            let result = SetWindowLongPtrW(w, GWLP_WNDPROC, original as isize);
            if result == 0 {
                println!("[WaspInput]: Failed to restore original WndProc.\r\n");
                return false;
            }
            *guard = None;
            println!("[WaspInput]: WndProc successfully restored.\r\n");
            return true;
        }
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

pub fn key_down(hwnd: u64, vkey: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WI_KEYDOWN,
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
            LPARAM(0x00000001),
        );
    }
}
