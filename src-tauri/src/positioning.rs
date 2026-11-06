use tauri::{AppHandle, PhysicalPosition, PhysicalRect, PhysicalSize, Position, WebviewWindow};

fn clamp_to_work_area(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    area: &PhysicalRect<i32, u32>,
) -> PhysicalPosition<i32> {
    let max_x = area.position.x + (area.size.width as i32 - size.width as i32).max(0);
    let max_y = area.position.y + (area.size.height as i32 - size.height as i32).max(0);
    PhysicalPosition {
        x: position.x.clamp(area.position.x, max_x),
        y: position.y.clamp(area.position.y, max_y),
    }
}

fn bottom_aligned_position(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    area: &PhysicalRect<i32, u32>,
) -> PhysicalPosition<i32> {
    clamp_to_work_area(
        PhysicalPosition {
            x: position.x,
            y: area.position.y + area.size.height.saturating_sub(size.height) as i32,
        },
        size,
        area,
    )
}

/// Keep the panel's bottom edge at the work-area boundary as content grows or
/// shrinks, so collapsing HDR strength controls does not leave a taskbar gap.
pub fn keep_window_in_work_area(window: &WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let area = monitor.work_area();
    let position = window.outer_position()?;
    let mut size = window.outer_size()?;
    if size.height > area.size.height {
        size.height = area.size.height;
        window.set_size(size)?;
    }
    let clamped = bottom_aligned_position(position, size, area);
    if clamped != position {
        window.set_position(clamped)?;
    }
    Ok(())
}

fn window_size_px(window: &WebviewWindow) -> (i32, i32) {
    let size = window.outer_size().ok();
    let width_px = size.as_ref().map(|s| s.width as i32).unwrap_or(300);
    let height_px = size.as_ref().map(|s| s.height as i32).unwrap_or(600);

    (width_px, height_px)
}

pub fn compute_window_position_for_tray_click(
    app: &AppHandle,
    window: &WebviewWindow,
    rect_position: Position,
) -> Position {
    // Determine current window size in physical pixels
    let (width_px, height_px) = window_size_px(window);

    // Compute click position in physical pixels
    let (click_x, click_y): (i32, i32) = match rect_position {
        Position::Physical(p) => (p.x, p.y),
        Position::Logical(p) => {
            let scale = window.scale_factor().unwrap_or(1.0);
            ((p.x * scale).round() as i32, (p.y * scale).round() as i32)
        }
    };

    // Position the window so it appears above the tray area (align left edge to click)
    let mut x_px = click_x;
    let mut y_px = click_y - height_px;

    // Clamp to the monitor that contains the click to avoid jumping across screens
    if let Ok(monitors) = app.available_monitors() {
        if let Some(m) = monitors.into_iter().find(|m| {
            let mp = m.position();
            let ms = m.size();
            click_x >= mp.x
                && click_x < mp.x + ms.width as i32
                && click_y >= mp.y
                && click_y < mp.y + ms.height as i32
        }) {
            let position = clamp_to_work_area(
                PhysicalPosition { x: x_px, y: y_px },
                PhysicalSize::new(width_px as u32, height_px as u32),
                m.work_area(),
            );
            x_px = position.x;
            y_px = position.y;
        }
    }

    Position::Physical(PhysicalPosition { x: x_px, y: y_px })
}

pub fn is_position_on_any_monitor(app: &AppHandle, position: PhysicalPosition<i32>) -> bool {
    app.available_monitors()
        .ok()
        .map(|monitors| {
            monitors.into_iter().any(|monitor| {
                let monitor_position = monitor.position();
                let monitor_size = monitor.size();

                position.x >= monitor_position.x
                    && position.x < monitor_position.x + monitor_size.width as i32
                    && position.y >= monitor_position.y
                    && position.y < monitor_position.y + monitor_size.height as i32
            })
        })
        .unwrap_or(false)
}

pub fn compute_default_window_position(app: &AppHandle, window: &WebviewWindow) -> Position {
    let (width_px, height_px) = window_size_px(window);
    let margin_px = 16;

    let monitor = app.primary_monitor().ok().flatten().or_else(|| {
        app.available_monitors()
            .ok()
            .and_then(|monitors| monitors.into_iter().next())
    });

    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let x_px = area.position.x + (area.size.width as i32 - width_px - margin_px).max(0);
        let y_px = area.position.y + (area.size.height as i32 - height_px - margin_px).max(0);

        Position::Physical(PhysicalPosition { x: x_px, y: y_px })
    } else {
        Position::Physical(PhysicalPosition { x: 0, y: 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanding_and_collapsing_panel_stays_against_bottom_taskbar() {
        let work = PhysicalRect {
            position: PhysicalPosition::new(0, 0),
            size: PhysicalSize::new(3072, 1848),
        };
        let collapsed_size = PhysicalSize::new(720, 1308);
        let expanded_size = PhysicalSize::new(720, 1470);
        let mut position = PhysicalPosition::new(2352, 540);

        for _ in 0..3 {
            position = bottom_aligned_position(position, expanded_size, &work);
            assert_eq!(position, PhysicalPosition::new(2352, 378));
            assert_eq!(position.y + expanded_size.height as i32, 1848);

            position = bottom_aligned_position(position, collapsed_size, &work);
            assert_eq!(position, PhysicalPosition::new(2352, 540));
            assert_eq!(position.y + collapsed_size.height as i32, 1848);
        }
    }

    #[test]
    fn bottom_alignment_respects_offset_monitor_and_oversized_content() {
        let work = PhysicalRect {
            position: PhysicalPosition::new(-1920, -1032),
            size: PhysicalSize::new(1920, 984),
        };
        let position = PhysicalPosition::new(-100, -900);
        assert_eq!(
            bottom_aligned_position(position, PhysicalSize::new(360, 700), &work),
            PhysicalPosition::new(-360, -748),
        );
        assert_eq!(
            bottom_aligned_position(position, PhysicalSize::new(360, 1200), &work),
            PhysicalPosition::new(-360, -1032),
        );
    }

    #[test]
    fn respects_top_taskbar_and_negative_monitor_coordinates() {
        let work = PhysicalRect {
            position: PhysicalPosition::new(-1920, 48),
            size: PhysicalSize::new(1920, 1032),
        };
        assert_eq!(
            clamp_to_work_area(
                PhysicalPosition::new(-100, -20),
                PhysicalSize::new(360, 700),
                &work
            ),
            PhysicalPosition::new(-360, 48),
        );
        assert_eq!(
            clamp_to_work_area(
                PhysicalPosition::new(-1920, 48),
                PhysicalSize::new(360, 1200),
                &work
            ),
            PhysicalPosition::new(-1920, 48),
        );
    }
}
