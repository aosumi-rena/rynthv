use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

use crate::synthesis::voice::{Result, VoiceError};

pub(super) fn open_read(path: &Path) -> io::Result<File> {
    options().read(true).open(path)
}

pub(super) fn open_lock(path: &Path) -> io::Result<File> {
    options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

pub(super) fn create_temporary(path: &Path) -> io::Result<File> {
    options().write(true).create_new(true).open(path)
}

fn options() -> OpenOptions {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::os::unix::fs::OpenOptionsExt;

        // O_NOFOLLOW rejects a symlink in the final path component; newly
        // created context and lock files are private to their user.
        const O_NOFOLLOW: i32 = if cfg!(target_os = "macos") { 0x100 } else { 0x20000 };
        let mut options = OpenOptions::new();
        options.custom_flags(O_NOFOLLOW).mode(0o600);
        options
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        OpenOptions::new()
    }
}

#[cfg(windows)]
pub(super) fn verify(file: &File, expected: &Path) -> Result<()> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFinalPathNameByHandleW(
            handle: *mut c_void,
            path: *mut u16,
            length: u32,
            flags: u32,
        ) -> u32;
    }
    let mut buffer = vec![0u16; 32768];

    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            0,
        )
    };
    if length == 0 || length as usize >= buffer.len() {
        return Err(VoiceError::new("无法核对模型上下文的实际保存路径。"));
    }
    let actual = normalize(&String::from_utf16_lossy(&buffer[..length as usize]));
    let expected = normalize(&expected.to_string_lossy());
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(VoiceError::new(
            "模型上下文路径被宿主程序重定向。请从桌面启动 RynthV 后执行导入。",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn normalize(path: &str) -> String {
    let path = path.replace('/', "\\");
    if let Some(path) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{path}")
    } else {
        path.strip_prefix("\\\\?\\").unwrap_or(&path).to_owned()
    }
}

#[cfg(target_os = "linux")]
pub(super) fn verify(file: &File, expected: &Path) -> Result<()> {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(VoiceError::new(
            "模型上下文必须是普通文件，不能使用硬链接。",
        ));
    }
    let expected = std::path::absolute(expected)?;
    let parent = expected
        .parent()
        .ok_or_else(|| VoiceError::new("模型上下文路径缺少父目录。"))?;
    let filename = expected
        .file_name()
        .ok_or_else(|| VoiceError::new("模型上下文路径缺少文件名。"))?;

    let expected = parent.canonicalize()?.join(filename);
    let actual = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    if actual != expected {
        return Err(VoiceError::new("模型上下文的实际路径与预期路径不一致。"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn verify(file: &File, expected: &Path) -> Result<()> {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

    unsafe extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }

    // F_GETPATH returns the canonical path of an open descriptor; it is the
    // macOS counterpart to /proc/self/fd on Linux.
    const F_GETPATH: i32 = 50;

    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(VoiceError::new(
            "模型上下文必须是普通文件，不能使用硬链接。",
        ));
    }
    let expected = std::path::absolute(expected)?;
    let parent = expected
        .parent()
        .ok_or_else(|| VoiceError::new("模型上下文路径缺少父目录。"))?;
    let filename = expected
        .file_name()
        .ok_or_else(|| VoiceError::new("模型上下文路径缺少文件名。"))?;
    // Resolve only the parent: user directories may be symlinked, while
    // open_read/open_lock/create_temporary reject links to the file itself.
    let expected = parent.canonicalize()?.join(filename);
    let mut buffer = [0u8; 4096];
    // SAFETY: the descriptor is live and the buffer is writable for its full
    // length; F_GETPATH writes a NUL-terminated path and never overruns it.
    let length = unsafe {
        fcntl(
            file.as_raw_fd(),
            F_GETPATH,
            buffer.as_mut_ptr() as *mut std::ffi::c_char,
        )
    };
    if length < 0 {
        return Err(VoiceError::new("无法读取模型上下文的实际路径。"));
    }
    let end = buffer.iter().position(|&byte| byte == 0).unwrap_or(buffer.len());
    let actual = std::str::from_utf8(&buffer[..end])
        .map_err(|_| VoiceError::new("模型上下文的实际路径不是有效 UTF-8。"))?;
    if Path::new(actual) != expected {
        return Err(VoiceError::new("模型上下文的实际路径与预期路径不一致。"));
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(super) fn verify(_file: &File, _expected: &Path) -> Result<()> {
    Err(VoiceError::new(
        "模型上下文导入当前仅支持 Windows、Linux 和 macOS。",
    ))
}
