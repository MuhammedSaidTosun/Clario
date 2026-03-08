pub mod pdf_reader;

use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const TEMP_DIR_NAME: &str = "temp";
const CACHE_DIR_NAME: &str = "cache";
const LOGS_DIR_NAME: &str = "logs";
const LOG_FILE_NAME: &str = "app.log";

#[derive(Debug)]
struct InternalDirs {
    temp_dir: PathBuf,
    cache_dir: PathBuf,
    log_file: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct AppDirs {
    temp_dir: String,
    cache_dir: String,
    log_file: String,
}

impl From<InternalDirs> for AppDirs {
    fn from(value: InternalDirs) -> Self {
        Self {
            temp_dir: value.temp_dir.to_string_lossy().into_owned(),
            cache_dir: value.cache_dir.to_string_lossy().into_owned(),
            log_file: value.log_file.to_string_lossy().into_owned(),
        }
    }
}

fn internal_dirs(app: &AppHandle) -> Result<InternalDirs, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app_data_dir: {error}"))?;

    let temp_dir = app_data_dir.join(TEMP_DIR_NAME);
    let cache_dir = app_data_dir.join(CACHE_DIR_NAME);
    let logs_dir = app_data_dir.join(LOGS_DIR_NAME);
    let log_file = logs_dir.join(LOG_FILE_NAME);

    Ok(InternalDirs {
        temp_dir,
        cache_dir,
        log_file,
    })
}

fn create_app_dirs(dirs: &InternalDirs) -> Result<(), String> {
    fs::create_dir_all(&dirs.temp_dir).map_err(|error| {
        format!(
            "failed to create temp dir '{}': {error}",
            dirs.temp_dir.display()
        )
    })?;
    fs::create_dir_all(&dirs.cache_dir).map_err(|error| {
        format!(
            "failed to create cache dir '{}': {error}",
            dirs.cache_dir.display()
        )
    })?;

    if let Some(logs_dir) = dirs.log_file.parent() {
        fs::create_dir_all(logs_dir).map_err(|error| {
            format!(
                "failed to create logs dir '{}': {error}",
                logs_dir.display()
            )
        })?;
    }

    Ok(())
}

#[tauri::command]
pub fn ping() -> String {
    "pong".to_string()
}

#[tauri::command]
pub fn app_dirs(app: AppHandle) -> Result<AppDirs, String> {
    internal_dirs(&app).map(AppDirs::from)
}

#[tauri::command]
pub fn ensure_app_dirs(app: AppHandle) -> Result<(), String> {
    let dirs = internal_dirs(&app)?;
    create_app_dirs(&dirs)?;
    Ok(())
}

#[tauri::command]
pub fn log_info(app: AppHandle, message: String) -> Result<(), String> {
    let dirs = internal_dirs(&app)?;
    create_app_dirs(&dirs)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("failed to resolve system time: {error}"))?
        .as_secs();

    let mut log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&dirs.log_file)
        .map_err(|error| {
            format!(
                "failed to open log file '{}': {error}",
                dirs.log_file.display()
            )
        })?;

    writeln!(log_file, "[{timestamp}] INFO {message}")
        .map_err(|error| format!("failed to write log line: {error}"))?;

    Ok(())
}
