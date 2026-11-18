# OpenWrt 打包 Makefile（预编译二进制方案）
#
# 用法：
#   1. 将本目录整体放入 OpenWrt SDK / 源码树：package/simadmin/
#   2. 确认 bin/<arch>/simadmin 与 www/ 已存在（本包已附带）
#   3. make menuconfig 选中 Network -> SimAdmin，然后 make package/simadmin/compile
#
# 说明：本项目 C 依赖（ring / sqlite3）较重，先在普通 Linux 上做静态交叉编译，
#      再由 OpenWrt 打包成 ipk，比在 SDK 内直接编 Rust 更省时可靠。

include $(TOPDIR)/rules.mk

PKG_NAME:=simadmin
PKG_VERSION:=1.2.4
PKG_RELEASE:=1
PKG_LICENSE:=GPL-3.0-or-later
PKG_LICENSE_FILES:=LICENSE

include $(INCLUDE_DIR)/package.mk

# ---- 依据目标架构挑选预编译二进制 ----
ifneq ($(filter aarch64%,$(ARCH)),)
  SIMADMIN_BIN:=$(CURDIR)/bin/aarch64/simadmin
else ifneq ($(filter arm%,$(ARCH)),)
  SIMADMIN_BIN:=$(CURDIR)/bin/armv7/simadmin
else ifneq ($(filter x86_64 i386%,$(ARCH)),)
  SIMADMIN_BIN:=$(CURDIR)/bin/x86_64/simadmin
else
  SIMADMIN_BIN:=
endif

define Package/simadmin
  SECTION:=net
  CATEGORY:=Network
  TITLE:=SimAdmin - SIM/eSIM 蜂窝设备 Web 管理
  URL:=https://github.com/3899/SimAdmin
  DEPENDS:=+dbus +modemmanager
  # 可选：+libqmi-utils +libmbim-utils +iptables（用于 QMI/MBIM 兜底读取与网络诊断）
endef

define Package/simadmin/description
  SimAdmin 是一套 SIM/eSIM、蜂窝网络、短信、DDNS 与系统状态的 Web 管理系统。
  后端为 Rust 静态二进制，通过 ModemManager D-Bus 管理 modem，单进程托管前端 SPA。
  本 OpenWrt 包使用预编译静态二进制，运行时不依赖 OpenWrt 的 libc 版本。
endef

define Build/Prepare
	$(INSTALL_DIR) $(PKG_BUILD_DIR)
	@test -n "$(SIMADMIN_BIN)" || (echo "当前 ARCH=$(ARCH) 无匹配预编译二进制" && false)
	$(CP) $(SIMADMIN_BIN) $(PKG_BUILD_DIR)/simadmin
	$(CP) $(CURDIR)/www $(PKG_BUILD_DIR)/www
endef

define Build/Compile
	# 使用预编译静态二进制，无需编译
endef

define Package/simadmin/install
	$(INSTALL_DIR) $(1)/opt/simadmin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/simadmin $(1)/opt/simadmin/simadmin
	$(INSTALL_DIR) $(1)/opt/simadmin/www
	$(CP) $(PKG_BUILD_DIR)/www/. $(1)/opt/simadmin/www/
	$(INSTALL_DIR) $(1)/etc/simadmin
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) $(CURDIR)/files/etc/init.d/simadmin $(1)/etc/init.d/simadmin
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) $(CURDIR)/files/etc/config/simadmin $(1)/etc/config/simadmin
endef

define Package/simadmin/conffiles
/etc/config/simadmin
endef

$(eval $(call BuildPackage,simadmin))
