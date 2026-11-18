#!/bin/sh
# SimAdmin VoLTE 版 —— OpenWrt 安装脚本
# 用法：sh install.sh [--no-start]

set -e

START_SERVICE=1
[ "$1" = "--no-start" ] && START_SERVICE=0

INSTALL_DIR=/opt/simadmin
BUNDLE_DIR=$(cd "$(dirname "$0")" && pwd)

[ "$(id -u)" = "0" ] || { echo "请用 root 运行"; exit 1; }

ARCH=$(uname -m)
echo "==> 设备架构: $ARCH"
case "$ARCH" in
	aarch64|arm64) : ;;
	*) echo "!! 本包仅含 aarch64 二进制，当前架构 $ARCH 可能无法运行" ;;
esac

echo "==> 安装到 $INSTALL_DIR"
mkdir -p "$INSTALL_DIR/bin" "$INSTALL_DIR/lpac" /etc/init.d /etc/config

cp -f "$BUNDLE_DIR/simadmin" "$INSTALL_DIR/simadmin"
chmod 755 "$INSTALL_DIR/simadmin"

[ -d "$BUNDLE_DIR/www" ] && rm -rf "$INSTALL_DIR/www" && cp -r "$BUNDLE_DIR/www" "$INSTALL_DIR/www"
[ -f "$BUNDLE_DIR/meta.json" ] && cp -f "$BUNDLE_DIR/meta.json" "$INSTALL_DIR/meta.json"

cp -f "$BUNDLE_DIR/files/opt/simadmin/bin/"* "$INSTALL_DIR/bin/"
chmod 755 "$INSTALL_DIR/bin/"*

cp -f "$BUNDLE_DIR/files/etc/init.d/simadmin" /etc/init.d/simadmin
chmod 755 /etc/init.d/simadmin

if [ ! -f /etc/config/simadmin ]; then
	cp -f "$BUNDLE_DIR/files/etc/config/simadmin" /etc/config/simadmin
	echo "==> 已写入默认 /etc/config/simadmin"
fi

# 内存/闪存提示
echo "==> 磁盘剩余:" $(df -k "$INSTALL_DIR" 2>/dev/null | awk 'NR==2{print $4"KB"}')

if [ "$START_SERVICE" = "1" ]; then
	/etc/init.d/simadmin enable >/dev/null 2>&1 || true
	/etc/init.d/simadmin restart >/dev/null 2>&1 || /etc/init.d/simadmin start
fi

cat <<'EOF'

==> 安装完成。接下来：

1) 安装运行依赖（OpenWrt）：
   opkg update
   opkg install dbus modemmanager libqmi-utils
   # 可选：libmbim-utils iptables

2) 蜂窝接口（/etc/config/network）：
   config interface 'wwan'
       option proto 'modemmanager'
       option apn 'ctnet'
       option autoconnect '0'

3) 打开 Web：http://<设备IP>:3000
   查看日志：logread -f | grep simadmin

4) VoLTE 开关在「仪表盘 / SIM 卡管理 → VoLTE」，首次打开会自动选 IMS APN 并注册。

注意：/etc/config/simadmin 的 data_dir 默认 /opt/simadmin，
若 /opt 在只读分区，请改到 /etc/simadmin 或 /root/simadmin。
EOF
