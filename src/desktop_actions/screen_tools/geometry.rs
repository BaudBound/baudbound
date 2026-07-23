use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct MonitorBounds {
    pub(crate) bottom: i32,
    pub(crate) height: u32,
    pub(crate) left: i32,
    pub(crate) right: i32,
    pub(crate) top: i32,
    pub(crate) width: u32,
}

impl MonitorBounds {
    #[cfg(any(windows, test))]
    pub(crate) fn new(left: i32, top: i32, right: i32, bottom: i32) -> Result<Self, String> {
        let width = i64::from(right) - i64::from(left);
        let height = i64::from(bottom) - i64::from(top);
        if width <= 0 || height <= 0 {
            return Err(format!(
                "invalid monitor bounds: left={left}, top={top}, right={right}, bottom={bottom}"
            ));
        }

        Ok(Self {
            bottom,
            height: u32::try_from(height)
                .map_err(|_| "monitor height exceeds the supported coordinate range".to_owned())?,
            left,
            right,
            top,
            width: u32::try_from(width)
                .map_err(|_| "monitor width exceeds the supported coordinate range".to_owned())?,
        })
    }

    #[cfg(any(windows, test))]
    pub(crate) fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    #[cfg(any(windows, test))]
    fn union(self, other: Self) -> Result<Self, String> {
        Self::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct MonitorInfo {
    pub(crate) bounds: MonitorBounds,
    pub(crate) device_name: String,
    pub(crate) dpi_x: Option<u32>,
    pub(crate) dpi_y: Option<u32>,
    pub(crate) id: String,
    pub(crate) is_primary: bool,
    pub(crate) scale_factor: Option<f64>,
    pub(crate) work_area: MonitorBounds,
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScreenLayout {
    pub(crate) monitors: Vec<MonitorInfo>,
    pub(crate) virtual_bounds: MonitorBounds,
}

#[cfg(any(windows, test))]
impl ScreenLayout {
    pub(crate) fn new(mut monitors: Vec<MonitorInfo>) -> Result<Self, String> {
        let first = monitors
            .first()
            .ok_or_else(|| "Windows did not report any connected monitors".to_owned())?;
        let virtual_bounds = monitors
            .iter()
            .skip(1)
            .try_fold(first.bounds, |bounds, monitor| bounds.union(monitor.bounds))?;

        monitors.sort_by(|left, right| {
            right
                .is_primary
                .cmp(&left.is_primary)
                .then_with(|| left.bounds.top.cmp(&right.bounds.top))
                .then_with(|| left.bounds.left.cmp(&right.bounds.left))
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(Self {
            monitors,
            virtual_bounds,
        })
    }

    pub(crate) fn monitor_at(&self, x: i32, y: i32) -> Result<MonitorInfo, String> {
        self.monitors
            .iter()
            .find(|monitor| monitor.bounds.contains(x, y))
            .cloned()
            .ok_or_else(|| self.coordinate_error(x, y))
    }

    fn coordinate_error(&self, x: i32, y: i32) -> String {
        let monitors = self
            .monitors
            .iter()
            .map(|monitor| {
                format!(
                    "{} [left={}, top={}, right={}, bottom={}]",
                    monitor.device_name,
                    monitor.bounds.left,
                    monitor.bounds.top,
                    monitor.bounds.right,
                    monitor.bounds.bottom
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "screen coordinate ({x}, {y}) is outside every connected monitor; virtual bounds are left={}, top={}, right={}, bottom={}; monitors: {monitors}",
            self.virtual_bounds.left,
            self.virtual_bounds.top,
            self.virtual_bounds.right,
            self.virtual_bounds.bottom
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_signed_coordinates_to_real_monitor_rectangles_and_rejects_gaps() {
        let layout = ScreenLayout::new(vec![
            monitor("primary", 0, 0, 1920, 1080, true),
            monitor("left", -1280, 200, 0, 1224, false),
            monitor("above", 500, -900, 2100, 0, false),
        ])
        .expect("layout should be valid");

        assert_eq!(layout.virtual_bounds, bounds(-1280, -900, 2100, 1224));
        assert_eq!(
            layout
                .monitor_at(-1, 200)
                .expect("left monitor should contain point")
                .id,
            "left"
        );
        assert_eq!(
            layout
                .monitor_at(500, -1)
                .expect("upper monitor should contain point")
                .id,
            "above"
        );
        assert!(layout.monitor_at(-500, 100).is_err());
    }

    #[test]
    fn treats_right_and_bottom_edges_as_exclusive() {
        let layout = ScreenLayout::new(vec![monitor("primary", 0, 0, 1920, 1080, true)])
            .expect("layout should be valid");

        assert!(layout.monitor_at(0, 0).is_ok());
        assert!(layout.monitor_at(1919, 1079).is_ok());
        assert!(layout.monitor_at(1920, 1079).is_err());
        assert!(layout.monitor_at(1919, 1080).is_err());
    }

    #[test]
    fn coordinate_errors_include_the_point_virtual_bounds_and_monitor_bounds() {
        let layout = ScreenLayout::new(vec![monitor("primary", 0, 0, 1920, 1080, true)])
            .expect("layout should be valid");

        let error = layout
            .monitor_at(-1, 40)
            .expect_err("point should be outside the monitor");
        assert!(error.contains("(-1, 40)"));
        assert!(error.contains("virtual bounds are left=0, top=0, right=1920, bottom=1080"));
        assert!(error.contains("primary [left=0, top=0, right=1920, bottom=1080]"));
    }

    fn monitor(
        id: &str,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        is_primary: bool,
    ) -> MonitorInfo {
        let bounds = bounds(left, top, right, bottom);
        MonitorInfo {
            bounds,
            device_name: id.to_owned(),
            dpi_x: Some(96),
            dpi_y: Some(96),
            id: id.to_owned(),
            is_primary,
            scale_factor: Some(1.0),
            work_area: bounds,
        }
    }

    fn bounds(left: i32, top: i32, right: i32, bottom: i32) -> MonitorBounds {
        MonitorBounds::new(left, top, right, bottom).expect("bounds should be valid")
    }
}
