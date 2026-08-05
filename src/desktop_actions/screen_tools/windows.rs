use std::{io, mem::size_of, os::windows::ffi::OsStringExt};

use windows_sys::Win32::{
    Foundation::{LPARAM, POINT, RECT},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
        CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, EnumDisplayMonitors, GetDC,
        GetDIBits, GetMonitorInfoW, GetPixel, HBITMAP, HDC, HGDIOBJ, HMONITOR, MONITORINFO,
        MONITORINFOEXW, ReleaseDC, SRCCOPY, SelectObject,
    },
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{GetCursorPos, MONITORINFOF_PRIMARY, SetCursorPos},
    },
};

use super::{MonitorBounds, MonitorInfo, ScreenLayout, ScreenPixel, ScreenSnapshot};

pub(super) fn discover_screen_layout() -> Result<ScreenLayout, String> {
    let mut state = MonitorEnumerationState::default();
    let state_pointer = (&mut state as *mut MonitorEnumerationState).cast::<()>() as LPARAM;
    let result = unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(enumerate_monitor),
            state_pointer,
        )
    };

    if let Some(error) = state.error {
        return Err(error);
    }
    if result == 0 {
        return Err(format!(
            "failed to enumerate connected monitors: {}",
            io::Error::last_os_error()
        ));
    }

    ScreenLayout::new(state.monitors)
}

pub(super) fn move_cursor_absolute(x: i32, y: i32) -> Result<(), String> {
    if unsafe { SetCursorPos(x, y) } == 0 {
        return Err(format!(
            "failed to move the cursor to ({x}, {y}): {}",
            io::Error::last_os_error()
        ));
    }

    let (actual_x, actual_y) = cursor_position()
        .map_err(|error| format!("moved the cursor but failed to verify its position: {error}"))?;
    if actual_x != x || actual_y != y {
        return Err(format!(
            "the cursor was constrained to ({}, {}) instead of the requested coordinate ({x}, {y})",
            actual_x, actual_y
        ));
    }

    Ok(())
}

pub(super) fn cursor_position() -> Result<(i32, i32), String> {
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err(format!(
            "failed to read the current cursor position: {}",
            io::Error::last_os_error()
        ));
    }
    Ok((point.x, point.y))
}

pub(super) fn sample_pixel(x: i32, y: i32) -> Result<ScreenPixel, String> {
    let device_context = unsafe { GetDC(std::ptr::null_mut()) };
    if device_context.is_null() {
        return Err("failed to get the screen device context".to_owned());
    }

    let pixel = unsafe { GetPixel(device_context, x, y) };
    unsafe { ReleaseDC(std::ptr::null_mut(), device_context) };
    if pixel == u32::MAX {
        return Err(format!("failed to read the screen pixel at ({x}, {y})"));
    }

    let red = (pixel & 0x0000_00ff) as u8;
    let green = ((pixel & 0x0000_ff00) >> 8) as u8;
    let blue = ((pixel & 0x00ff_0000) >> 16) as u8;
    Ok(ScreenPixel::from_rgb(red, green, blue))
}

pub(super) fn capture_snapshot(bounds: MonitorBounds) -> Result<ScreenSnapshot, String> {
    let width = i32::try_from(bounds.width)
        .map_err(|_| "screen snapshot width exceeds the Windows GDI range".to_owned())?;
    let height = i32::try_from(bounds.height)
        .map_err(|_| "screen snapshot height exceeds the Windows GDI range".to_owned())?;
    let pixel_count = usize::try_from(bounds.width)
        .ok()
        .and_then(|width| {
            usize::try_from(bounds.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| "screen snapshot dimensions exceed the supported memory range".to_owned())?;
    let image_bytes = pixel_count
        .checked_mul(size_of::<u32>())
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or_else(|| "screen snapshot byte size exceeds the Windows GDI range".to_owned())?;

    let screen_dc = ScreenDeviceContext::acquire()?;
    let memory_dc = MemoryDeviceContext::create(screen_dc.handle())?;
    let bitmap = OwnedBitmap::create(screen_dc.handle(), width, height)?;
    let selection = SelectedObject::select(memory_dc.handle(), bitmap.handle())?;
    if unsafe {
        BitBlt(
            memory_dc.handle(),
            0,
            0,
            width,
            height,
            screen_dc.handle(),
            bounds.left,
            bounds.top,
            SRCCOPY | CAPTUREBLT,
        )
    } == 0
    {
        return Err(format!(
            "failed to capture the desktop for the coordinate picker: {}",
            io::Error::last_os_error()
        ));
    }
    selection.restore()?;

    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(pixel_count)
        .map_err(|error| format!("failed to allocate the coordinate picker snapshot: {error}"))?;
    pixels.resize(pixel_count, 0u32);
    let mut bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(size_of::<BITMAPINFOHEADER>())
                .expect("BITMAPINFOHEADER size fits in u32"),
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: image_bytes,
            ..BITMAPINFOHEADER::default()
        },
        ..BITMAPINFO::default()
    };
    let copied_lines = unsafe {
        GetDIBits(
            memory_dc.handle(),
            bitmap.handle(),
            0,
            bounds.height,
            pixels.as_mut_ptr().cast(),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        )
    };
    if copied_lines != height {
        return Err(format!(
            "failed to read the coordinate picker snapshot: copied {copied_lines} of {height} rows"
        ));
    }

    ScreenSnapshot::new(bounds, pixels)
}

