mod geometry;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(crate) use geometry::ScreenLayout;
pub(crate) use geometry::{MonitorBounds, MonitorInfo};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct MonitorDiscoveryPayload {
    pub(crate) monitors: Vec<MonitorInfo>,
    pub(crate) supported: bool,
    pub(crate) unavailable_reason: Option<String>,
    pub(crate) virtual_bounds: Option<MonitorBounds>,
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ScreenPixel {
    pub(crate) alpha: u8,
    pub(crate) blue: u8,
    pub(crate) green: u8,
    pub(crate) hex: String,
    pub(crate) integer: u32,
    pub(crate) red: u8,
}

#[cfg(any(windows, test))]
impl ScreenPixel {
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            alpha: 255,
            blue,
            green,
            hex: format!("#{red:02X}{green:02X}{blue:02X}"),
            integer: (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue),
            red,
        }
    }
}

pub(crate) fn discover_monitors() -> Result<MonitorDiscoveryPayload, String> {
    #[cfg(windows)]
    {
        let layout = windows::discover_screen_layout()?;
        Ok(MonitorDiscoveryPayload {
            monitors: layout.monitors,
            supported: true,
            unavailable_reason: None,
            virtual_bounds: Some(layout.virtual_bounds),
        })
    }

    #[cfg(not(windows))]
    {
        Ok(MonitorDiscoveryPayload {
            monitors: Vec::new(),
            supported: false,
            unavailable_reason: Some(
                "Screen coordinate tools require the Windows desktop runner.".to_owned(),
            ),
            virtual_bounds: None,
        })
    }
}

#[cfg(windows)]
pub(crate) fn validate_coordinate(x: i32, y: i32) -> Result<MonitorInfo, String> {
    windows::discover_screen_layout()?.monitor_at(x, y)
}

#[cfg(windows)]
pub(crate) fn cursor_position() -> Result<(i32, i32), String> {
    windows::cursor_position()
}

#[cfg(windows)]
pub(crate) fn sample_pixel(x: i32, y: i32) -> Result<ScreenPixel, String> {
    windows::sample_pixel(x, y)
}

#[cfg(windows)]
pub(crate) fn flush_desktop_composition() -> Result<(), String> {
    windows::flush_desktop_composition()
}

#[cfg(windows)]
pub(crate) fn move_cursor_absolute(x: i32, y: i32) -> Result<(), String> {
    windows::move_cursor_absolute(x, y)
}

#[cfg(windows)]
pub(crate) fn move_cursor_relative(x: i32, y: i32) -> Result<(i32, i32), String> {
    let (current_x, current_y) = windows::cursor_position()?;
    let destination_x = checked_offset("X", current_x, x)?;
    let destination_y = checked_offset("Y", current_y, y)?;
    validate_coordinate(destination_x, destination_y)?;
    windows::move_cursor_absolute(destination_x, destination_y)?;
    Ok((destination_x, destination_y))
}

#[cfg(any(windows, test))]
fn checked_offset(axis: &str, current: i32, offset: i32) -> Result<i32, String> {
    current.checked_add(offset).ok_or_else(|| {
        format!("relative {axis} offset {offset} overflows from current coordinate {current}")
    })
}

#[cfg(test)]
mod tests {
    use super::{ScreenPixel, checked_offset};

    #[test]
    fn relative_coordinate_arithmetic_rejects_i32_overflow() {
        assert_eq!(checked_offset("X", -200, 50), Ok(-150));
        assert!(checked_offset("X", i32::MAX, 1).is_err());
        assert!(checked_offset("Y", i32::MIN, -1).is_err());
    }

    #[test]
    fn screen_pixel_uses_consistent_rgb_hex_and_integer_values() {
        assert_eq!(
            ScreenPixel::from_rgb(0x12, 0xAB, 0xF0),
            ScreenPixel {
                alpha: 255,
                blue: 0xF0,
                green: 0xAB,
                hex: "#12ABF0".to_owned(),
                integer: 0x12ABF0,
                red: 0x12,
            }
        );
    }
}
