//! 檔案：window_placement.rs
//! 用途：小工具視窗「放在哪裡、多大」的純計算（FR-14、43「頁面版面配置」）：
//!       - 第一次開啟：主螢幕工作區（不含工作列）的右上角，距邊緣 24px。
//!       - 恢復上次位置：視窗中心點不在任何螢幕的工作區內（例如拔掉外接螢幕）時，回主螢幕右上角；
//!         只有一部分超出工作區時，整個移回工作區內。
//!       - 切換輪播／並排：換成該模式記住的大小，以右上角為基準，超出工作區就往內移。
//!       這裡不碰視窗、不碰檔案，所有數字由參數傳入，所以可以直接寫單元測試。
//!       單位一律是實體像素（螢幕真正的點數），和 state.json 一致。
//! 誰會用到：widget_window.rs（啟動時恢復位置、切換顯示方式）。

use crate::runtime_state::WindowBounds;

/// 一個螢幕的工作區（扣掉工作列之後可以放視窗的範圍）與縮放比例。
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Windows 顯示縮放：100% 為 1.0、150% 為 1.5
    pub scale_factor: f64,
}

impl ScreenArea {
    /// 工作區右邊界（不含）
    fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    /// 工作區下邊界（不含）
    fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    /// 某一點是否落在這個工作區內
    fn contains_point(&self, x: i32, y: i32) -> bool {
        let inside_horizontally = x >= self.x && x < self.right();
        let inside_vertically = y >= self.y && y < self.bottom();
        inside_horizontally && inside_vertically
    }
}

/// 把「邏輯像素」的大小（例如 360×230）換算成這個縮放比例下的實體像素。
pub fn physical_size(logical_size: (f64, f64), scale_factor: f64) -> (u32, u32) {
    let width = (logical_size.0 * scale_factor).round() as u32;
    let height = (logical_size.1 * scale_factor).round() as u32;
    (width, height)
}

/// 放在工作區右上角、距邊緣 margin（邏輯像素，會乘上縮放比例）。
pub fn top_right_bounds(area: &ScreenArea, size: (u32, u32), margin_logical: f64) -> WindowBounds {
    let margin = (margin_logical * area.scale_factor).round() as i32;
    let bounds = WindowBounds {
        x: area.right() - size.0 as i32 - margin,
        y: area.y + margin,
        width: size.0,
        height: size.1,
    };
    // 工作區很小（例如直式小螢幕）時，仍保證整個視窗在工作區內
    clamp_into_area(bounds, area)
}

/// 找出視窗中心點所在的工作區；中心點不在任何工作區內就回傳 None。
pub fn area_containing_center<'a>(
    bounds: &WindowBounds,
    areas: &'a [ScreenArea],
) -> Option<&'a ScreenArea> {
    let center_x = bounds.x + (bounds.width / 2) as i32;
    let center_y = bounds.y + (bounds.height / 2) as i32;
    // 逐一檢查每個工作區，回傳第一個包含中心點的
    areas
        .iter()
        .find(|area| area.contains_point(center_x, center_y))
}

/// 把視窗整個移進工作區內：比工作區大就先縮成工作區大小，再把超出的部分往內推。
pub fn clamp_into_area(bounds: WindowBounds, area: &ScreenArea) -> WindowBounds {
    let width = bounds.width.min(area.width);
    let height = bounds.height.min(area.height);

    // 水平：先處理右邊超出，再處理左邊超出（左邊優先，確保標題列那側看得到）
    let mut x = bounds.x;
    if x + width as i32 > area.right() {
        x = area.right() - width as i32;
    }
    if x < area.x {
        x = area.x;
    }

    // 垂直：同樣先處理下方超出，再處理上方超出
    let mut y = bounds.y;
    if y + height as i32 > area.bottom() {
        y = area.bottom() - height as i32;
    }
    if y < area.y {
        y = area.y;
    }

    WindowBounds {
        x,
        y,
        width,
        height,
    }
}

/// 程式啟動時決定視窗位置大小。
/// 輸入：這個模式上次記住的位置大小（沒有就是 None）、目前所有螢幕的工作區、主螢幕工作區、
///       這個模式的預設大小（邏輯像素）、距螢幕邊緣的距離（邏輯像素）。
pub fn restore_bounds(
    saved: Option<WindowBounds>,
    areas: &[ScreenArea],
    primary: &ScreenArea,
    default_logical_size: (f64, f64),
    margin_logical: f64,
) -> WindowBounds {
    if let Some(saved_bounds) = saved {
        // 上次的位置還在某個螢幕上：沿用，只把超出工作區的部分推回來
        if let Some(area) = area_containing_center(&saved_bounds, areas) {
            return clamp_into_area(saved_bounds, area);
        }
        // 走到這裡代表那個螢幕已經不在（或位置跑到所有螢幕外）：回主螢幕右上角，大小沿用
        let saved_size = (saved_bounds.width, saved_bounds.height);
        return top_right_bounds(primary, saved_size, margin_logical);
    }

    // 第一次開啟：主螢幕右上角＋預設大小
    let size = physical_size(default_logical_size, primary.scale_factor);
    top_right_bounds(primary, size, margin_logical)
}

