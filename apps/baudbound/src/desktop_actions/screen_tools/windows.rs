use std::{io, mem::size_of, os::windows::ffi::OsStringExt};

use windows_sys::Win32::{
    Foundation::{LPARAM, POINT, RECT},
    Graphics::{
        Dwm::DwmFlush,
        Gdi::{
            EnumDisplayMonitors, GetDC, GetMonitorInfoW, GetPixel, HDC, HMONITOR, MONITORINFO,
            MONITORINFOEXW, ReleaseDC,
        },
    },
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{GetCursorPos, MONITORINFOF_PRIMARY, SetCursorPos},
    },
};

use super::{MonitorBounds, MonitorInfo, ScreenLayout, ScreenPixel};

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

pub(super) fn flush_desktop_composition() -> Result<(), String> {
    let result = unsafe { DwmFlush() };
    if result < 0 {
        return Err(format!(
            "failed to wait for the desktop to redraw after closing the picker: HRESULT {result:#010X}"
        ));
    }
    Ok(())
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