struct ScreenDeviceContext(HDC);

impl ScreenDeviceContext {
    fn acquire() -> Result<Self, String> {
        let handle = unsafe { GetDC(std::ptr::null_mut()) };
        if handle.is_null() {
            return Err("failed to get the screen device context".to_owned());
        }
        Ok(Self(handle))
    }

    fn handle(&self) -> HDC {
        self.0
    }
}

impl Drop for ScreenDeviceContext {
    fn drop(&mut self) {
        unsafe { ReleaseDC(std::ptr::null_mut(), self.0) };
    }
}

struct MemoryDeviceContext(HDC);

impl MemoryDeviceContext {
    fn create(screen_dc: HDC) -> Result<Self, String> {
        let handle = unsafe { CreateCompatibleDC(screen_dc) };
        if handle.is_null() {
            return Err(format!(
                "failed to create the coordinate picker memory context: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(Self(handle))
    }

    fn handle(&self) -> HDC {
        self.0
    }
}

impl Drop for MemoryDeviceContext {
    fn drop(&mut self) {
        unsafe { DeleteDC(self.0) };
    }
}

struct OwnedBitmap(HBITMAP);

impl OwnedBitmap {
    fn create(screen_dc: HDC, width: i32, height: i32) -> Result<Self, String> {
        let handle = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
        if handle.is_null() {
            return Err(format!(
                "failed to create the coordinate picker bitmap: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(Self(handle))
    }

    fn handle(&self) -> HBITMAP {
        self.0
    }
}

impl Drop for OwnedBitmap {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.0) };
    }
}

struct SelectedObject {
    device_context: HDC,
    previous: HGDIOBJ,
    restored: bool,
}

impl SelectedObject {
    fn select(device_context: HDC, bitmap: HBITMAP) -> Result<Self, String> {
        let previous = unsafe { SelectObject(device_context, bitmap) };
        if previous.is_null() {
            return Err(format!(
                "failed to select the coordinate picker bitmap: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(Self {
            device_context,
            previous,
            restored: false,
        })
    }

    fn restore(mut self) -> Result<(), String> {
        if unsafe { SelectObject(self.device_context, self.previous) }.is_null() {
            return Err(format!(
                "failed to release the coordinate picker bitmap: {}",
                io::Error::last_os_error()
            ));
        }
        self.restored = true;
        Ok(())
    }
}

impl Drop for SelectedObject {
    fn drop(&mut self) {
        if !self.restored {
            unsafe { SelectObject(self.device_context, self.previous) };
        }
    }
}

#[derive(Default)]
struct MonitorEnumerationState {
    error: Option<String>,
    monitors: Vec<MonitorInfo>,
}

unsafe extern "system" fn enumerate_monitor(
    monitor: HMONITOR,
    _device_context: HDC,
    _monitor_rect: *mut RECT,
    parameter: LPARAM,
) -> i32 {
    let state = unsafe { &mut *(parameter as *mut MonitorEnumerationState) };
    match monitor_info(monitor) {
        Ok(info) => {
            state.monitors.push(info);
            1
        }
        Err(error) => {
            state.error = Some(error);
            0
        }
    }
}

fn monitor_info(monitor: HMONITOR) -> Result<MonitorInfo, String> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    if unsafe {
        GetMonitorInfoW(
            monitor,
            (&mut info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
        )
    } == 0
    {
        return Err(format!(
            "failed to read monitor information: {}",
            io::Error::last_os_error()
        ));
    }

    let device_name = wide_string(&info.szDevice);
    if device_name.is_empty() {
        return Err("Windows returned a monitor without a device name".to_owned());
    }
    let (dpi_x, dpi_y) = monitor_dpi(monitor)
        .map(|(x, y)| (Some(x), Some(y)))
        .unwrap_or((None, None));

    Ok(MonitorInfo {
        bounds: monitor_bounds(info.monitorInfo.rcMonitor)?,
        device_name: device_name.clone(),
        dpi_x,
        dpi_y,
        id: format!("windows:{device_name}"),
        is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        scale_factor: dpi_x.map(|dpi| f64::from(dpi) / 96.0),
        work_area: monitor_bounds(info.monitorInfo.rcWork)?,
    })
}

fn monitor_bounds(rect: RECT) -> Result<MonitorBounds, String> {
    MonitorBounds::new(rect.left, rect.top, rect.right, rect.bottom)
}

fn monitor_dpi(monitor: HMONITOR) -> Option<(u32, u32)> {
    let mut dpi_x = 0;
    let mut dpi_y = 0;
    let result = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    (result >= 0 && dpi_x > 0 && dpi_y > 0).then_some((dpi_x, dpi_y))
}

fn wide_string(value: &[u16]) -> String {
    let length = value
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(value.len());
    std::ffi::OsString::from_wide(&value[..length])
        .to_string_lossy()
        .into_owned()
}
