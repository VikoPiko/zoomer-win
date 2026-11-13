use std::ptr;
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND},
    System::Threading::CreateMutexW,
    UI::WindowsAndMessaging::{
        GetForegroundWindow, IsWindow, MB_ICONINFORMATION, MB_OK, MessageBoxW, SetForegroundWindow,
    },
};

/// Opt-in lifecycle diagnostics go to stderr only; never include screenshot contents.
pub fn diagnostic(event: impl std::fmt::Display) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("ZOOMER_DIAGNOSTICS").is_some()) {
        eprintln!("Zoomer diagnostic: {event}");
    }
}

/// Winit permits one event loop per process; isolate desktop tests in fresh children.
#[cfg(test)]
pub fn gui_test_in_child(name: &str) -> bool {
    if std::env::var("ZOOMER_GUI_TEST_CHILD").as_deref() == Ok(name) {
        return false;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", name, "--nocapture"])
        .env("ZOOMER_GUI_TEST_CHILD", name)
        .status()
        .unwrap();
    assert!(status.success(), "desktop test failed: {name}");
    true
}

pub fn message(message: &str) {
    eprintln!("Zoomer: {message}");
    let message: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "Zoomer".encode_utf16().chain(Some(0)).collect();
    // SAFETY: both strings are NUL terminated and retained throughout the synchronous call.
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

pub struct Instance(HANDLE);
impl Instance {
    pub fn acquire() -> Result<Option<Self>, std::io::Error> {
        let name: Vec<u16> = "Local\\Zoomer-6C876F12-2E74-456F-A5AC-01C56564F6B1"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: valid NUL-terminated mutex name, default security; no ownership requested.
        unsafe {
            let handle = CreateMutexW(ptr::null(), 0, name.as_ptr());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let instance = Self(handle);
            if GetLastError() == ERROR_ALREADY_EXISTS {
                return Ok(None);
            }
            Ok(Some(instance))
        }
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: unique ownership of the handle returned by CreateMutexW; not an owned lock.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[derive(Default)]
pub struct Foreground(Option<HWND>);
impl Foreground {
    pub fn forget(&mut self) {
        self.0 = None;
    }
    pub fn remember(&mut self) {
        // SAFETY: returns a borrowed OS handle; only inspected/used on the event-loop thread.
        self.0 = Some(unsafe { GetForegroundWindow() });
    }
    pub fn restore(&mut self) {
        if let Some(window) = self.0.take() {
            // SAFETY: check handle is still valid; Windows may decline activation, which is fine.
            unsafe {
                if !window.is_null() && IsWindow(window) != 0 {
                    SetForegroundWindow(window);
                }
            }
        }
    }
}

/// Original procedural icon: cyan magnifying glass on a dark square; no asset decoder needed.
pub fn icon_rgba() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32 * 32 * 4);
    for y in 0..32_i32 {
        for x in 0..32_i32 {
            let r = (x - 13) * (x - 13) + (y - 13) * (y - 13);
            let glass = (49..=100).contains(&r)
                || ((20..=28).contains(&x) && (x - y).abs() <= 2 && y >= 20);
            bytes.extend_from_slice(if glass {
                &[89, 221, 235, 255]
            } else {
                &[23, 29, 43, 255]
            });
        }
    }
    bytes
}
