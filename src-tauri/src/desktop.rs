use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

#[cfg(windows)]
mod win;

#[derive(Debug, Clone, Serialize)]
pub struct DesktopItem {
    pub name: String,
    pub path: Option<String>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[tauri::command]
pub fn list_desktop_items() -> Result<Vec<DesktopItem>, String> {
    let mut items = Vec::new();
    collect_desktop_dir(&mut items, user_desktop_dir()?);

    if let Some(public_desktop) = public_desktop_dir() {
        collect_desktop_dir(&mut items, public_desktop);
    }

    items.sort_by_key(|item| item.name.to_lowercase());
    items.dedup_by(|left, right| left.name.eq_ignore_ascii_case(&right.name) && left.path == right.path);
    assign_estimated_positions(&mut items);

    #[cfg(windows)]
    win::enrich_desktop_item_positions(&mut items);

    Ok(items)
}

#[tauri::command]
pub fn highlight_desktop_item(app: AppHandle, name: String, path: Option<String>) -> Result<(), String> {
    let items = list_desktop_items()?;
    let item = items
        .into_iter()
        .find(|item| {
            path.as_ref()
                .and_then(|target_path| item.path.as_ref().map(|item_path| item_path.eq_ignore_ascii_case(target_path)))
                .unwrap_or(false)
                || item.name.eq_ignore_ascii_case(&name)
        })
        .ok_or_else(|| format!("未在桌面找到：{name}"))?;

    #[cfg(windows)]
    {
        win::highlight_desktop_item(&app, &item)
    }

    #[cfg(not(windows))]
    {
        let _ = app;
        let _ = item;
        Err("仅支持 Windows 桌面图标高亮".to_string())
    }
}

fn collect_desktop_dir(items: &mut Vec<DesktopItem>, dir: PathBuf) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = display_name(&path) else {
            continue;
        };

        items.push(DesktopItem {
            name,
            path: Some(path.to_string_lossy().into_owned()),
            x: None,
            y: None,
            width: None,
            height: None,
        });
    }
}

fn assign_estimated_positions(items: &mut [DesktopItem]) {
    const START_X: i32 = 24;
    const START_Y: i32 = 24;
    const CELL_W: i32 = 96;
    const CELL_H: i32 = 92;
    const ICON_W: u32 = 88;
    const ICON_H: u32 = 76;
    const ROWS_PER_COLUMN: usize = 8;

    for (index, item) in items.iter_mut().enumerate() {
        let column = index / ROWS_PER_COLUMN;
        let row = index % ROWS_PER_COLUMN;
        item.x = Some(START_X + (column as i32 * CELL_W));
        item.y = Some(START_Y + (row as i32 * CELL_H));
        item.width = Some(ICON_W);
        item.height = Some(ICON_H);
    }
}

fn show_overlay(app: &AppHandle, item: &DesktopItem) -> Result<(), String> {
    // 留一点边距，让边框包住整个图标（含文字）。
    let padding = 8i32;
    let x = item.x.unwrap_or(24);
    let y = item.y.unwrap_or(24);
    let width = item.width.unwrap_or(88).max(24);
    let height = item.height.unwrap_or(76).max(24);
    let outer_x = x.saturating_sub(padding);
    let outer_y = y.saturating_sub(padding);
    let outer_width = width + (padding as u32 * 2);
    let outer_height = height + (padding as u32 * 2);

    // 单个透明窗 + CSS 边框，避免多窗各自带 DWM 阴影形成的“透明包围”。
    let window = app
        .get_webview_window("highlight-overlay")
        .ok_or_else(|| "高亮窗口不存在：highlight-overlay".to_string())?;
    window
        .set_size(PhysicalSize::new(outer_width, outer_height))
        .map_err(|error| format!("设置高亮窗口大小失败：{error}"))?;
    window
        .set_position(PhysicalPosition::new(outer_x, outer_y))
        .map_err(|error| format!("设置高亮窗口位置失败：{error}"))?;
    window.show().map_err(|error| format!("显示高亮窗口失败：{error}"))?;

    let hide_window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1300));
        let _ = hide_window.hide();
    });

    Ok(())
}

fn display_name(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_string_lossy();
    let name = file_name.strip_suffix(".lnk").or_else(|| file_name.strip_suffix(".url")).unwrap_or(&file_name);
    Some(name.to_string())
}

fn user_desktop_dir() -> Result<PathBuf, String> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|path| path.join("Desktop"))
        .ok_or_else(|| "无法读取 USERPROFILE 环境变量".to_string())
}

fn public_desktop_dir() -> Option<PathBuf> {
    env::var_os("PUBLIC").map(PathBuf::from).map(|path| path.join("Desktop"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_removes_shortcut_suffix() {
        assert_eq!(display_name(Path::new("微信.lnk")), Some("微信".to_string()));
        assert_eq!(display_name(Path::new("网站.url")), Some("网站".to_string()));
        assert_eq!(display_name(Path::new("资料夹")), Some("资料夹".to_string()));
    }

    #[test]
    fn assign_estimated_positions_sets_coordinates() {
        let mut items = vec![DesktopItem {
            name: "微信".to_string(),
            path: None,
            x: None,
            y: None,
            width: None,
            height: None,
        }];

        assign_estimated_positions(&mut items);

        assert_eq!(items[0].x, Some(24));
        assert_eq!(items[0].y, Some(24));
        assert_eq!(items[0].width, Some(88));
        assert_eq!(items[0].height, Some(76));
    }
}
