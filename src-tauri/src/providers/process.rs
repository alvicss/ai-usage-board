//! 檔案：providers/process.rs
//! 用途：不經 shell 啟動官方 CLI、逐行讀取輸出、限制 stderr，並用 Job Object 清理整棵行程樹。
//! 誰會用到：Claude、Codex、Antigravity provider。

use std::fmt::{Display, Formatter};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
#[cfg(windows)]
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

const STDERR_LIMIT_BYTES: usize = 2_048;
const GRACEFUL_EXIT_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub enum ProcessError {
    Io(std::io::Error),
    StreamUnavailable(&'static str),
    Timeout,
    Windows(String),
}

impl Display for ProcessError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "CLI 行程錯誤：{error}"),
            Self::StreamUnavailable(stream) => write!(formatter, "無法使用 CLI 的 {stream}"),
            Self::Timeout => write!(formatter, "CLI 查詢逾時"),
            Self::Windows(message) => write!(formatter, "Windows 行程控制失敗：{message}"),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<std::io::Error> for ProcessError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub struct ProcessCompletion {
    pub status: ExitStatus,
    pub stderr: String,
}

pub struct ManagedProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout_receiver: mpsc::Receiver<String>,
    stderr_bytes: Arc<Mutex<Vec<u8>>>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<()>>,
    #[cfg(windows)]
    job: JobObject,
}

impl ManagedProcess {
    pub fn spawn(
        program: &Path,
        arguments: &[&str],
        environment: &[(&str, &str)],
    ) -> Result<Self, ProcessError> {
        let mut command = Command::new(program);
        command
            .args(arguments)
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (name, value) in environment {
            command.env(name, value);
        }

        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW.0);

        let mut child = command.spawn()?;

        #[cfg(windows)]
        let job = match JobObject::create_and_assign(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                return Err(error);
            }
        };

        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or(ProcessError::StreamUnavailable("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(ProcessError::StreamUnavailable("stderr"))?;

        let (stdout_sender, stdout_receiver) = mpsc::channel();
        let stdout_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line_bytes = Vec::new();

            loop {
                line_bytes.clear();
                let Ok(read_count) = reader.read_until(b'\n', &mut line_bytes) else {
                    break;
                };
                if read_count == 0 {
                    break;
                }

                while matches!(line_bytes.last(), Some(b'\n' | b'\r')) {
                    line_bytes.pop();
                }

                let line = String::from_utf8_lossy(&line_bytes).into_owned();
                if stdout_sender.send(line).is_err() {
                    break;
                }
            }
        });

        let stderr_bytes = Arc::new(Mutex::new(Vec::new()));
        let stderr_target = Arc::clone(&stderr_bytes);
        let stderr_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut chunk = [0_u8; 512];

            while let Ok(read_count) = reader.read(&mut chunk) {
                if read_count == 0 {
                    break;
                }

                let Ok(mut collected) = stderr_target.lock() else {
                    break;
                };
                let remaining = STDERR_LIMIT_BYTES.saturating_sub(collected.len());
                let copy_count = remaining.min(read_count);
                collected.extend_from_slice(&chunk[..copy_count]);
            }
        });

        Ok(Self {
            child,
            stdin,
            stdout_receiver,
            stderr_bytes,
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            #[cfg(windows)]
            job,
        })
    }

    pub fn write_line(&mut self, line: &str) -> Result<(), ProcessError> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or(ProcessError::StreamUnavailable("stdin"))?;
        stdin.write_all(line.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(())
    }

    pub fn close_stdin(&mut self) {
        self.stdin.take();
    }

    pub fn receive_line_until(&self, deadline: Instant) -> Result<Option<String>, ProcessError> {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err(ProcessError::Timeout);
        };

        match self.stdout_receiver.recv_timeout(remaining) {
            Ok(line) => Ok(Some(line)),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ProcessError::Timeout),
        }
    }

    pub fn finish(mut self) -> Result<ProcessCompletion, ProcessError> {
        self.close_stdin();
        let wait_deadline = Instant::now() + GRACEFUL_EXIT_WAIT;

        let status = loop {
            if let Some(status) = self.child.try_wait()? {
                break status;
            }

            if Instant::now() >= wait_deadline {
                #[cfg(windows)]
                self.job.terminate()?;
                #[cfg(not(windows))]
                self.child.kill()?;
                break self.child.wait()?;
            }

            thread::sleep(Duration::from_millis(25));
        };

        self.join_reader_threads();
        let stderr = self.stderr_text();
        Ok(ProcessCompletion { status, stderr })
    }

    pub fn terminate(mut self) -> Result<ProcessCompletion, ProcessError> {
        self.close_stdin();

        #[cfg(windows)]
        self.job.terminate()?;
        #[cfg(not(windows))]
        self.child.kill()?;

        let status = self.child.wait()?;
        self.join_reader_threads();
        let stderr = self.stderr_text();
        Ok(ProcessCompletion { status, stderr })
    }

    fn join_reader_threads(&mut self) {
        if let Some(stdout_thread) = self.stdout_thread.take() {
            let _ = stdout_thread.join();
        }
        if let Some(stderr_thread) = self.stderr_thread.take() {
            let _ = stderr_thread.join();
        }
    }

    fn stderr_text(&self) -> String {
        let Ok(bytes) = self.stderr_bytes.lock() else {
            return String::new();
        };
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[cfg(windows)]
struct JobObject {
    handle: HANDLE,
}

#[cfg(windows)]
impl JobObject {
    fn create_and_assign(child: &Child) -> Result<Self, ProcessError> {
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|error| ProcessError::Windows(error.to_string()))?;
        let job = Self { handle };

        let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&information).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(|error| ProcessError::Windows(error.to_string()))?;

        let child_handle = HANDLE(child.as_raw_handle());
        unsafe { AssignProcessToJobObject(job.handle, child_handle) }
            .map_err(|error| ProcessError::Windows(error.to_string()))?;

        Ok(job)
    }

    fn terminate(&self) -> Result<(), ProcessError> {
        unsafe { TerminateJobObject(self.handle, 1) }
            .map_err(|error| ProcessError::Windows(error.to_string()))
    }
}

#[cfg(windows)]
impl Drop for JobObject {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
