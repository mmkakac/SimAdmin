# OpenWrt 适配说明

本目录的代码改动让 SimAdmin 能在 **OpenWrt（procd/ubus、无 systemd、无 NetworkManager）** 上运行。
所有改动都以「运行时后端探测」方式实现，**Debian 上的行为保持不变**。

## 1. 关键改动

| 改动 | 文件 | 说明 |
|---|---|---|
| 服务管理抽象 | `backend/src/service_manager.rs`（新增） | `systemctl` ⇄ `/etc/init.d/<svc>` 自动翻译；`SIMADMIN_SERVICE_BACKEND=procd\|systemd\|auto` 可强制指定 |
| 基带 / ModemManager 重启 | `backend/src/modem_manager.rs` | 4 处 `systemctl` 改为 `service_manager::{stop,start,restart,is_active}` |
| 重启准备阶段停 MM | `backend/src/handlers.rs` | 同上 |
| 自愈资产安装 | `backend/src/modem_recovery.rs` | OpenWrt 下跳过 systemd 单元/定时器安装 |
| MM debug drop-in | `backend/src/main.rs` | OpenWrt 下跳过（改由 MM 包自身策略决定） |
| 蜂窝拨号 | `backend/src/modem_manager.rs` | OpenWrt 下改为 **netifd** `ifup/ifdown <wwan>`，不再操作 NM profile |
| NM profile 初始化 | `backend/src/modem_manager.rs` | OpenWrt 下直接跳过 |
| WLAN 客户端 | `backend/src/device_network.rs` | OpenWrt 下明确返回「请用 LuCI / uci 管理」，不再调用 nmcli |
| OTA 安装路径 | `backend/src/ota.rs` | 支持 `SIMADMIN_INSTALL_DIR`（默认 `/opt/simadmin`） |
| 防火墙诊断 | `backend/src/iptables.rs` | `iptables` 不存在时退回只读 `nft list ruleset` 统计 |

## 2. 环境变量

| 变量 | 默认 | 用途 |
|---|---|---|
| `SIMADMIN_DATA_DIR` | exe 同级 / `/data` | SQLite 与 `config.json` 存放目录（OpenWrt 建议 `/etc/simadmin`） |
| `SIMADMIN_INSTALL_DIR` | `/opt/simadmin` | OTA 替换二进制 / `www` / `meta.json` 的根目录 |
| `SIMADMIN_SERVICE_BACKEND` | `auto` | `auto` / `systemd` / `procd` |
| `SIMADMIN_WWAN_IFACE` | `wwan` | netifd 中承载蜂窝数据的接口名 |

## 3. 前置条件（设备侧）

```sh
opkg update
opkg install dbus modemmanager        # 必需：系统总线 + ModemManager(含 mmcli)
opkg install libqmi-utils libmbim-utils iptables   # 可选
```

`/etc/config/network` 中需要有 netifd 的 modemmanager 协议接口，SimAdmin 的「数据连接」开关
就是对它做 `ifup/ifdown`：

```uci
config interface 'wwan'
    option proto 'modemmanager'
    option apn 'ctnet'
    # option pin '1234'
    option autoconnect '0'
```

## 4. 编译

GitHub Actions 已提供 `.github/workflows/build-openwrt.yml`（手动触发 / 推送自动触发），
产出三个架构的静态二进制包（tar.gz），不依赖上游 Release 权限。

本地编译：

```sh
# x86_64（musl 静态）
sudo apt install musl-tools
CC_x86_64_unknown_linux_musl=musl-gcc RUSTFLAGS="-C target-feature=+crt-static" \
SQLITE3_STATIC=1 LIBSQLITE3_SYS_USE_PKG_CONFIG=0 \
cargo build --locked --release --target x86_64-unknown-linux-musl -p simadmin

# aarch64 / armv7（交叉 gcc + crt-static，全静态）
sudo apt install gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf
CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
RUSTFLAGS="-C target-feature=+crt-static" SQLITE3_STATIC=1 \
cargo build --locked --release --target aarch64-unknown-linux-gnu -p simadmin
```

> 要求 Rust **≥ 1.87**（`crates/simadmin-device-runtime/src/apdu.rs` 使用了 `is_multiple_of`，
> 1.85 会报 E0658）。

## 5. 安装

```sh
tar xzf simadmin-openwrt-<arch>-v<version>.tar.gz
cd simadmin-openwrt-<arch>-v<version>
sh install.sh
```

`install.sh` 会按 `uname -m` 选择 `bin/<arch>/simadmin`，安装到 `/opt/simadmin`，
数据目录 `/etc/simadmin`，注册 procd 服务 `/etc/init.d/simadmin`。

## 6. 功能可用性

| 能力 | 状态 | 说明 |
|---|---|---|
| Web UI / 登录 / 配置 / 备份 / 自动化 / 通知 / DDNS / OTA 上传 | ✅ | 应用层功能，实测可用 |
| SIM 信息 / 信号 / 注册 / 运营商 / APN / 短信 | ✅ | 依赖 `modemmanager` 与 `mmcli`（OpenWrt 包内自带） |
| 数据连接开关 | ✅ | 走 netifd `ifup/ifdown`，需先配好 `proto modemmanager` 接口 |
| 基带重启 / ModemManager 重启 | ✅ | 翻译为 `/etc/init.d/modemmanager restart` |
| Modem 开机自愈（systemd timer） | ⚠️ | OpenWrt 下跳过，改由 procd respawn + MM hotplug 承担 |
| WLAN 客户端扫描/连接 | ❌ | 由 LuCI/uci 管理；接口返回明确提示 |
| 频段/小区锁定 | ⚠️ | 依赖 MM 固件能力 |
| eSIM（lpac） | ⚠️ | 需为 OpenWrt 目标自行编译 lpac 并放到 `/opt/simadmin/lpac/lpac` |
| iptables 诊断 | ⚠️ | nft 环境下改为 `nft list ruleset` 只读统计 |

## 7. 注意

- 默认监听 `0.0.0.0:3000`（由 `/etc/config/simadmin` 的 `port` / `host` 控制）。
- 首次访问需设置管理员密码；也可在设备上执行 `simadmin auth reset-password`。
- 本项目为 GPLv3，分发预编译包请附带源码与许可证。
