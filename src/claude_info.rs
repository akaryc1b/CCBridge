//! Prompt-free Claude control queries and explicit historical quota. Never reads auth files.
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
    control(claude, false)
}
fn control(claude: &str, usage: bool) -> Result<Value> {
    use std::os::unix::process::CommandExt;
    if !claude.starts_with('/') {
        bail!("远端未找到 Claude Code，请先检测运行环境");
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
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("无法启动 Claude 信息查询")?;
    let group = child.id() as i32;
    let output = child.stdout.take().unwrap();
    let mut input = child.stdin.take().unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let found = (|| -> Result<Value> {
            writeln!(
                input,
                "{}",
                json!({"type":"control_request","request_id":"ccbridge-models","request":{"subtype":"initialize","hooks":{}}})
            )?;
            input.flush()?;
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                let n = reader
                    .by_ref()
                    .take(1024 * 1024 + 1)
                    .read_until(b'\n', &mut line)?;
                if n == 0 {
                    bail!("Claude 未返回查询结果，请检查登录状态和版本");
                }
                if n > 1024 * 1024 {
                    bail!("Claude 查询响应过大");
                }
                let v: Value = serde_json::from_slice(&line)?;
                if v["type"] != "control_response" {
                    continue;
                }
                if v["response"]["request_id"] == "ccbridge-models" {
                    if v["response"]["subtype"] != "success" {
                        bail!("此 Claude 版本不支持初始化查询");
                    }
                    if !usage {
                        return sanitize_models(&v["response"]["response"]);
                    }
                    writeln!(
                        input,
                        "{}",
                        json!({"type":"control_request","request_id":"ccbridge-usage","request":{"subtype":"get_usage","skip_behaviors":true}})
                    )?;
                    input.flush()?;
                } else if usage && v["response"]["request_id"] == "ccbridge-usage" {
                    if v["response"]["subtype"] != "success" {
                        bail!("Claude 账户用量查询失败；请检查登录、网络或 Claude 版本");
                    }
                    return sanitize_usage(&v["response"]["response"]);
                }
            }
        })();
        let _ = tx.send(found);
    });
    let result = rx
        .recv_timeout(Duration::from_secs(30))
        .context("查询 Claude 信息超时，请重试");
    // No user prompt is sent. Reap only our private probe and its inherited pipes.
    unsafe {
        libc::kill(-group, libc::SIGKILL);
    }
    let _ = child.wait();
    let _ = reader.join();
    result?
}
fn sanitize_usage(v: &Value) -> Result<Value> {
    let available = v["rate_limits_available"]
        .as_bool()
        .context("Claude 未返回可识别的额度接口")?;
    let mut windows = serde_json::Map::new();
    for key in [
        "five_hour",
        "seven_day",
        "seven_day_opus",
        "seven_day_sonnet",
        "seven_day_oauth_apps",
    ] {
        let row = &v["rate_limits"][key];
        if let Some(n) = row["utilization"]
            .as_f64()
            .filter(|n| n.is_finite() && (0.0..=100.0).contains(n))
        {
            windows.insert(key.into(),json!({"used_percentage":n,"resets_at_iso":row["resets_at"].as_str(),"observed_at":now()}));
        }
    }
    Ok(
        json!({"state":if !available{"not_applicable"}else if windows.is_empty(){"unavailable"}else{"reported"},
        "windows":windows,"checked_at":now(),"source":"claude_account","live":true}),
    )
}
/// Claude performs its own authenticated query. We never read or forward OAuth secrets.
pub fn usage(claude: &str, root: &Path) -> Result<Value> {
    let cache = root.join("claude-usage.json");
    if let Ok(v) = read_json::<Value>(&cache) {
        if v["checked_at"]
            .as_u64()
            .is_some_and(|t| now().saturating_sub(t) < 60)
            && v["source"] == "claude_account"
        {
            return account_or_history(v, root);
        }
    }
    match control(claude, true) {
        Ok(v) => {
            write_json(&cache, &v)?;
            account_or_history(v, root)
        }
        Err(error) => {
            let mut historical = usage_at(root)?;
            historical["live_error"] = json!(error.to_string());
            Ok(historical)
        }
    }
}
fn account_or_history(account: Value, root: &Path) -> Result<Value> {
    if account["state"] == "reported" {
        return Ok(account);
    }
    let mut historical = usage_at(root)?;
    historical["account_state"] = account["state"].clone();
    historical["account_checked_at"] = account["checked_at"].clone();
    Ok(historical)
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
    fn insert(key: &str, w: &Value, at: u64, windows: &mut serde_json::Map<String, Value>) {
        if ![
            "five_hour",
            "seven_day",
            "seven_day_opus",
            "seven_day_sonnet",
        ]
        .contains(&key)
        {
            return;
        }
        let reset = w["resetsAt"].as_u64();
        let utilization = w["utilization"]
            .as_f64()
            .filter(|n| n.is_finite() && (0.0..=1.0).contains(n));
        if let (Some(reset), Some(n)) = (reset, utilization) {
            if windows
                .get(key)
                .is_none_or(|old| at > 0 && old["observed_at"].as_u64().unwrap_or(0) <= at)
            {
                windows.insert(
                    key.into(),
                    json!({"used_percentage":n*100.0,
                    "resets_at":reset,"observed_at":if at > 0 {Some(at)} else {None}}),
                );
            }
        }
    }
    // Both shapes are emitted by Claude. Missing utilization is unknown, not 0%.
    for key in [
        "five_hour",
        "seven_day",
        "seven_day_opus",
        "seven_day_sonnet",
    ] {
        insert(key, &info["unifiedWindows"][key], at, windows);
    }
    if let Some(key) = info["rateLimitType"].as_str() {
        insert(key, info, at, windows);
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
    let mut tasks_scanned = 0;
    let mut quota_events = 0;
    let mut recognized_events = 0;
    for (_, dir) in tasks.into_iter().take(100) {
        if let Ok(mut f) = File::open(dir.join("events.jsonl")) {
            tasks_scanned += 1;
            let len = f.metadata()?.len();
            let start = len.saturating_sub(512 * 1024);
            f.seek(SeekFrom::Start(start))?;
            let mut bytes = Vec::new();
            (&mut f).take(512 * 1024).read_to_end(&mut bytes)?;
            // A quota event often precedes a large tool response. Inspect the head
            // as well as the tail, with a fixed total I/O budget per legacy task.
            let head = if start > 0 {
                f.seek(SeekFrom::Start(0))?;
                let mut head = Vec::new();
                f.take(start.min(512 * 1024)).read_to_end(&mut head)?;
                head
            } else {
                Vec::new()
            };
            let text = String::from_utf8_lossy(&bytes);
            for line in text
                .lines()
                .skip(if start > 0 { 1 } else { 0 })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .chain(String::from_utf8_lossy(&head).lines().rev())
            {
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    if v["type"] == "rate_limit_event" {
                        quota_events += 1;
                        let mut sample = serde_json::Map::new();
                        quota_event(&v["rate_limit_info"], 0, &mut sample);
                        if !sample.is_empty() {
                            recognized_events += 1;
                        }
                        quota_event(
                            &v["rate_limit_info"],
                            v["_ccbridge_observed_at"].as_u64().unwrap_or(0),
                            &mut windows,
                        )
                    }
                }
            }
        }
    }
    Ok(
        json!({"state":if !windows.is_empty(){"reported"}else if quota_events>0{"unrecognized"}else{"no_events"},
        "windows":windows,"checked_at":now(),"source":"claude_events","live":false,
        "tasks_scanned":tasks_scanned,"quota_events":quota_events,"recognized_events":recognized_events}),
    )
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
    fn prompt_free_usage_control_queries_account_percent() {
        let fixture = format!("{}/tests/fake_claude.py", env!("CARGO_MANIFEST_DIR"));
        let r = control(&fixture, true).unwrap();
        assert_eq!(r["source"], "claude_account");
        assert_eq!(r["windows"]["five_hour"]["used_percentage"], 10.0);
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
    fn direct_quota_events_and_missing_utilization_are_distinct() {
        let mut out = serde_json::Map::new();
        quota_event(
            &json!({"rateLimitType":"five_hour","utilization":0.1,"resetsAt":100}),
            5,
            &mut out,
        );
        assert_eq!(out["five_hour"]["used_percentage"], 10.0);
        quota_event(
            &json!({"rateLimitType":"seven_day","status":"allowed","resetsAt":100}),
            6,
            &mut out,
        );
        assert!(!out.contains_key("seven_day"));
        quota_event(
            &json!({"rateLimitType":"seven_day_opus","utilization":0.0,"resetsAt":100}),
            6,
            &mut out,
        );
        assert_eq!(out["seven_day_opus"]["used_percentage"], 0.0);
        quota_event(
            &json!({"rateLimitType":"five_hour","utilization":0.9,"resetsAt":100}),
            0,
            &mut out,
        );
        assert_eq!(out["five_hour"]["used_percentage"], 10.0);
    }
    #[test]
    fn quota_at_task_head_is_found_and_task_timestamp_is_not_observation() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let d = root.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&d).unwrap();
        write_json(&d.join("task.json"), &json!({"updated_at":99999})).unwrap();
        let event = json!({"type":"rate_limit_event","_ccbridge_observed_at":123,
            "rate_limit_info":{"rateLimitType":"five_hour","utilization":0.1,"resetsAt":999999}});
        std::fs::write(
            d.join("events.jsonl"),
            format!("{event}\n{}\n", "x".repeat(600 * 1024)),
        )
        .unwrap();
        let r = usage_at(&root).unwrap();
        assert_eq!(r["windows"]["five_hour"]["used_percentage"], 10.0);
        assert_eq!(r["windows"]["five_hour"]["observed_at"], 123);
        assert_eq!(r["quota_events"], 1);
        assert_eq!(r["live"], false);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unavailable_account_retains_explicitly_labeled_history() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let r =
            account_or_history(json!({"state":"not_applicable","checked_at":10}), &root).unwrap();
        assert_eq!(r["account_state"], "not_applicable");
        assert_eq!(r["source"], "claude_events");
        assert_eq!(r["windows"], json!({}));
        assert_eq!(r["live"], false);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn live_usage_percent_is_not_event_fraction_and_secrets_are_not_returned() {
        let r=sanitize_usage(&json!({"rate_limits_available":true,"subscription_type":"max",
            "session":{"private":"secret"},"rate_limits":{"five_hour":{"utilization":10.0,"resets_at":"2026-10-22T20:00:00Z"}}})).unwrap();
        assert_eq!(r["windows"]["five_hour"]["used_percentage"], 10.0);
        assert_eq!(r["live"], true);
        assert!(!r.to_string().contains("secret"));
        assert_eq!(
            sanitize_usage(&json!({"rate_limits_available":false})).unwrap()["state"],
            "not_applicable"
        );
        assert_eq!(
            sanitize_usage(&json!({"rate_limits_available":true,"rate_limits":null})).unwrap()["state"],
            "unavailable"
        );
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
