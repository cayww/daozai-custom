//! 单实例保护，对应 Python `halo/diagnostic_guard.py` 的 `InstanceGuard`。
//!
//! 语义与 Python 完全一致：`CreateMutexW` 创建具名内核对象后**一直持有句柄**，
//! 通过 `GetLastError() == ERROR_ALREADY_EXISTS (183)` 判断是否已有实例；
//! 不获取/释放 mutex 所有权，也不等待。句柄在 `Drop` 时关闭。
//!
//! 注意：本模块使用 `Global\HaloBattery_single_instance`（Python 侧原名），
//! 与主程序 `src/tray.rs` 的 `Local\Daozai.SingleInstance` 是两个独立对象，
//! 不会互相干扰。若目标环境无 SeCreateGlobalPrivilege，`CreateMutexW` 会失败，
//! `acquire()` 返回 `GuardError::CreateFailed`，由调用方决定如何处理。

use std::fmt;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE},
        System::Threading::CreateMutexW,
    },
};

/// 与 Python `INSTANCE_NAME` 一致。
pub const INSTANCE_NAME: PCWSTR = w!("Global\\HaloBattery_single_instance");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardError {
    /// 同名互斥体已存在（GetLastError == ERROR_ALREADY_EXISTS）。
    AlreadyRunning,
    /// CreateMutexW 调用本身失败（如缺少 Global 命名空间权限）。
    CreateFailed,
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => f.write_str("已有 daozai-halo 实例在运行"),
            Self::CreateFailed => f.write_str("无法创建 daozai-halo 实例互斥体"),
        }
    }
}

/// 持有具名互斥体句柄；只要本值存活，第二实例就会拿到 `AlreadyRunning`。
pub struct InstanceGuard {
    handle: HANDLE,
}

impl InstanceGuard {
    /// 尝试成为唯一实例。成功时返回 guard（调用方必须把它绑定到进程生命周期
    /// 的变量，如 `let _guard = ...`）；已有实例时返回 `GuardError::AlreadyRunning`。
    pub fn acquire() -> Result<Self, GuardError> {
        // Python: set_last_error(0) -> CreateMutexW -> get_last_error()
        // windows-rs 的 CreateMutexW 成功时返回 Ok(HANDLE)；紧接着读 GetLastError
        // 即可拿到 ERROR_ALREADY_EXISTS（与主程序 tray.rs::single_instance 同款）。
        unsafe {
            match CreateMutexW(None, false, INSTANCE_NAME) {
                Ok(handle) => {
                    if GetLastError() == ERROR_ALREADY_EXISTS {
                        // 即使对象已存在，返回的 handle 仍属于本进程，必须关闭。
                        let _ = CloseHandle(handle);
                        Err(GuardError::AlreadyRunning)
                    } else {
                        Ok(Self { handle })
                    }
                }
                Err(_) => Err(GuardError::CreateFailed),
            }
        }
    }

}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_183_is_already_running() {
        assert_eq!(ERROR_ALREADY_EXISTS.0, 183);
        assert!(ERROR_ALREADY_EXISTS == windows::Win32::Foundation::WIN32_ERROR(183));
        assert!(ERROR_ALREADY_EXISTS != windows::Win32::Foundation::WIN32_ERROR(0));
    }

    #[test]
    fn guard_error_messages() {
        assert_eq!(GuardError::AlreadyRunning.to_string(), "已有 daozai-halo 实例在运行");
        assert_eq!(GuardError::CreateFailed.to_string(), "无法创建 daozai-halo 实例互斥体");
    }
}
