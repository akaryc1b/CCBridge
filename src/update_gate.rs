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
    pub fn begin(&mut self) -> Result<()> {
        if self.draining {
            bail!("App 正在更新排空，请稍后再开始新操作")
        }
        self.active += 1;
        Ok(())
    }
    pub fn prepare(&mut self) -> Result<()> {
        self.prune();
        if self.legacy_until > now() {
            bail!("旧版 MCP 等待窗口尚未结束，请稍后更新；重新加载 MCP 后新请求使用租约协议")
        }
        if self.active > 0 || !self.leases.is_empty() {
            bail!("仍有活动操作或 MCP 等待，请完成后再安装更新")
        }
        self.draining = true;
        Ok(())
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
        g.begin().unwrap();
        assert!(g.prepare().is_err());
        g.active -= 1;
        g.prepare().unwrap();
        assert!(g.begin().is_err());
        assert!(g.lease("client_activity_begin", &id).is_err());
    }
}
