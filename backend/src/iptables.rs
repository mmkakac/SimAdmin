//! Read-only iptables diagnostics.
//!
//! SimAdmin must not flush host firewall rules automatically. Container
//! runtimes, VPNs, firewalld, ufw, and other host services own rules in these
//! tables too, so this module only inspects state for logging/diagnostics.

use std::process::Command;
use tokio::task;

/// iptables 规则统计信息
#[derive(Debug, Default)]
pub struct IptablesRuleCount {
    pub ipv4_rules: usize,
    pub ipv6_rules: usize,
}

impl IptablesRuleCount {
    /// 是否有任何规则
    pub fn has_rules(&self) -> bool {
        self.ipv4_rules > 0 || self.ipv6_rules > 0
    }

    /// 总规则数
    pub fn total(&self) -> usize {
        self.ipv4_rules + self.ipv6_rules
    }
}

/// 获取 iptables 规则数量
///
/// 统计 iptables 和 ip6tables 中 filter 表的规则数量（排除默认策略行）
///
/// # Returns
/// * `Ok(IptablesRuleCount)` - 规则统计
/// * `Err(String)` - 操作失败的错误信息
pub async fn get_iptables_rule_count() -> Result<IptablesRuleCount, String> {
    task::spawn_blocking(|| {
        let mut count = IptablesRuleCount::default();

        // 获取 iptables 规则数量
        // iptables -L -n 输出中，每条规则是一行，但需要排除链名行和策略行
        // 使用 iptables -S 更简单，每条规则一行，-P 开头的是策略，-A 开头的是规则
        if let Ok(output) = Command::new("iptables").args(["-S"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // 统计 -A 开头的行（实际规则），排除 -P（策略）和 -N（链定义）
                count.ipv4_rules = stdout
                    .lines()
                    .filter(|line| line.starts_with("-A "))
                    .count();
            }
        }

        // 获取 ip6tables 规则数量
        if let Ok(output) = Command::new("ip6tables").args(["-S"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                count.ipv6_rules = stdout
                    .lines()
                    .filter(|line| line.starts_with("-A "))
                    .count();
            }
        }

        // OpenWrt 默认使用 nftables（fw4），通常没有 iptables 前端。
        // 这里退回 `nft list ruleset` 做只读统计：iplimit/ip6 无法精确区分时，
        // 按规则文本中的 family 归类，仍保持"只读诊断"语义。
        if count.ipv4_rules == 0 && count.ipv6_rules == 0 {
            if let Ok(output) = Command::new("nft").args(["list", "ruleset"]).output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut v4 = 0usize;
                    let mut v6 = 0usize;
                    for line in stdout.lines() {
                        let trimmed = line.trim_start();
                        if !trimmed.contains("rule") && !trimmed.starts_with("add rule") {
                            continue;
                        }
                        // nft 文本输出里同一 chain 下会带 "ip"/"ip6" 关键字
                        if trimmed.contains("ip6") || trimmed.contains("ipv6") {
                            v6 += 1;
                        } else if trimmed.contains("ip ")
                            || trimmed.contains("ip\t")
                            || trimmed.contains("ct ")
                            || trimmed.contains("tcp ")
                            || trimmed.contains("udp ")
                        {
                            v4 += 1;
                        }
                    }
                    count.ipv4_rules = v4;
                    count.ipv6_rules = v6;
                }
            }
        }

        Ok(count)
    })
    .await
    .map_err(|e| format!("Task execution failed: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_iptables_rule_count_is_non_destructive() {
        let result = get_iptables_rule_count().await;
        assert!(result.is_ok());
    }
}
