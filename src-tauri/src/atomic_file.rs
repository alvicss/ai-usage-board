//! 檔案：atomic_file.rs
//! 用途：「先寫暫存檔、再改名蓋過正式檔」的安全寫檔工具，避免別人讀到寫到一半的檔案。
//! 誰會用到：cache.rs（cache.json）、runtime_state.rs（state.json）、settings/writer.rs（settings.toml）。

use std::fs;
use std::io::Write;
use std::path::Path;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

/// 把 bytes 安全寫進 path。
/// 輸入：目標檔案路徑、完整內容。
/// 做法：先寫到同資料夾的「原檔名.tmp」，確實存到磁碟後，再一次改名蓋過正式檔。
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut temporary_name = path.as_os_str().to_os_string();
    temporary_name.push(".tmp");
    let temporary_path = Path::new(&temporary_name);

    let mut file = fs::File::create(temporary_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    // 先關閉檔案，Windows 才允許改名
    drop(file);

    replace_file(temporary_path, path)
}

/// Windows：用 MoveFileExW 一次完成「改名＋蓋過舊檔」。
#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| std::io::Error::other(error.to_string()))
}

/// 其他系統：rename 本身就會蓋過舊檔。
#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::write_atomic;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn replaces_existing_file_and_leaves_no_temporary_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系統時間應有效")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("ai-usage-atomic-{unique}"));
        std::fs::create_dir_all(&directory).expect("應建立測試資料夾");
        let path = directory.join("data.json");

        write_atomic(&path, b"first").expect("第一次寫入應成功");
        write_atomic(&path, b"second").expect("第二次寫入應蓋過舊檔");

        let text = std::fs::read_to_string(&path).expect("應可讀回");
        assert_eq!(text, "second");
        assert!(!directory.join("data.json.tmp").exists());
        std::fs::remove_dir_all(directory).expect("應清除測試資料");
    }
}
