//! Human-readable conversation projection from the stable task/event protocol.
use serde_json::{Value, json};
pub fn terminal(state: &str) -> bool {
    matches!(state, "completed" | "failed" | "cancelled" | "interrupted")
}
pub fn excerpt(s: &str, n: usize) -> String {
    let s = s.lines().find(|s| !s.trim().is_empty()).unwrap_or(s).trim();
    let mut out: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        out.push('…')
    }
    out
}
pub fn merge(old: Option<&Value>, mut next: Value, cursor: u64) -> Value {
    if cursor > 0 {
        if let Some(old) = old {
            let mut events = old["events"].as_array().cloned().unwrap_or_default();
            if old["next_cursor"].as_u64().unwrap_or(0) <= cursor {
                events.extend(next["events"].as_array().cloned().unwrap_or_default());
            }
            next["events"] = json!(events);
        }
    }
    next
}
pub fn assistant_text(turn: &Value) -> String {
    if let Some(s) = turn["result"]["result"].as_str().filter(|s| !s.is_empty()) {
        return s.into();
    }
    let mut messages = Vec::<String>::new();
    for e in turn["events"].as_array().into_iter().flatten() {
        if e["type"] == "assistant" {
            for b in e["message"]["content"].as_array().into_iter().flatten() {
                if b["type"] == "text" {
                    if let Some(t) = b["text"].as_str() {
                        if messages.last().map(String::as_str) != Some(t) {
                            messages.push(t.into());
                        }
                    }
                }
            }
        }
    }
    messages.join("\n\n")
}
pub fn tools(turn: &Value) -> Vec<Value> {
    let mut out = vec![];
    for e in turn["events"].as_array().into_iter().flatten() {
        if e["type"] == "assistant" {
            for b in e["message"]["content"].as_array().into_iter().flatten() {
                if b["type"] == "tool_use" {
                    out.push(b.clone())
                }
            }
        }
    }
    out
}
pub fn tool_label(name: &str) -> &str {
    match name {
        "Read" => "读取文件",
        "Write" => "写入文件",
        "Edit" | "MultiEdit" => "修改文件",
        "Bash" => "执行命令",
        "Glob" => "查找文件",
        "Grep" => "搜索内容",
        "WebFetch" => "读取网页",
        "WebSearch" => "搜索网页",
        _ => name,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_state_and_readable_content() {
        assert!(terminal("completed"));
        assert!(!terminal("running"));
        let a = json!({"events":[{"type":"assistant","message":{"content":[{"type":"text","text":"hello"},{"type":"tool_use","name":"Read","input":{"file_path":"README.md"}}]}}],"next_cursor":10});
        let b = json!({"events":[{"type":"assistant","message":{"content":[{"type":"text","text":"world"}]}}],"next_cursor":20,"state":"completed","result":{"result":"# Final\n\nReadable"}});
        let merged = merge(Some(&a), b, 10);
        assert_eq!(merged["events"].as_array().unwrap().len(), 2);
        assert_eq!(assistant_text(&merged), "# Final\n\nReadable");
        assert_eq!(tools(&merged).len(), 1);
        assert_eq!(excerpt("你好世界", 2), "你好…");
    }
}
