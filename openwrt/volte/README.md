# SimAdmin VoLTE 版 · OpenWrt 适配说明

## 这是什么

上游 `3899/SimAdmin` 的 **VoLTE 版**（`simadmin-volte-aarch64-v{ver}.tar.gz`）是把
标准版 + 原生 IMS 客户端（SIP / IPsec / AKAv1-MD5 / TS 24.011 短信）编在同一个二进制里。
它由上游以 **aarch64-unknown-linux-musl 全静态** 方式编译，所以**内核 ABI 上可以直接跑在 OpenWrt**，
不需要 glibc，也不需要重新编译。

唯一的障碍是「宿主环境假设」：它默认宿主是 Debian + systemd + NetworkManager。
本目录用一层极薄的 **shim（命令兼容层）+ procd 服务脚本** 把这些假设抹平：

| 上游假设 | OpenWrt 现实 | 本方案 |
|---|---|---|
| `systemctl restart ModemManager.service` | 没有 systemd | `bin/systemctl` 翻译成 `/etc/init.d/modemmanager restart` |
| `nmcli connection add/show/up/down` | 没有 NetworkManager | `bin/nmcli` 把 up/down 映射到 netifd `ifup/ifdown`，探测类命令返回空 |
| `udevadm control --reload-rules` | 没有 udev | `bin/udevadm` 空实现（成功返回） |
| `iptables -S` 只读诊断 | 可能只有 nftables | `bin/iptables` 有就转交，没有就返回空规则集 |
| systemd 单元 / drop-in / 定时器 | procd | 文件写入无害化；服务由 `/etc/init.d/simadmin` 托管 |

**VoLTE 本身的数据面**（IMS 承载、SIP 信令绑定蜂窝网卡、IPsec 策略、QMI WDS profile）
是上游自己用 QMI + netlink 做的，不依赖 NetworkManager，所以这部分在 OpenWrt 上是**原生可用的**。

## 已验证到什么程度

在 x86_64 的同版本 VoLTE 二进制上实测（同源码同逻辑）：

- 启动正常，`/api/health` ok
- `/api/volte/control` 返回完整 schema，`platform_supported: true`
- `POST /api/volte/feature {"enabled":true}` → 成功，返回
  `feature_enabled: true, sms_enabled: true`
- 运行态进入 `phase=waiting_for_network`，日志出现
  `Native VoLTE IMS runtime supervisor enabled` —— 即引擎已就绪，等基带驻网后做 IMS 注册
- 缺 `nmcli` 时不会崩，只是日志提示 `No NetworkManager cellular profile to reserve for VoLTE`

未验证的部分（必须有真机 + 可注册 IMS 的卡 + 支持 QMI IMS 的模组）：

- IMS PDN 建立、P-CSCF 发现、SIP REGISTER、IPsec 协商、VoLTE 收短信/打电话

## 依赖（设备侧）

```sh
opkg update
opkg install dbus modemmanager libqmi-utils
# 建议：libmbim-utils（MBIM 模组）、iptables（诊断更好看）
```

内核还需要 XFRM/IPsec 支持（SIP over IPsec 需要 `CONFIG_INET_ESP` + `xfrm_user`）。
多数 OpenWrt 官方 target 默认已开；极简固件若裁掉，需要重新编译内核。

## 安装

```sh
tar xzf simadmin-volte-openwrt-aarch64-v<ver>.tar.gz
cd simadmin-volte-openwrt-aarch64-v<ver>
sh install.sh
```

`install.sh` 会：

1. 把二进制 / `www` / `meta.json` 放到 `/opt/simadmin`
2. 把 4 个 shim 放到 `/opt/simadmin/bin`
3. 写 `/etc/init.d/simadmin`（procd，respawn）与默认 `/etc/config/simadmin`
4. 启用并启动服务

数据（`config.json` / `data.db`）默认落在 `/opt/simadmin`；若 `/opt` 在只读分区，
把 `/etc/config/simadmin` 的 `data_dir` 改到 `/etc/simadmin` 后重启服务。

## 蜂窝接口

```uci
config interface 'wwan'
    option proto 'modemmanager'
    option apn 'ctnet'
    option autoconnect '0'
```

VoLTE 引擎启用时会临时「接管」这个数据 profile 再交还；`autoconnect 0` 是为了避免
开机时普通数据抢先占用，等 SimAdmin 自己按需拉起。

## 使用

1. Web 打开 `http://<设备IP>:3000`，首次设置管理员密码
2. 仪表盘 / SIM 卡管理 → VoLTE 卡片：先开「功能开关」，再开「连接」
3. 诊断页可看：阶段（waiting_for_network → 注册中 → registered）、P-CSCF、本地 IP、
   SIP 收发计数、最近错误
