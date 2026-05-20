use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Aggregated token usage for one session, summed across every assistant
/// message in its transcript (main thread plus subagent sidechains).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub web_search_requests: i64,
    pub web_fetch_requests: i64,
    /// Number of assistant messages (API requests) counted.
    pub api_calls: i64,
    /// Model driving the most API calls in the transcript.
    pub model: Option<String>,
}

impl SessionUsage {
    /// Total billable tokens — input + output + both cache tiers. Input legitimately
    /// repeats across API calls because each call is billed for its full input.
    pub fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }
}

/// Parse a Claude Code transcript JSONL file into aggregated token usage.
///
/// Each line is one transcript entry. Assistant entries carry a `message.usage`
/// block; those are summed. Non-assistant entries and synthetic entries (API-error
/// placeholders, model `<synthetic>`) are skipped. Unparseable lines are skipped
/// silently — a truncated tail must not discard the rest of the file.
pub fn parse_transcript(path: &Path) -> Result<SessionUsage> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut usage = SessionUsage::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut model_calls: HashMap<String, i64> = HashMap::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if entry.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }

        // Dedup on the transcript entry uuid — guards against duplicate lines.
        if let Some(uuid) = entry.get("uuid").and_then(|v| v.as_str()) {
            if !seen.insert(uuid.to_string()) {
                continue;
            }
        }

        let message = match entry.get("message") {
            Some(m) => m,
            None => continue,
        };

        // Skip synthetic entries — API-error placeholders carry model "<synthetic>".
        let model = message.get("model").and_then(|v| v.as_str()).unwrap_or("");
        if model.is_empty() || model.starts_with('<') {
            continue;
        }

        let u = match message.get("usage") {
            Some(u) => u,
            None => continue,
        };

        let get = |key: &str| u.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        usage.input_tokens += get("input_tokens");
        usage.output_tokens += get("output_tokens");
        usage.cache_creation_tokens += get("cache_creation_input_tokens");
        usage.cache_read_tokens += get("cache_read_input_tokens");

        if let Some(st) = u.get("server_tool_use") {
            usage.web_search_requests += st
                .get("web_search_requests")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            usage.web_fetch_requests += st
                .get("web_fetch_requests")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
        }

        usage.api_calls += 1;
        *model_calls.entry(model.to_string()).or_insert(0) += 1;
    }

    // Dominant model = the one driving the most API calls.
    usage.model = model_calls
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(model, _)| model);

    Ok(usage)
}
