use super::{CaptureError, Display, Frame, PixelFormat, buffer_layout};
use std::{mem::size_of, ptr};
use windows_sys::Win32::{
    Foundation::{LPARAM, POINT, RECT},
    Graphics::{Dwm::DwmFlush, Gdi::*},
    UI::WindowsAndMessaging::GetCursorPos,
};

fn native(operation: &'static str) -> CaptureError {
    CaptureError::Native {
        operation,
        source: std::io::Error::last_os_error(),
    }
}

/// Read once on activation so moving the pointer while the worker starts cannot change target.
pub fn target_under_cursor() -> Result<(Display, (i32, i32)), CaptureError> {
    let mut point = POINT::default();
    // SAFETY: writable POINT lives for the call; monitor handles are borrowed from Windows.
    unsafe {
        if GetCursorPos(&mut point) == 0 {
            return Err(native("GetCursorPos"));
        }
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        Ok((monitor_display(monitor)?, (point.x, point.y)))
    }
}

fn monitor_display(monitor: HMONITOR) -> Result<Display, CaptureError> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: monitor is borrowed from a Win32 monitor API; structure has the required size.
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return Err(native("GetMonitorInfoW"));
    }
    let r = info.rcMonitor;
    let width = r
        .right
        .checked_sub(r.left)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or(CaptureError::InvalidFrame)?;
    let height = r
        .bottom
        .checked_sub(r.top)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or(CaptureError::InvalidFrame)?;
    buffer_layout(width, height)?;
    Ok(Display {
        x: r.left,
        y: r.top,
        width,
        height,
    })
}

pub fn displays() -> Result<Vec<Display>, CaptureError> {
    struct Enumeration {
        displays: Vec<Display>,
        error: Option<CaptureError>,
    }
    // SAFETY: EnumDisplayMonitors calls synchronously; data is the exclusive pointer to Enumeration.
    unsafe extern "system" fn visit(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> i32 {
        let enumeration = unsafe { &mut *(data as *mut Enumeration) };
        match monitor_display(monitor) {
            Ok(d) => {
                enumeration.displays.push(d);
                1
            }
            Err(e) => {
                enumeration.error = Some(e);
                0
            }
        }
    }
    let mut result = Enumeration {
        displays: Vec::new(),
        error: None,
    };
    // SAFETY: the callback never retains the pointer and enumeration finishes before result is read.
    let success = unsafe {
        EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(visit),
            &mut result as *mut _ as LPARAM,
        )
    };
    if let Some(error) = result.error {
        return Err(error);
    }
    if success == 0 {
        return Err(native("EnumDisplayMonitors"));
    }
    Ok(result.displays)
}

struct ScreenDc(HDC);
impl Drop for ScreenDc {
    fn drop(&mut self) {
        // SAFETY: DC was acquired from the desktop (null HWND) on this same thread.
        unsafe {
            ReleaseDC(ptr::null_mut(), self.0);
        }
    }
}
struct MemoryDc(HDC);
impl Drop for MemoryDc {
    fn drop(&mut self) {
        // SAFETY: uniquely owns a compatible DC; selections have already been restored.
        unsafe {
            DeleteDC(self.0);
        }
    }
}
struct Bitmap(HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: owns bitmap and it is not selected into a DC at this point.
        unsafe {
            DeleteObject(self.0);
        }
    }
}
struct Selection {
    dc: HDC,
    previous: HGDIOBJ,
}
impl Drop for Selection {
    fn drop(&mut self) {
        // SAFETY: restores the live DC's original object before bitmap/DC destruction.
        unsafe {
            SelectObject(self.dc, self.previous);
        }
    }
}

pub fn capture(display: Display) -> Result<Frame, CaptureError> {
    let (stride, size) = buffer_layout(display.width, display.height)?;
    let width = display.width as i32;
    let height = display.height as i32;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(size)
        .map_err(|_| CaptureError::Allocation)?;
    pixels.resize(size, 0);
    // SAFETY: all handles stay on this worker thread and are immediately guarded. Dimensions
    // and allocation are checked above. The bitmap is deselected before GetDIBits, as required.
    unsafe {
        // Wait for destruction/hiding of a previous viewer to reach the compositor.
        // This runs only on an explicitly requested capture, never as idle polling.
        let composition = DwmFlush();
        if composition < 0 {
            return Err(CaptureError::Composition(composition));
        }
        let dc = GetDC(ptr::null_mut());
        if dc.is_null() {
            return Err(native("GetDC"));
        }
        let screen = ScreenDc(dc);
        let dc = CreateCompatibleDC(screen.0);
        if dc.is_null() {
            return Err(native("CreateCompatibleDC"));
        }
        let memory = MemoryDc(dc);
        let bitmap = CreateCompatibleBitmap(screen.0, width, height);
        if bitmap.is_null() {
            return Err(native("CreateCompatibleBitmap"));
        }
        let bitmap = Bitmap(bitmap);
        {
            let previous = SelectObject(memory.0, bitmap.0);
            if previous.is_null() || previous as isize == -1 {
                return Err(native("SelectObject"));
            }
            let _selection = Selection {
                dc: memory.0,
                previous,
            };
            if BitBlt(
                memory.0,
                0,
                0,
                width,
                height,
                screen.0,
                display.x,
                display.y,
                SRCCOPY | CAPTUREBLT,
            ) == 0
            {
                return Err(native(
                    "BitBlt: desktop unavailable; unlock Windows or leave secure desktop",
                ));
            }
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        if GetDIBits(
            memory.0,
            bitmap.0,
            0,
            display.height,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        ) != height
        {
            return Err(native("GetDIBits"));
        }
    }
    let frame = Frame {
        display,
        stride,
        format: PixelFormat::Bgra8,
        pixels,
    };
    frame.validate()?;
    Ok(frame)
}