4. 短信页会显示 `volte_ims` 传输徽标，VoLTE 承载优先、蜂窝短信热备

## 已知限制

- `secondary-qmi-init`（DATA6 / RPMSG 双 QMI 通道）依赖 systemd unit，OpenWrt 上不安装。
  只有部分高通模组（需要第二 QMI 通道跑 IMS）才用到；这类设备需自行把 unit 换成 procd 实例。
- eSIM 需要 `/opt/simadmin/lpac/lpac`：官方 lpac 是 glibc+libqmi 版，OpenWrt 上需自己交叉编译
  （或放一个 musl 静态 lpac）。
- 上游会在 `/etc/systemd/system/` 写单元文件（自愈/调试 drop-in），OpenWrt 上这些目录会是死文件，
  不影响运行，也不需要清理。
- 本包是**上游二进制原样分发**（GPLv3：源码见上游仓库与 VoLTE 版对应的源码发布渠道），
  本目录只额外提供 shim 与 procd 封装。

## 硬前提：内核必须支持 XFRM（IPsec）

中国移动 / 联通 / 电信的 IMS 在 SIP REGISTER 阶段**强制要求 IPsec 安全协商**
（P-CSCF 返回 `Security-Server: ipsec-3gpp;alg=hmac-md5-96;prot=esp`）。
VoLTE 引擎通过 `ip xfrm` 安装 SA 与策略，因此内核必须编入 XFRM 与 ESP。

若内核裁剪掉了这块，日志会出现：

```
WARN Native VoLTE IPsec registration failed, falling back to plain UDP SIP
reason=volte_command_failed:ip:Cannot open netlink socket: Protocol not supported
WARN Native VoLTE plain UDP runtime failed reason=volte_register_initial_unexpected_status:420
```

自查：

```sh
ip xfrm state                                     # 报 Protocol not supported 即内核缺 XFRM
zcat /proc/config.gz | grep -E "XFRM_USER|INET_ESP"
```

处理：内核内置选项见 `openwrt/kernel/xfrm.config`，追加到 target 的 `config-<版本>` 后重编固件。
这是固件层限制，软件侧无法绕过（用户态 IPsec 方案无法与引擎自行安装的 SA 对接）。

顺带说明：msm89xx/msm8916（UF896 类）社区 target 的 `config-5.10` 默认没有任何 XFRM 选项，
属于常见裁剪点；若固件由厂商提供且无法重编，VoLTE 只能等厂商放带 IMS 的固件。

## 依赖项：ModemManager 必须处于 debug 日志级别

VoLTE 引擎用 `mmcli -m any --command='AT+...'` 读写模组（AT+CIMI / AT+CIREG / 短信收发）。
ModemManager 的安全策略规定：**非 debug 模式下所有 `--command` 一律拒绝**，返回
`Unauthorized: Operation only allowed in debug mode`，日志表现为：

```
WARN Native VoLTE ModemManager AT+CIMI failed ... volte_modemmanager_debug_required
WARN Native VoLTE SMS runtime failed ... volte_modemmanager_debug_required
```

包内 `bin/mm-debug-ensure` 会在服务启动前把 `/etc/init.d/modemmanager` 的
`LOG_LEVEL="INFO"` 改成 `DEBUG`（幂等），并重启一次 MM。刷机后该修改会丢失，
重装本包即自动恢复。

## 防火墙：必须放行入站 ESP/AH

IMS 的 SIP 走 ESP（IP 协议 50）/AH，不是 TCP/UDP，fw4 默认 input 策略会丢包，
表现为 IPsec SA 已安装、REGISTER 已发出，但等不到回包：

```
INFO Native VoLTE 3GPP TS 33.203 4-SA IPsec xfrm installed ...
INFO Native VoLTE IPsec authenticated REGISTER sent, awaiting response ...
WARN Native VoLTE IPsec registration failed, falling back to plain UDP SIP
     reason=volte_udp_recv_timeout
```

`install.sh` 会自动写入两条 UCI 规则（幂等，已存在则跳过）：

```ini
config rule
	option name 'Allow-IPsec-ESP'
	option src 'wan'
	option proto 'esp'
	option target 'ACCEPT'

config rule
	option name 'Allow-IPsec-AH'
	option src 'wan'
	option proto 'ah'
	option target 'ACCEPT'
```

若 IMS 接口不在 wan zone（自建 zone / 未归属 zone），需把 `src` 改成对应 zone，
或改用 `/etc/nftables.d/20-volte-ipsec.nft`：

```
chain volte_ipsec {
	type filter hook input priority -10; policy accept;
	ip protocol esp accept
	ip6 nexthdr esp accept
}
```
