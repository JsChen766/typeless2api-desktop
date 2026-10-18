#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Point {
    pub x: f64,
    pub y: f64,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Area {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[cfg(target_os = "macos")]
pub(crate) fn settings_position(
    area: Area,
    anchor_x: Option<f64>,
    width: f64,
    height: f64,
) -> Point {
    let center_x = anchor_x.unwrap_or(area.left + area.width / 2.0);
    Point {
        x: (center_x - width / 2.0).clamp(area.left, (area.left + area.width - width).max(area.left)),
        y: (area.top + area.height - height).max(area.top),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn pill_position(display: Area, visible: Area, width: f64, height: f64) -> Point {
    let visible_bottom = visible.top + visible.height - 24.0;
    let screen_bottom = display.top + display.height - 96.0;
    Point {
        x: visible.left + ((visible.width - width) / 2.0).max(0.0),
        y: (visible_bottom.min(screen_bottom) - height).max(visible.top + 8.0),
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn settings_is_aligned_above_dock_icon() {
        let area = Area {
            left: 0.0,
            top: 24.0,
            width: 1512.0,
            height: 890.0,
        };
        let position = settings_position(area, Some(800.0), 340.0, 580.0);
        assert_eq!(position, Point { x: 630.0, y: 334.0 });
        assert!(position.y + 580.0 <= area.top + area.height);
    }

    #[test]
    fn settings_bottom_touches_the_dock_edge() {
        let area = Area {
            left: 0.0,
            top: 24.0,
            width: 1512.0,
            height: 890.0,
        };
        let position = settings_position(area, Some(800.0), 340.0, 580.0);
        assert_eq!(position.y + 580.0, area.top + area.height);
    }

    #[test]
    fn settings_stays_on_screen_for_edge_dock_icons() {
        let area = Area { left: 100.0, top: 20.0, width: 1000.0, height: 700.0 };
        assert_eq!(settings_position(area, Some(110.0), 340.0, 580.0).x, 100.0);
        assert_eq!(settings_position(area, Some(1090.0), 340.0, 580.0).x, 760.0);
    }

    #[test]
    fn pill_stays_above_the_dock() {
        let display = Area {
            left: 0.0,
            top: 0.0,
            width: 1512.0,
            height: 982.0,
        };
        let visible = Area {
            left: 0.0,
            top: 24.0,
            width: 1512.0,
            height: 890.0,
        };
        let position = pill_position(display, visible, 280.0, 72.0);
        assert!(position.y + 72.0 + 16.0 <= visible.top + visible.height);
        assert!(position.y + 72.0 + 96.0 <= display.top + display.height);
    }
}
