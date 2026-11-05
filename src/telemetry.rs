//! Receipt-time streaming metrics. No model content or credentials are persisted here.
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
#[derive(Default)]
struct Request {
    tokens: u64,
    first: Option<u64>,
    last: u64,
    estimated: f64,
}
#[derive(Default)]
pub struct StreamMetrics {
    requests: BTreeMap<String, Request>,
    current: String,
    samples: VecDeque<(u64, f64)>,
    token_samples: VecDeque<(u64, u64)>,
    phase: &'static str,
    observed: u64,
    average: Option<f64>,
}
impl StreamMetrics {
    pub fn accept(&mut self, frame: &Value, at: u64) {
        // Subagents have their own generation; never blend their counters into the main reply.
        if !frame["parent_tool_use_id"].is_null() {
            return;
        }
        let event = if frame["type"] == "stream_event" {
            &frame["event"]
        } else {
            frame
        };
        self.observed = at;
        match event["type"].as_str().unwrap_or("") {
            "message_start" => {
                self.current = event["message"]["id"].as_str().unwrap_or("").into();
                if !self.current.is_empty() {
                    self.requests.entry(self.current.clone()).or_default();
                }
                self.samples.clear();
                self.token_samples.clear();
                self.phase = "waiting";
            }
            "content_block_start" => {
                self.phase = match event["content_block"]["type"].as_str() {
                    Some("tool_use") => "tool",
                    Some("thinking") => "thinking",
                    _ => "waiting",
                };
                self.samples.clear();
            }
            "content_block_delta" => {
                let delta = &event["delta"];
                if delta["type"] == "thinking_delta" {
                    self.phase = "thinking";
                    self.samples.clear();
                    return;
                }
                if delta["type"] != "text_delta" {
                    return;
                }
                let text = delta["text"].as_str().unwrap_or("");
                if text.is_empty() || self.current.is_empty() {
                    return;
                }
                let r = self.requests.entry(self.current.clone()).or_default();
                if r.first.is_none() {
                    r.first = Some(at);
                    self.token_samples.push_back((at, r.tokens));
                }
                if self.phase != "generating" {
                    self.token_samples.clear();
                    self.token_samples.push_back((at, r.tokens));
                    self.samples.clear();
                    self.samples.push_back((at, r.estimated));
                }
                self.phase = "generating";
                // Explicit heuristic, not a tokenizer: ASCII ~4 chars/token, other scripts ~1/token.
                r.estimated += text
                    .chars()
                    .map(|c| if c.is_ascii() { 0.25 } else { 1.0 })
                    .sum::<f64>();
                r.last = at;
                self.samples.push_back((at, r.estimated));
                while self.samples.len() > 2
                    && self
                        .samples
                        .get(1)
                        .is_some_and(|s| at.saturating_sub(s.0) > 5000)
                {
                    self.samples.pop_front();
                }
                // Bound rapid event streams without preserving message text.
                if self.samples.len() > 512 {
                    self.samples.remove(1);
                }
            }
            "message_delta" => {
                if let Some(r) = self.requests.get_mut(&self.current) {
                    let next = event["usage"]["output_tokens"].as_u64().unwrap_or(0);
                    if next > r.tokens && self.phase == "generating" {
                        self.token_samples.push_back((at, next));
                        while self.token_samples.len() > 2
                            && self
                                .token_samples
                                .get(1)
                                .is_some_and(|s| at.saturating_sub(s.0) > 5000)
                        {
                            self.token_samples.pop_front();
                        }
                        if self.token_samples.len() > 512 {
                            self.token_samples.remove(1);
                        }
                    }
                    r.tokens = r.tokens.max(next);
                }
            }
            "message_stop" => {
                self.phase = "waiting";
                self.samples.clear();
            }
            "result" => {
                self.average = event["duration_api_ms"]
                    .as_f64()
                    .filter(|ms| *ms > 0.0)
                    .zip(event["usage"]["output_tokens"].as_f64())
                    .map(|(ms, tokens)| tokens * 1000.0 / ms);
                self.phase = "finished";
                self.samples.clear();
            }
            _ => {}
        }
    }
    pub fn snapshot(&self) -> Value {
        let rate = if self.phase == "generating" {
            self.samples
                .front()
                .zip(self.samples.back())
                .and_then(|(a, b)| {
                    let ms = b.0.saturating_sub(a.0);
                    (ms >= 250 && self.observed.saturating_sub(b.0) < 1500)
                        .then(|| (b.1 - a.1) * 1000.0 / ms as f64)
                })
        } else {
            None
        };
        let measured = if self.phase == "generating" {
            self.token_samples
                .front()
                .zip(self.token_samples.back())
                .and_then(|(a, b)| {
                    let ms = b.0.saturating_sub(a.0);
                    (ms >= 250 && self.observed.saturating_sub(b.0) < 1500)
                        .then(|| b.1.saturating_sub(a.1) as f64 * 1000.0 / ms as f64)
                })
        } else {
            None
        };
        let source = if measured.is_some() {
            "reported_usage"
        } else if rate.is_some() {
            "text_heuristic"
        } else {
            "unavailable"
        };
        json!({"phase":if self.phase.is_empty(){"waiting"}else{self.phase},"observed_at_ms":self.observed,
            "tokens_per_second":measured.or(rate),"speed_source":source,
            "average_tokens_per_second":self.average,"output_tokens":self.requests.values().map(|r|r.tokens).sum::<u64>()})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn start(m: &mut StreamMetrics, id: &str) {
        m.accept(
            &json!({"type":"stream_event","event":{"type":"message_start","message":{"id":id}}}),
            0,
        );
    }
    fn text(m: &mut StreamMetrics, s: &str, t: u64) {
        m.accept(&json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":s}}}),t);
    }
    #[test]
    fn shows_speed_before_completion_and_clears_during_tools() {
        let mut m = StreamMetrics::default();
        start(&mut m, "a");
        text(&mut m, "abcd", 1000);
        text(&mut m, "efgh", 2000);
        assert_eq!(m.snapshot()["phase"], "generating");
        assert!(m.snapshot()["tokens_per_second"].as_f64().unwrap() > 0.0);
        assert_eq!(m.snapshot()["speed_source"], "text_heuristic");
        m.accept(&json!({"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"tool_use"}}}),2500);
        assert_eq!(m.snapshot()["phase"], "tool");
        assert!(m.snapshot()["tokens_per_second"].is_null());
    }
    #[test]
    fn prefers_reported_counter_when_available_mid_stream() {
        let mut m = StreamMetrics::default();
        start(&mut m, "a");
        text(&mut m, "a", 1000);
        text(&mut m, "b", 2000);
        m.accept(
            &json!({"type":"message_delta","usage":{"output_tokens":20}}),
            2000,
        );
        assert_eq!(m.snapshot()["speed_source"], "reported_usage");
        assert_eq!(m.snapshot()["tokens_per_second"], 20.0);
    }
    #[test]
    fn cumulative_usage_and_repeated_ids_are_not_summed() {
        let mut m = StreamMetrics::default();
        start(&mut m, "a");
        for n in [40, 120, 310, 310] {
            m.accept(&json!({"type":"stream_event","event":{"type":"message_delta","usage":{"output_tokens":n}}}),1);
        }
        start(&mut m, "a");
        assert_eq!(m.snapshot()["output_tokens"], 310);
        start(&mut m, "b");
        m.accept(
            &json!({"type":"message_delta","usage":{"output_tokens":20}}),
            3,
        );
        assert_eq!(m.snapshot()["output_tokens"], 330);
    }
    #[test]
    fn final_or_thinking_never_show_stale_live_speed() {
        let mut m = StreamMetrics::default();
        start(&mut m, "a");
        text(&mut m, "中文", 100);
        text(&mut m, "输出", 1100);
        m.accept(&json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta"}}}),1200);
        assert!(m.snapshot()["tokens_per_second"].is_null());
        m.accept(&json!({"type":"result"}), 1300);
        assert_eq!(m.snapshot()["phase"], "finished");
    }
}
