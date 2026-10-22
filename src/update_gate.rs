//! In-process drain barrier. A lease spans a waiting MCP call, not one socket poll.
use crate::common::{now, valid_id};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Default)]
pub struct UpdateGate {
    pub draining: bool,
    pub active: usize,
    pub legacy_until: u64,
    leases: BTreeMap<String, u64>,
    operations: BTreeMap<String, usize>,
}
impl UpdateGate {
    pub fn lease(&mut self, op: &str, id: &str) -> Result<Value> {
        valid_id(id)?;
        self.prune();
        if op == "client_activity_end" {
            self.leases.remove(id);
        } else {
            if self.draining {
                bail!("App 正在准备更新，请稍后重试原任务查询")
            }
            self.leases.insert(id.into(), now() + 120);
        }
        Ok(json!({"lease_id":id,"expires_in":120}))
    }
    fn prune(&mut self) {
        self.leases.retain(|_, until| *until > now());
    }
    pub fn snapshot(&mut self) -> Value {
        self.prune();
        json!({"draining":self.draining,"active_operations":self.active,"mcp_leases":self.leases.len(),"legacy_wait_seconds":self.legacy_until.saturating_sub(now())})
    }
    pub fn begin(&mut self, operation: &str) -> Result<()> {
        if self.draining {
            bail!("App 正在准备安装更新，请安装完成后再开始新操作")
        }
        self.active += 1;
        *self.operations.entry(operation.into()).or_default() += 1;
        Ok(())
    }
    pub fn finish(&mut self, operation: &str) {
        self.active = self.active.saturating_sub(1);
        if let Some(count) = self.operations.get_mut(operation) {
            *count -= 1;
            if *count == 0 {
                self.operations.remove(operation);
            }
        }
    }
    pub fn prepare(&mut self) -> Result<()> {
        self.prune();
        if self.legacy_until > now() {
            bail!(
                "Codex 的旧版请求仍可能在等待结果。请等待约 {} 秒后重试安装，不需要重启 Codex。",
                self.legacy_until.saturating_sub(now())
            )
        }
        if self.active > 0 {
            let names = self
                .operations
                .keys()
                .map(|s| operation_label(s))
                .collect::<Vec<_>>()
                .join("、");
            bail!(
                "暂不能安装：{}正在进行。完成后再点“安装并重启”。",
                if names.is_empty() {
                    "后台操作"
                } else {
                    &names
                }
            );
        }
        if !self.leases.is_empty() {
            bail!(
                "Codex 正在等待远端结果（{} 个请求）。请等这些请求返回后重试安装，不需要退出 Codex。",
                self.leases.len()
            );
        }
        self.draining = true;
        Ok(())
    }
}
fn operation_label(name: &str) -> &str {
    match name {
        "clone_into_directory" => "克隆仓库",
        "git_update" => "Git 更新",
        "branch_fetch" => "获取分支",
        "branch_change" => "切换分支",
        "connect" => "SSH 连接",
        "disconnect" => "断开 SSH",
        "install_runner" => "安装 Runner",
        "db_test" | "db_protocol_test" => "数据库检测",
        "file_mutate" | "trash_restore" | "file_create" | "create_directory" => "文件操作",
        "start" | "reply" => "发送任务",
        "git_vps_auth_install" | "git_vps_auth_remove" => "VPS 认证配置",
        "git_auth_save" | "git_auth_remove" | "git_auth_migrate" => "Git 认证配置",
        "save_profile" | "preferences_save" | "set_auto" | "load_profile" => "保存配置",
        _ => "后台操作",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leases_and_writes_block_install() {
        let mut g = UpdateGate::default();
        let id = uuid::Uuid::new_v4().to_string();
        g.lease("client_activity_begin", &id).unwrap();
        assert!(g.prepare().is_err());
        g.lease("client_activity_end", &id).unwrap();
        g.begin("git_update").unwrap();
        assert!(g.prepare().is_err());
        g.finish("git_update");
        g.prepare().unwrap();
        assert!(g.begin("git_update").is_err());
        assert!(g.lease("client_activity_begin", &id).is_err());
    }
}
