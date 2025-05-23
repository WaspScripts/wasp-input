use std::ptr::null_mut;

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            CreateEventW, OpenEventW, ResetEvent, SetEvent, WaitForSingleObject,
            EVENT_MODIFY_STATE, INFINITE,
        },
    },
};

use crate::client::hooks::reenable_hooks;

fn to_wide_null_terminated(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn event_listener(hwnd: u64) {
    let event_name = to_wide_null_terminated(&format!("Global\\Restart-{}", hwnd));
    let event_name_ptr = PCWSTR(event_name.as_ptr());

    unsafe {
        let event = CreateEventW(Some(null_mut()), true, false, event_name_ptr)
            .expect("[WaspInput]: Failed to create/open event.\r\n");

        loop {
            WaitForSingleObject(event, INFINITE);
            reenable_hooks();
            let _ = ResetEvent(event);
        }
    }
}

pub fn call_event(hwnd: u64) {
    let event_name = to_wide_null_terminated(&format!("Global\\Restart-{}", hwnd));
    let event_name_ptr = PCWSTR(event_name.as_ptr());
    unsafe {
        let event = match OpenEventW(EVENT_MODIFY_STATE, false, event_name_ptr) {
            Ok(event) => event,
            Err(_) => return,
        };
        let _ = SetEvent(event);
    }
}

pub fn close_event(hwnd: u64) {
    let event_name = to_wide_null_terminated(&format!("Global\\Restart-{}", hwnd));
    let event_name_ptr = PCWSTR(event_name.as_ptr());

    unsafe {
        let event = match OpenEventW(EVENT_MODIFY_STATE, false, event_name_ptr) {
            Ok(event) => event,
            Err(_) => return,
        };
        let _ = CloseHandle(event).expect("[WaspInput]: Failed to create/open event.\r\n");
    }
}
