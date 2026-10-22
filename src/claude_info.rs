//! Read-only Claude control handshake and last-reported quota. Never reads auth files.
use crate::common::*;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

pub fn models(claude: &str) -> Result<Value> {
    if !claude.starts_with('/') {
        bail!("远端未找到 Claude Code，请先检测运行环境")
    }
    let mut child = Command::new(claude)
        .args([
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--no-session-persistence",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--settings",
            "{\"disableAllHooks\":true}",
        ])
        .current_dir(home()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("无法启动 Claude 模型列表探测")?;
    let output = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(output);
        let found = (|| -> Result<Value> {
            loop {
                let mut line = Vec::new();
                let n = reader
                    .by_ref()
                    .take(1024 * 1024 + 1)
                    .read_until(b'\n', &mut line)?;
                if n == 0 {
                    bail!("Claude 未返回模型列表，请检查登录状态和版本")
                }
                if n > 1024 * 1024 {
                    bail!("Claude 初始化响应过大")
                }
                let v: Value = serde_json::from_slice(&line)?;
                if v["type"] == "control_response"
                    && v["response"]["request_id"] == "ccbridge-models"
                {
                    if v["response"]["subtype"] != "success" {
                        bail!("Claude 不支持模型列表初始化，请更新 Claude Code")
                    }
                    return sanitize_models(&v["response"]["response"]);
                }
            }
        })();
        let _ = tx.send(found);
    });
    let result = (|| -> Result<Value> {
        writeln!(
            child.stdin.as_mut().unwrap(),
            "{}",
            json!({"type":"control_request","request_id":"ccbridge-models","request":{"subtype":"initialize","hooks":{}}})
        )?;
        child.stdin.as_mut().unwrap().flush()?;
        rx.recv_timeout(Duration::from_secs(20))
            .context("获取 Claude 模型列表超时，请重试")?
    })();
    // Only our prompt-free probe is terminated. No session or other task is touched.
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    result
}
fn sanitize_models(v: &Value) -> Result<Value> {
    let input = v["models"]
        .as_array()
        .context("此 Claude 版本没有返回可选模型，请更新 Claude Code")?;
    let mut models = Vec::new();
    for row in input.iter().take(100) {
        let Some(value) = row["value"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        if crate::models::validate(value).is_err()
            || models.iter().any(|x: &Value| x["value"] == value)
        {
            continue;
        }
        let label = row["displayName"].as_str().unwrap_or(value);
        let resolved = row["resolvedModel"]
            .as_str()
            .filter(|s| crate::models::validate(s).is_ok());
        models.push(json!({"value":value,"label":label.chars().take(160).collect::<String>(),"resolved":resolved}));
    }
    if models.is_empty() {
        bail!("Claude 未返回可选模型，请检查当前账号")
    }
    Ok(json!({"models":models,"checked_at":now()}))
}

fn quota_event(info: &Value, at: u64, windows: &mut serde_json::Map<String, Value>) {
    for key in ["five_hour", "seven_day"] {
        let w = &info["unifiedWindows"][key];
        let reset = w["resetsAt"].as_u64();
        let utilization = w["utilization"]
            .as_f64()
            .filter(|n| n.is_finite() && (0.0..=1.0).contains(n));
        if let (Some(reset), Some(n)) = (reset, utilization) {
            if windows
                .get(key)
                .is_none_or(|old| old["observed_at"].as_u64().unwrap_or(0) <= at)
            {
                windows.insert(
                    key.into(),
                    json!({"used_percentage":n*100.0,"resets_at":reset,"observed_at":at}),
                );
            }
        }
    }
}
/// Account-local events only; no credential/OAuth endpoints or inference calls.
pub fn usage_at(root: &Path) -> Result<Value> {
    let mut tasks = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() || valid_id(&entry.file_name().to_string_lossy()).is_err() {
            continue;
        }
        if let Ok(t) = read_json::<Value>(&entry.path().join("task.json")) {
            tasks.push((t["updated_at"].as_u64().unwrap_or(0), entry.path()));
        }
    }
    tasks.sort_by_key(|t| std::cmp::Reverse(t.0));
    let mut windows = serde_json::Map::new();
    for (at, dir) in tasks.into_iter().take(100) {
        if let Ok(mut f) = File::open(dir.join("events.jsonl")) {
            let len = f.metadata()?.len();
            let start = len.saturating_sub(512 * 1024);
            f.seek(SeekFrom::Start(start))?;
            let mut bytes = Vec::new();
            f.take(512 * 1024).read_to_end(&mut bytes)?;
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines().skip(if start > 0 { 1 } else { 0 }) {
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    if v["type"] == "rate_limit_event" {
                        quota_event(&v["rate_limit_info"], at, &mut windows)
                    }
                }
            }
        }
    }
    Ok(json!({"windows":windows,"checked_at":now(),"source":"claude_events"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_free_handshake_reaps_child() {
        let fixture = format!("{}/tests/fake_claude.py", env!("CARGO_MANIFEST_DIR"));
        let start = std::time::Instant::now();
        let result = models(&fixture).unwrap();
        assert_eq!(result["models"][1]["value"], "fixture-fast");
        assert!(start.elapsed() < Duration::from_secs(10));
    }
    #[test]
    fn account_events_are_scoped_and_do_not_expose_task_content() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let d = root.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&d).unwrap();
        write_json(
            &d.join("task.json"),
            &json!({"updated_at":10,"prompt":"private"}),
        )
        .unwrap();
        std::fs::write(d.join("events.jsonl"), format!("{}\n",json!({"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.5,"resetsAt":100}}}}))).unwrap();
        let r = usage_at(&root).unwrap();
        assert_eq!(r["windows"]["five_hour"]["used_percentage"], 50.0);
        assert!(!r.to_string().contains("private"));
        let empty = root.join("other-account");
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(usage_at(&empty).unwrap()["windows"], json!({}));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn models_are_remote_values_not_history_or_secrets() {
        let v = sanitize_models(&json!({"account":{"token":"secret"},"models":[{"value":"opus","displayName":"Opus","resolvedModel":"model-real"},{"value":"opus"},{"value":"--bad"}]})).unwrap();
        assert_eq!(v["models"].as_array().unwrap().len(), 1);
        assert_eq!(v["models"][0]["resolved"], "model-real");
        assert!(!v.to_string().contains("secret"));
        assert!(sanitize_models(&json!({})).is_err());
    }
    #[test]
    fn quota_preserves_zero_missing_windows_and_latest_observation() {
        let mut out = serde_json::Map::new();
        quota_event(
            &json!({"unifiedWindows":{"five_hour":{"utilization":0.0,"resetsAt":100}}}),
            5,
            &mut out,
        );
        quota_event(
            &json!({"unifiedWindows":{"five_hour":{"utilization":0.9,"resetsAt":100}}}),
            4,
            &mut out,
        );
        assert_eq!(out["five_hour"]["used_percentage"], 0.0);
        assert!(!out.contains_key("seven_day"));
        quota_event(
            &json!({"unifiedWindows":{"seven_day":{"utilization":1.5,"resetsAt":100}}}),
            8,
            &mut out,
        );
        assert!(!out.contains_key("seven_day"));
    }
}
