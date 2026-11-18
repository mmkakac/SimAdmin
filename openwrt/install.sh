#!/bin/sh
# SimAdmin OpenWrt 一键安装脚本（在设备上以 root 执行）
# 用法：sh install.sh
# 前置条件：已安装 dbus 与 modemmanager（opkg install dbus modemmanager libqmi libmbim）
set -e

ROOT_DIR=$(cd "$(dirname "$0")" && pwd)
PREFIX=/opt/simadmin
DATA_DIR=/etc/simadmin

case "$(uname -m)" in
	x86_64|amd64)          ARCH=x86_64 ;;
	aarch64|arm64)         ARCH=aarch64 ;;
	armv7l|armv7|armhf)    ARCH=armv7 ;;
	*)
		echo "不支持的架构: $(uname -m)（可用: x86_64 / aarch64 / armv7）" >&2
		exit 1
		;;
esac

BIN="$ROOT_DIR/bin/$ARCH/simadmin"
[ -f "$BIN" ] || { echo "未找到对应架构的二进制: $BIN" >&2; exit 1; }

echo "==> 安装到 $PREFIX (架构: $ARCH)"
mkdir -p "$PREFIX" "$DATA_DIR"
/etc/init.d/simadmin stop 2>/dev/null || true

if [ -f "$PREFIX/simadmin" ]; then
	cp -f "$PREFIX/simadmin" "$PREFIX/simadmin.bak.$(date +%Y%m%d%H%M%S)"
fi

cp -f "$BIN" "$PREFIX/simadmin"
chmod 755 "$PREFIX/simadmin"

rm -rf "$PREFIX/www"
mkdir -p "$PREFIX/www"
cp -rf "$ROOT_DIR/www/." "$PREFIX/www/"

install -m 755 "$ROOT_DIR/files/etc/init.d/simadmin" /etc/init.d/simadmin
[ -f /etc/config/simadmin ] || install -m 644 "$ROOT_DIR/files/etc/config/simadmin" /etc/config/simadmin

/etc/init.d/simadmin enable
echo
echo "==> 依赖检查"
for c in dbus modemmanager; do
	[ -d "/etc/init.d/$c" ] || echo "   ⚠ 未检测到 $c 服务，请执行: opkg update && opkg install dbus modemmanager"
done
command -v mmcli >/dev/null 2>&1 || echo "   ⚠ 未检测到 mmcli（通常由 modemmanager 包提供）"

echo
echo "==> 启动服务"
/etc/init.d/dbus start 2>/dev/null || true
/etc/init.d/modemmanager start 2>/dev/null || true
/etc/init.d/simadmin start

PORT=$(uci -q get simadmin.main.port || echo 3000)
LAN_IP=$(uci -q get network.lan.ipaddr || echo 192.168.1.1)
echo
echo "完成。访问 http://$LAN_IP:$PORT （首次进入需设置管理员密码）"
echo "日志: logread -f -e simadmin"
