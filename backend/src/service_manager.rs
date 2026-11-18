//! 服务管理后端抽象：systemd ⇄ OpenWrt procd
//!
//! 上游代码大量直接调用 `systemctl`，在 OpenWrt（procd/ubus，无 systemd）上全部失效，
//! 表现为「基带重启、Modem 自愈、服务重载」静默不生效。本模块把这类操作收敛到一处：
//!
//! - systemd 环境：原行为不变（`systemctl restart ModemManager.service`）
//! - OpenWrt 环境：翻译为 `/etc/init.d/modemmanager restart` 或 `/etc/init.d/network reload`
//!
//! 判定方式：`SIMADMIN_SERVICE_BACKEND=procd|systemd|auto`（默认 auto）。
//! auto 规则：`/etc/rc.common` 存在且 `/run/systemd/system` 不存在 → procd。

use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    Systemd,
    Procd,
}

static BACKEND: OnceLock<Backend> = OnceLock::new();

pub fn backend() -> Backend {
    *BACKEND.get_or_init(|| {
        let forced = std::env::var("SIMADMIN_SERVICE_BACKEND").unwrap_or_default();
        let detected = match forced.to_ascii_lowercase().as_str() {
            "procd" | "openwrt" => Backend::Procd,
            "systemd" => Backend::Systemd,
            _ => detect(),
        };
        info!(backend = ?detected, "Service backend resolved");
        detected
    })
}

pub fn is_openwrt() -> bool {
    backend() == Backend::Procd
}

fn detect() -> Backend {
    if Path::new("/etc/rc.common").exists() && !Path::new("/run/systemd/system").exists() {
        Backend::Procd
    } else {
        Backend::Systemd
    }
}

/// 把 systemd 单元名映射为 OpenWrt 的 init 脚本名
fn procd_service_name(unit: &str) -> String {
    let base = unit
        .trim_end_matches(".service")
        .trim_end_matches(".timer")
        .trim_end_matches(".socket");
    match base {
        "ModemManager" => "modemmanager".to_string(),
        "NetworkManager" => "network".to_string(),
        "simadmin" => "simadmin".to_string(),
        other => other.to_ascii_lowercase(),
    }
}

fn init_script_exists(name: &str) -> bool {
    Path::new(&format!("/etc/init.d/{name}")).exists()
}

async fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|err| format!("failed to run {cmd}: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "{cmd} {} failed (exit {:?}): {stderr}",
            args.join(" "),
            output.status.code()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn procd_action(unit: &str, action: &str, fallback_to_reload: bool) -> Result<String, String> {
    let svc = procd_service_name(unit);
    if !init_script_exists(&svc) {
        return Err(format!("no /etc/init.d/{svc} on this system"));
    }
    match run(&format!("/etc/init.d/{svc}"), &[action]).await {
        Ok(out) => Ok(out),
        Err(err) if fallback_to_reload && action == "restart" => {
            warn!(service = %svc, error = %err, "restart failed, falling back to reload");
            run(&format!("/etc/init.d/{svc}"), &["reload"]).await
        }
        Err(err) => Err(err),
    }
}

/// 同步版本（供启动早期的非 async 场景使用）
pub fn restart_blocking(unit: &str) -> Result<String, String> {
    match backend() {
        Backend::Procd => {
            let svc = procd_service_name(unit);
            if !init_script_exists(&svc) {
                return Err(format!("no /etc/init.d/{svc} on this system"));
            }
            let out = std::process::Command::new(format!("/etc/init.d/{svc}"))
                .arg("restart")
                .output()
                .map_err(|err| format!("failed to restart {svc}: {err}"))?;
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        Backend::Systemd => {
            let out = std::process::Command::new("systemctl")
                .args(["restart", unit])
                .output()
                .map_err(|err| format!("failed to run systemctl: {err}"))?;
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
    }
}

/// 重启服务（systemd: `systemctl restart <unit>`；procd: `/etc/init.d/<svc> restart`）
pub async fn restart(unit: &str) -> Result<String, String> {
    match backend() {
        Backend::Procd => procd_action(unit, "restart", true).await,
        Backend::Systemd => run("systemctl", &["restart", unit]).await,
    }
}

pub async fn stop(unit: &str) -> Result<String, String> {
    match backend() {
        Backend::Procd => procd_action(unit, "stop", false).await,
        Backend::Systemd => run("systemctl", &["stop", unit]).await,
    }
}

pub async fn start(unit: &str) -> Result<String, String> {
    match backend() {
        Backend::Procd => procd_action(unit, "start", false).await,
        Backend::Systemd => run("systemctl", &["start", unit]).await,
    }
}

/// 服务是否处于运行状态
pub async fn is_active(unit: &str) -> bool {
    match backend() {
        Backend::Procd => {
            let svc = procd_service_name(unit);
            if !init_script_exists(&svc) {
                return false;
            }
            run(&format!("/etc/init.d/{svc}"), &["status"]).await.is_ok()
        }
        Backend::Systemd => run("systemctl", &["is-active", "--quiet", unit])
            .await
            .is_ok(),
    }
}

/// systemd 需要 `daemon-reload`；procd 没有等价概念（脚本即配置）
pub fn daemon_reload() {
    match backend() {
        Backend::Procd => {}
        Backend::Systemd => {
            let _ = std::process::Command::new("systemctl")
                .arg("daemon-reload")
                .output();
        }
    }
}

/// 启用开机自启（OpenWrt 上由安装脚本 / `/etc/init.d/<svc> enable` 负责）
pub async fn enable(unit: &str) -> bool {
    match backend() {
        Backend::Procd => {
            let svc = procd_service_name(unit);
            if !init_script_exists(&svc) {
                return false;
            }
            run(&format!("/etc/init.d/{svc}"), &["enable"]).await.is_ok()
        }
        Backend::Systemd => run("systemctl", &["enable", unit]).await.is_ok(),
    }
}

// ---------------------------------------------------------------------------
// 蜂窝数据拨号（OpenWrt 路径）
// ---------------------------------------------------------------------------

/// OpenWrt 上承载蜂窝数据的 netifd 接口名（默认 `wwan`，可用环境变量覆盖）
fn wwan_interface() -> String {
    std::env::var("SIMADMIN_WWAN_IFACE").unwrap_or_else(|_| "wwan".to_string())
}

/// 通过 netifd 启停蜂窝接口。
///
/// 前提：`/etc/config/network` 中已有 `proto modemmanager`（或 qmi/mbim）的接口，
/// 例如：
/// ```text
/// config interface 'wwan'
///     option proto 'modemmanager'
///     option apn 'ctnet'
/// ```
/// 这样 ModemManager 会自己完成 Simple.Connect，本程序只负责 ifup/ifdown，
/// 不再自己改 NM profile，避免两套管理互相打架。
pub async fn set_wwan_link(active: bool) -> Result<String, String> {
    let iface = wwan_interface();
    if active {
        // 先确保无线/蜂窝接口没有被 netifd 侧禁用
        let _ = run("ifup", &[&iface]).await;
        // ifup 是异步的，给 netifd 一点时间
        tokio::time::sleep(Duration::from_millis(800)).await;
        match run("ifstatus", &[&iface]).await {
            Ok(_) => Ok(format!("cellular interface '{iface}' brought up via netifd")),
            Err(err) => Err(format!("ifup '{iface}' failed: {err}")),
        }
    } else {
        let _ = run("ifdown", &[&iface]).await;
        Ok(format!("cellular interface '{iface}' brought down via netifd"))
    }
}

/// 查询 netifd 侧接口是否 up（供诊断）
pub async fn wwan_link_status() -> Option<bool> {
    let iface = wwan_interface();
    let out = run("ifstatus", &[&iface]).await.ok()?;
    Some(out.contains("\"up\": true"))
}
