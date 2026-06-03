use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScheduleItem {
    pub id: String,
    pub title: String,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub status: String,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateScheduleInput {
    pub title: String,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateScheduleInput {
    pub id: String,
    pub title: String,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub status: String,
    pub note: String,
}

#[tauri::command]
pub fn list_schedules(date: Option<String>) -> Result<Vec<ScheduleItem>, String> {
    let mut schedules = read_schedules()?;
    if let Some(date) = date {
        schedules.retain(|item| item.date == date);
    }
    schedules.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then(left.start_time.cmp(&right.start_time))
            .then(left.title.cmp(&right.title))
    });
    Ok(schedules)
}

#[tauri::command]
pub fn create_schedule(input: CreateScheduleInput) -> Result<ScheduleItem, String> {
    validate_required(&input.title, "标题")?;
    validate_required(&input.date, "日期")?;

    let mut schedules = read_schedules()?;
    let item = ScheduleItem {
        id: generate_id(),
        title: input.title.trim().to_string(),
        date: input.date,
        start_time: input.start_time,
        end_time: input.end_time,
        status: "todo".to_string(),
        note: input.note.unwrap_or_default(),
    };
    schedules.push(item.clone());
    write_schedules(&schedules)?;
    Ok(item)
}

#[tauri::command]
pub fn update_schedule(input: UpdateScheduleInput) -> Result<ScheduleItem, String> {
    validate_required(&input.id, "ID")?;
    validate_required(&input.title, "标题")?;
    validate_required(&input.date, "日期")?;

    let mut schedules = read_schedules()?;
    let Some(item) = schedules.iter_mut().find(|item| item.id == input.id) else {
        return Err(format!("未找到日程：{}", input.id));
    };

    item.title = input.title.trim().to_string();
    item.date = input.date;
    item.start_time = input.start_time;
    item.end_time = input.end_time;
    item.status = normalize_status(&input.status);
    item.note = input.note;
    let updated = item.clone();

    write_schedules(&schedules)?;
    Ok(updated)
}

#[tauri::command]
pub fn delete_schedule(id: String) -> Result<(), String> {
    validate_required(&id, "ID")?;
    let mut schedules = read_schedules()?;
    let original_len = schedules.len();
    schedules.retain(|item| item.id != id);

    if schedules.len() == original_len {
        return Err(format!("未找到日程：{id}"));
    }

    write_schedules(&schedules)
}

#[tauri::command]
pub fn toggle_schedule_done(id: String) -> Result<ScheduleItem, String> {
    validate_required(&id, "ID")?;
    let mut schedules = read_schedules()?;
    let Some(item) = schedules.iter_mut().find(|item| item.id == id) else {
        return Err(format!("未找到日程：{id}"));
    };

    item.status = if item.status == "done" { "todo" } else { "done" }.to_string();
    let updated = item.clone();
    write_schedules(&schedules)?;
    Ok(updated)
}

fn validate_required(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field}不能为空"))
    } else {
        Ok(())
    }
}

fn normalize_status(status: &str) -> String {
    if status == "done" {
        "done".to_string()
    } else {
        "todo".to_string()
    }
}

fn read_schedules() -> Result<Vec<ScheduleItem>, String> {
    let path = schedule_file_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path).map_err(|error| format!("读取日程文件失败：{error}"))?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&content).map_err(|error| format!("解析日程文件失败：{error}"))
}

fn write_schedules(schedules: &[ScheduleItem]) -> Result<(), String> {
    let path = schedule_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建日程目录失败：{error}"))?;
    }

    let content = serde_json::to_string_pretty(schedules).map_err(|error| format!("序列化日程失败：{error}"))?;
    fs::write(&path, content).map_err(|error| format!("写入日程文件失败：{error}"))
}

fn schedule_file_path() -> Result<PathBuf, String> {
    let appdata = env::var_os("APPDATA").ok_or_else(|| "无法读取 APPDATA 环境变量".to_string())?;
    Ok(PathBuf::from(appdata).join("Desktop Find").join("schedules.json"))
}

fn generate_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("schedule-{millis}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_status_rejects_unknown_status() {
        assert_eq!(normalize_status("done"), "done");
        assert_eq!(normalize_status("anything"), "todo");
    }

    #[test]
    fn validate_required_rejects_empty_title() {
        assert!(validate_required("", "标题").is_err());
        assert!(validate_required("写代码", "标题").is_ok());
    }
}
