//! 檔案：logging.rs
//! 用途：把程式事件寫入每日一檔的 log（ai-usage-board-YYYY-MM-DD.log），
//!       並在「程式啟動時」與「每天第一次寫 log 時」清理超過保留天數的舊檔（只清本程式命名的檔案）。
//!       保留天數可以在執行中更改（設定畫面按儲存後立即生效）。
//!       另外負責：程式發生未預期錯誤（panic）時，把原因寫進 log 再結束。
//! 誰會用到：lib.rs 啟動流程、settings_commands.rs（儲存設定後更新保留天數）；
//!           所有 Rust 模組透過 log::info!／log::warn! 寫入。

use chrono::{Local, NaiveDate};
use log::{LevelFilter, Log, Metadata, Record};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};

const LOG_FILE_PREFIX: &str = "ai-usage-board-";
const LOG_FILE_SUFFIX: &str = ".log";

/// 目前的 log 保留天數（設定 general.log_retention_days）；設定畫面儲存後會更新。
static RETENTION_DAYS: AtomicI64 = AtomicI64::new(7);
/// logs 資料夾位置（initialize 時記下，更改保留天數時要馬上清理用）。
static LOGS_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

struct OpenLogFile {
    date: NaiveDate,
    file: File,
}

struct DailyFileLogger {
    logs_directory: PathBuf,
    open_file: Mutex<Option<OpenLogFile>>,
    level: LevelFilter,
}

#[derive(Debug)]
pub enum LoggingError {
    Io(std::io::Error),
    SetLogger(log::SetLoggerError),
}

impl Display for LoggingError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "無法準備 log：{error}"),
            Self::SetLogger(error) => write!(formatter, "無法啟用 log：{error}"),
        }
    }
}

impl Error for LoggingError {}

impl From<std::io::Error> for LoggingError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl Log for DailyFileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let now = Local::now();
        let today = now.date_naive();
        let Ok(mut open_file) = self.open_file.lock() else {
            return;
        };

        let needs_new_file = open_file
            .as_ref()
            .is_none_or(|current_file| current_file.date != today);

        if needs_new_file {
            // 換日（或第一次寫 log）時順便清理過期的 log 檔
            let retention_days = RETENTION_DAYS.load(Ordering::Relaxed);
            let _ = remove_expired_log_files(&self.logs_directory, retention_days);
            let Ok(file) = open_log_file(&self.logs_directory, today) else {
                return;
            };
            *open_file = Some(OpenLogFile { date: today, file });
        }

        if let Some(current_file) = open_file.as_mut() {
            let timestamp = now.format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(
                current_file.file,
                "{timestamp} [{:<5}] {}",
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Ok(mut open_file) = self.open_file.lock() {
            if let Some(current_file) = open_file.as_mut() {
                let _ = current_file.file.flush();
            }
        }
    }
}

/// 啟用每日檔案 log，並依設定的天數移除專用 logs 資料夾內的過期檔案。
pub fn initialize(logs_directory: &Path, retention_days: i64) -> Result<(), LoggingError> {
    fs::create_dir_all(logs_directory)?;
    RETENTION_DAYS.store(retention_days, Ordering::Relaxed);
    let _ = LOGS_DIRECTORY.set(logs_directory.to_path_buf());
    remove_expired_log_files(logs_directory, retention_days)?;

    let logger = DailyFileLogger {
        logs_directory: logs_directory.to_path_buf(),
        open_file: Mutex::new(None),
        level: LevelFilter::Info,
    };

    log::set_boxed_logger(Box::new(logger)).map_err(LoggingError::SetLogger)?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}

/// 程式發生未預期的錯誤（panic）時，先把錯誤內容與發生位置寫進 log，再照原本的方式結束。
/// 為什麼需要：release 版設定 panic = "abort"、又沒有主控台視窗，不寫進 log 的話，
/// 使用者只會看到小工具突然消失，查不到原因（22 踩過的坑：tao 的 flush_paint_messages 斷言）。
pub fn install_panic_hook() {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        log::error!("程式發生未預期的錯誤，即將結束：{panic_info}");
        log::logger().flush();
        previous_hook(panic_info);
    }));
}

/// 設定畫面改了 log 保留天數：記下新天數，並馬上清掉超過新天數的舊檔。
pub fn set_retention_days(retention_days: i64) {
    RETENTION_DAYS.store(retention_days, Ordering::Relaxed);
    let Some(logs_directory) = LOGS_DIRECTORY.get() else {
        return;
    };
    match remove_expired_log_files(logs_directory, retention_days) {
        Ok(()) => log::info!("log 保留天數改為 {retention_days} 天"),
        Err(error) => log::warn!("清理過期 log 失敗：{error}"),
    }
}

fn open_log_file(logs_directory: &Path, date: NaiveDate) -> Result<File, std::io::Error> {
    let file_name = format!(
        "{LOG_FILE_PREFIX}{}{LOG_FILE_SUFFIX}",
        date.format("%Y-%m-%d")
    );
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs_directory.join(file_name))
}

/// 刪除超過保留天數的 log 檔；保留天數 7 代表保留今天在內的最近 7 天。
fn remove_expired_log_files(
    logs_directory: &Path,
    retention_days: i64,
) -> Result<(), std::io::Error> {
    let today = Local::now().date_naive();

    for directory_entry in fs::read_dir(logs_directory)? {
        let directory_entry = directory_entry?;
        let path = directory_entry.path();

        // 只處理由本程式命名且日期可正確解析的檔案，避免誤刪其他內容。
        let Some(file_date) = log_file_date(&path) else {
            continue;
        };

        let age_in_days = today.signed_duration_since(file_date).num_days();
        if age_in_days >= retention_days {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn log_file_date(path: &Path) -> Option<NaiveDate> {
    let file_name = path.file_name()?.to_str()?;
    let date_text = file_name
        .strip_prefix(LOG_FILE_PREFIX)?
        .strip_suffix(LOG_FILE_SUFFIX)?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::{log_file_date, remove_expired_log_files};
    use chrono::{Duration, Local, NaiveDate};
    use std::path::Path;

    #[test]
    fn removes_only_expired_app_log_files() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系統時間應有效")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("ai-usage-logs-{unique}"));
        std::fs::create_dir_all(&directory).expect("應建立測試資料夾");
        let today = Local::now().date_naive();
        let file_for = |days_ago: i64| {
            let date = today - Duration::days(days_ago);
            directory.join(format!("ai-usage-board-{}.log", date.format("%Y-%m-%d")))
        };
        for days_ago in [0, 6, 7, 30] {
            std::fs::write(file_for(days_ago), "log").expect("應建立測試 log");
        }
        let unrelated = directory.join("notes.txt");
        std::fs::write(&unrelated, "keep").expect("應建立其他檔案");

        remove_expired_log_files(&directory, 7).expect("清理應成功");

        assert!(file_for(0).exists());
        assert!(file_for(6).exists());
        assert!(!file_for(7).exists());
        assert!(!file_for(30).exists());
        assert!(unrelated.exists(), "不是本程式命名的檔案不能刪");
        std::fs::remove_dir_all(directory).expect("應清除測試資料");
    }

    #[test]
    fn only_accepts_the_app_log_file_name() {
        let date = log_file_date(Path::new("ai-usage-board-2026-09-18.log"));
        assert_eq!(date, NaiveDate::from_ymd_opt(2026, 9, 18));
        assert!(log_file_date(Path::new("other-2026-09-18.log")).is_none());
    }
}