/// ⇄ 切換顯示方式：以目前的右上角為基準換成新大小，超出工作區就往內移。
/// 用「切換前」視窗所在的螢幕為準；找不到時用主螢幕。
pub fn switch_mode_bounds(
    current: WindowBounds,
    new_size: (u32, u32),
    areas: &[ScreenArea],
    primary: &ScreenArea,
) -> WindowBounds {
    let right_edge = current.x + current.width as i32;
    let anchored = WindowBounds {
        x: right_edge - new_size.0 as i32,
        y: current.y,
        width: new_size.0,
        height: new_size.1,
    };

    let area = match area_containing_center(&current, areas) {
        Some(found) => found,
        None => primary,
    };
    clamp_into_area(anchored, area)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1920×1080、工作列在下方 40px 的主螢幕（工作區 1920×1040），縮放 100%
    fn primary_area() -> ScreenArea {
        ScreenArea {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
            scale_factor: 1.0,
        }
    }

    /// 放在主螢幕左邊的外接螢幕（座標是負的）
    fn left_area() -> ScreenArea {
        ScreenArea {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
        }
    }

    fn bounds(x: i32, y: i32, width: u32, height: u32) -> WindowBounds {
        WindowBounds {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn first_launch_goes_to_primary_top_right_with_margin() {
        let result = restore_bounds(
            None,
            &[primary_area()],
            &primary_area(),
            (360.0, 230.0),
            24.0,
        );
        assert_eq!(result, bounds(1920 - 360 - 24, 24, 360, 230));
    }

    #[test]
    fn first_launch_scales_size_and_margin_with_windows_zoom() {
        let zoomed = ScreenArea {
            x: 0,
            y: 0,
            width: 2880,
            height: 1560,
            scale_factor: 1.5,
        };
        let areas = [zoomed.clone()];
        let result = restore_bounds(None, &areas, &zoomed, (360.0, 230.0), 24.0);
        assert_eq!(result, bounds(2880 - 540 - 36, 36, 540, 345));
    }

    #[test]
    fn taskbar_on_top_moves_the_work_area_down() {
        let area = ScreenArea {
            x: 0,
            y: 48,
            width: 1920,
            height: 1032,
            scale_factor: 1.0,
        };
        let areas = [area.clone()];
        let result = restore_bounds(None, &areas, &area, (360.0, 230.0), 24.0);
        assert_eq!(result.y, 48 + 24);
    }

    #[test]
    fn saved_position_inside_a_screen_is_kept() {
        let saved = bounds(200, 300, 400, 250);
        let areas = [primary_area(), left_area()];
        let result = restore_bounds(Some(saved), &areas, &primary_area(), (360.0, 230.0), 24.0);
        assert_eq!(result, saved);
    }

    #[test]
    fn saved_position_on_a_left_monitor_with_negative_coordinates_is_kept() {
        let saved = bounds(-800, 100, 360, 230);
        let areas = [primary_area(), left_area()];
        let result = restore_bounds(Some(saved), &areas, &primary_area(), (360.0, 230.0), 24.0);
        assert_eq!(result, saved);
    }

    #[test]
    fn unplugged_monitor_falls_back_to_primary_top_right_keeping_size() {
        // 上次放在左邊外接螢幕，這次只剩主螢幕
        let saved = bounds(-800, 100, 400, 250);
        let result = restore_bounds(
            Some(saved),
            &[primary_area()],
            &primary_area(),
            (360.0, 230.0),
            24.0,
        );
        assert_eq!(result, bounds(1920 - 400 - 24, 24, 400, 250));
    }

    #[test]
    fn partly_outside_window_is_pushed_back_inside() {
        // 中心點在主螢幕內，但右下角超出工作區
        let saved = bounds(1700, 900, 360, 230);
        let result = restore_bounds(
            Some(saved),
            &[primary_area()],
            &primary_area(),
            (360.0, 230.0),
            24.0,
        );
        assert_eq!(result, bounds(1920 - 360, 1040 - 230, 360, 230));
    }

    #[test]
    fn window_larger_than_the_work_area_is_shrunk() {
        let result = clamp_into_area(bounds(-50, -50, 3000, 2000), &primary_area());
        assert_eq!(result, bounds(0, 0, 1920, 1040));
    }

    #[test]
    fn switching_mode_keeps_the_top_right_corner() {
        let carousel = bounds(1536, 24, 360, 230);
        let result = switch_mode_bounds(carousel, (640, 240), &[primary_area()], &primary_area());
        assert_eq!(result, bounds(1896 - 640, 24, 640, 240));
    }

    #[test]
    fn switching_mode_near_the_left_edge_moves_inside() {
        let carousel = bounds(10, 24, 360, 230);
        let result = switch_mode_bounds(carousel, (640, 240), &[primary_area()], &primary_area());
        assert_eq!(result, bounds(0, 24, 640, 240));
    }

    #[test]
    fn switching_mode_near_the_bottom_moves_up() {
        let overview = bounds(1000, 900, 640, 120);
        let result = switch_mode_bounds(overview, (360, 230), &[primary_area()], &primary_area());
        assert_eq!(result, bounds(1640 - 360, 1040 - 230, 360, 230));
    }

    #[test]
    fn switching_mode_uses_the_monitor_the_window_is_on() {
        let on_left_monitor = bounds(-400, 24, 360, 230);
        let areas = [primary_area(), left_area()];
        let result = switch_mode_bounds(on_left_monitor, (640, 240), &areas, &primary_area());
        // 右邊界 -40 維持不變，左邊延伸到 -680，仍在左邊螢幕內
        assert_eq!(result, bounds(-680, 24, 640, 240));
    }
}
