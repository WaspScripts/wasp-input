use std::{
    cell::RefCell,
    ffi::{c_char, c_void},
    ptr::null_mut,
    sync::Mutex,
};

use windows::{
    core::{s, BOOL, PCSTR},
    Win32::{
        Foundation::{
            CloseHandle, GetLastError, FALSE, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, RECT,
            TRUE, WAIT_TIMEOUT, WPARAM,
        },
        System::{
            Diagnostics::Debug::WriteProcessMemory,
            LibraryLoader::{GetModuleHandleA, GetProcAddress},
            Memory::{
                VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
            },
            Threading::{CreateRemoteThread, OpenProcess, WaitForSingleObject, PROCESS_ALL_ACCESS},
        },
        UI::{
            Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
            WindowsAndMessaging::{
                CallWindowProcW, DefWindowProcW, EnumChildWindows, EnumWindows, GetClassNameW,
                GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, PostMessageW,
                SetWindowLongPtrW, GWLP_WNDPROC, WM_KEYDOWN, WM_KEYUP, WM_USER, WNDPROC,
            },
        },
    },
};

#[no_mangle]
pub static mut MODULE: HMODULE = HMODULE(null_mut());

#[no_mangle]
pub extern "system" fn DllMain(
    hinst_dll: HINSTANCE,
    _fdw_reason: u32, // DWORD is u32 in windows crate
    _lpv_reserved: *mut std::ffi::c_void,
) -> BOOL {
    unsafe { MODULE = HMODULE(hinst_dll.0) };
    TRUE
}

pub unsafe fn get_proc_address(name: *const c_char) -> *mut c_void {
    let name_str = PCSTR::from_raw(name as *const u8);
    let func_ptr = GetProcAddress(MODULE, name_str);
    std::mem::transmute(func_ptr)
}

pub struct Injector;

impl Injector {
    pub fn inject(module_path: &str, pid: u32) -> bool {
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
                eprintln!("CreateRemoteThread failed\r\n");
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

        if wait_result != windows::Win32::Foundation::WAIT_OBJECT_0 {
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

static ORIGINAL_WNDPROC: Mutex<Option<WNDPROC>> = Mutex::new(None);

unsafe extern "system" fn wndproc_hook(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let new_msg = match msg {
        x if x == (WM_USER + 1) => WM_KEYDOWN,
        x if x == (WM_USER + 2) => WM_KEYUP,
        _ => msg,
    };

    print!("MSG: {}, NEW: {}\r\n", msg, new_msg);

    let guard = ORIGINAL_WNDPROC.lock().unwrap();
    if let Some(original) = *guard {
        CallWindowProcW(original, hwnd, new_msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, new_msg, wparam, lparam)
    }
}

pub unsafe fn hook_wndproc(hwnd: u64) -> bool {
    let w = HWND(hwnd as *mut c_void);

    let prev_proc = GetWindowLongPtrW(w, GWLP_WNDPROC);
    if prev_proc == 0 {
        println!("Failed to get original WndProc: {:?}\r\n", GetLastError());
        return false;
    }

    let mut guard = ORIGINAL_WNDPROC.lock().unwrap();
    if guard.is_some() {
        println!("WndProc already hooked.\r\n");
        return false;
    }

    *guard = Some(std::mem::transmute(prev_proc));
    let result = SetWindowLongPtrW(w, GWLP_WNDPROC, wndproc_hook as isize);
    if result == 0 {
        println!("Failed to set new WndProc.\r\n: {:?}\r\n", GetLastError());
        return false;
    }

    println!("WndProc successfully hooked.\r\n");
    true
}

pub unsafe fn unhook_wndproc(hwnd: u64) -> bool {
    let w = HWND(hwnd as isize as *mut c_void);

    let mut guard = ORIGINAL_WNDPROC.lock().unwrap();
    if let Some(original) = *guard {
        if let Some(original) = original {
            let result = SetWindowLongPtrW(w, GWLP_WNDPROC, original as isize);
            if result == 0 {
                eprintln!("Failed to restore original WndProc.\r\n");
                return false;
            }
            *guard = None;
            println!("WndProc successfully restored.\r\n");
            return true;
        }
    }

    eprintln!("No original WndProc stored.\r\n");
    false
}

//Input
pub fn is_input_enabled(hwnd: u64) -> bool {
    unsafe { IsWindowEnabled(HWND(hwnd as *mut c_void)).as_bool() }
}

pub fn enable_input(hwnd: u64) -> bool {
    unsafe { EnableWindow(HWND(hwnd as *mut c_void), true).as_bool() }
}

pub fn disable_input(hwnd: u64) -> bool {
    unsafe { EnableWindow(HWND(hwnd as *mut c_void), false).as_bool() }
}

pub fn key_down(hwnd: u64, vkey: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_USER + 1,
            WPARAM(vkey as usize),
            LPARAM(0x001E0001),
        );
    }
}

pub fn key_up(hwnd: u64, vkey: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_USER + 2,
            WPARAM(vkey as usize),
            LPARAM(0xC01E0001),
        );
    }
}
