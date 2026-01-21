use std::process::Command;
use std::thread;
use std::time::Duration;
use sysinfo::System;

/// 检查 Antigravity 是否在运行
pub fn is_antigravity_running() -> bool {
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let current_pid = std::process::id();

    for (pid, process) in system.processes() {
        let pid_u32 = pid.as_u32();
        if pid_u32 == current_pid {
            continue;
        }

        let name = process.name().to_string_lossy().to_lowercase();
        let exe_path = process
            .exe()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 通用的辅助进程排除逻辑
        let args = process.cmd();
        let args_str = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_lowercase())
            .collect::<Vec<String>>()
            .join(" ");

        let is_helper = args_str.contains("--type=")
            || name.contains("helper")
            || name.contains("plugin")
            || name.contains("renderer")
            || name.contains("gpu")
            || name.contains("crashpad")
            || name.contains("utility")
            || name.contains("audio")
            || name.contains("sandbox")
            || exe_path.contains("crashpad");

        #[cfg(target_os = "macos")]
        {
            if exe_path.contains("antigravity.app") && !is_helper {
                return true;
            }
        }

        #[cfg(target_os = "windows")]
        {
            if name == "antigravity.exe" && !is_helper {
                return true;
            }
        }

        #[cfg(target_os = "linux")]
        {
            if (name.contains("antigravity") || exe_path.contains("/antigravity"))
                && !name.contains("tools")
                && !is_helper
            {
                return true;
            }
        }
    }

    false
}

/// 获取所有 Antigravity 进程的 PID（包括主进程和Helper进程）
fn get_antigravity_pids() -> Vec<u32> {
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut pids = Vec::new();
    let current_pid = std::process::id();

    for (pid, process) in system.processes() {
        let pid_u32 = pid.as_u32();

        // 排除自身 PID
        if pid_u32 == current_pid {
            continue;
        }

        let name = process.name().to_string_lossy().to_lowercase();
        let exe_path = process
            .exe()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 通用的辅助进程排除逻辑
        let args = process.cmd();
        let args_str = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_lowercase())
            .collect::<Vec<String>>()
            .join(" ");

        let is_helper = args_str.contains("--type=")
            || name.contains("helper")
            || name.contains("plugin")
            || name.contains("renderer")
            || name.contains("gpu")
            || name.contains("crashpad")
            || name.contains("utility")
            || name.contains("audio")
            || name.contains("sandbox")
            || exe_path.contains("crashpad");

        #[cfg(target_os = "macos")]
        {
            // 匹配 Antigravity 主程序包内的进程，但排除 Helper/Plugin/Renderer 等辅助进程
            if exe_path.contains("antigravity.app") && !is_helper {
                pids.push(pid_u32);
            }
        }

        #[cfg(target_os = "windows")]
        {
            if name == "antigravity.exe" && !is_helper {
                pids.push(pid_u32);
            }
        }

        #[cfg(target_os = "linux")]
        {
            if (name == "antigravity" || exe_path.contains("/antigravity"))
                && !name.contains("tools")
                && !is_helper
            {
                pids.push(pid_u32);
            }
        }
    }

    if !pids.is_empty() {
        crate::modules::logger::log_info(&format!(
            "找到 {} 个 Antigravity 进程: {:?}",
            pids.len(),
            pids
        ));
    }

    pids
}

/// 关闭 Antigravity 进程
pub fn close_antigravity(timeout_secs: u64) -> Result<(), String> {
    crate::modules::logger::log_info("正在关闭 Antigravity...");

    let pids = get_antigravity_pids();
    if pids.is_empty() {
        crate::modules::logger::log_info("Antigravity 未在运行，无需关闭");
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        crate::modules::logger::log_info(&format!(
            "正在 Windows 上关闭 {} 个 Antigravity 进程...",
            pids.len()
        ));
        for pid in &pids {
            let _ = Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .output();
        }
        thread::sleep(Duration::from_millis(200));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        // 阶段 1: 优雅退出 (SIGTERM)
        crate::modules::logger::log_info(&format!(
            "向 {} 个 Antigravity 进程发送 SIGTERM...",
            pids.len()
        ));
        for pid in &pids {
            let _ = Command::new("kill")
                .args(["-15", &pid.to_string()])
                .output();
        }

        // 等待优雅退出（最多 timeout_secs 的 70%）
        let graceful_timeout = (timeout_secs * 7) / 10;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(graceful_timeout) {
            if !is_antigravity_running() {
                crate::modules::logger::log_info("所有 Antigravity 进程已优雅关闭");
                return Ok(());
            }
            thread::sleep(Duration::from_millis(500));
        }

        // 阶段 2: 强制杀死 (SIGKILL)
        if is_antigravity_running() {
            let remaining_pids = get_antigravity_pids();
            if !remaining_pids.is_empty() {
                crate::modules::logger::log_warn(&format!(
                    "优雅关闭超时，强制杀死 {} 个残留进程 (SIGKILL)",
                    remaining_pids.len()
                ));
                for pid in &remaining_pids {
                    let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                }
                thread::sleep(Duration::from_secs(1));
            }
        }
    }

    // 最终检查
    if is_antigravity_running() {
        return Err("无法关闭 Antigravity 进程，请手动关闭后重试".to_string());
    }

    crate::modules::logger::log_info("Antigravity 已成功关闭");
    Ok(())
}

/// 启动 Antigravity
pub fn start_antigravity() -> Result<(), String> {
    crate::modules::logger::log_info("正在启动 Antigravity...");

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("open")
            .args(["-a", "Antigravity"])
            .output()
            .map_err(|e| format!("启动 Antigravity 失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Unable to find application") {
                return Err("未找到 Antigravity 应用，请确保已安装 Antigravity".to_string());
            }
            return Err(format!("启动 Antigravity 失败: {}", stderr));
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        
        // 尝试常见安装路径
        let possible_paths = [
            std::env::var("LOCALAPPDATA").ok().map(|p| format!("{}/Programs/Antigravity/Antigravity.exe", p)),
            std::env::var("PROGRAMFILES").ok().map(|p| format!("{}/Antigravity/Antigravity.exe", p)),
        ];

        for path_opt in possible_paths.iter().flatten() {
            let path = std::path::Path::new(path_opt);
            if path.exists() {
                Command::new(path_opt)
                    .creation_flags(0x08000000) // CREATE_NO_WINDOW
                    .spawn()
                    .map_err(|e| format!("启动 Antigravity 失败: {}", e))?;
                crate::modules::logger::log_info(&format!("Antigravity 已启动: {}", path_opt));
                return Ok(());
            }
        }
        return Err("未找到 Antigravity 可执行文件".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        // 尝试常见安装路径
        let possible_paths = [
            "/usr/bin/antigravity",
            "/opt/antigravity/antigravity",
        ];

        for path in possible_paths {
            if std::path::Path::new(path).exists() {
                Command::new(path)
                    .spawn()
                    .map_err(|e| format!("启动 Antigravity 失败: {}", e))?;
                crate::modules::logger::log_info(&format!("Antigravity 已启动: {}", path));
                return Ok(());
            }
        }

        // 尝试 PATH 中的 antigravity
        if Command::new("antigravity").spawn().is_ok() {
            crate::modules::logger::log_info("Antigravity 已启动 (从 PATH)");
            return Ok(());
        }

        return Err("未找到 Antigravity 可执行文件".to_string());
    }

    crate::modules::logger::log_info("Antigravity 启动命令已发送");
    Ok(())
}
