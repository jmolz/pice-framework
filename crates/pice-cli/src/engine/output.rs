use pice_daemon::orchestrator::{NoticeLevel, SharedSink, StreamEvent, StreamSink};
use pice_protocol::{CriterionScore, EvaluateResultParams};
use std::sync::Arc;

/// Print a streaming text chunk to the terminal (no newline — the chunk itself contains formatting).
pub fn print_chunk(text: &str) {
    use std::io::Write;
    print!("{text}");
    std::io::stdout().flush().ok();
}

/// Stream sink that writes chunks to stdout and advisory events to stderr.
///
/// T12-era CLI adapter for `pice_daemon::orchestrator::StreamSink`. Used by
/// workflow commands (prime, plan, execute, review, handoff) that stream
/// model output directly to the terminal in v0.1 parity mode.
///
/// Events are written to stderr so JSON-mode stdout remains a single valid
/// JSON object. T22 will relocate this to `pice-cli/src/adapter/sink.rs`
/// once the adapter module exists.
pub struct TerminalSink;

impl StreamSink for TerminalSink {
    fn send_chunk(&self, text: &str) {
        print_chunk(text);
    }

    fn send_event(&self, event: StreamEvent) {
        match event {
            StreamEvent::Notice { level, message } => {
                let prefix = match level {
                    NoticeLevel::Info => "info:",
                    NoticeLevel::Warn => "warning:",
                    NoticeLevel::Error => "error:",
                    // Forward-compat: `NoticeLevel` is `#[non_exhaustive]` so
                    // T19/T21 can add `Debug`/`Trace` without breaking this match.
                    _ => "notice:",
                };
                eprintln!("{prefix} {message}");
            }
            // Forward-compat: `StreamEvent` is `#[non_exhaustive]`. New variants
            // added in T19+ will land here and log at debug level until a
            // concrete renderer is wired up.
            _ => {
                tracing::debug!("TerminalSink: unhandled StreamEvent variant");
            }
        }
    }
}

/// Convenience factory returning an `Arc<TerminalSink>` as a `SharedSink`.
pub fn terminal_sink() -> SharedSink {
    Arc::new(TerminalSink)
}

/// Print evaluation results as a formatted table.
pub fn print_evaluation_report(
    primary_results: &EvaluateResultParams,
    adversarial_results: Option<&serde_json::Value>,
    tier: u8,
) {
    println!();
    println!("\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}");
    println!("\u{2551}       Evaluation Report \u{2014} Tier {tier}      \u{2551}");
    println!("\u{2560}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2563}");

    // Print each criterion score
    for score in &primary_results.scores {
        let status = if score.passed { "\u{2705}" } else { "\u{274c}" };
        println!(
            "\u{2551} {status} {name:<28} {score:>2}/{threshold:<2} \u{2551}",
            name = truncate(&score.name, 28),
            score = score.score,
            threshold = score.threshold,
        );
        if let Some(findings) = &score.findings {
            for line in findings.lines().take(2) {
                println!("\u{2551}   {line:<35} \u{2551}", line = truncate(line, 35));
            }
        }
    }

    // Adversarial findings
    if let Some(adversarial) = adversarial_results {
        println!("\u{2560}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2563}");
        println!("\u{2551}  Adversarial Review              \u{2551}");
        if let Some(challenges) = adversarial
            .get("designChallenges")
            .and_then(|c| c.as_array())
        {
            for challenge in challenges {
                let severity = challenge
                    .get("severity")
                    .and_then(|s| s.as_str())
                    .unwrap_or("?");
                let finding = challenge
                    .get("finding")
                    .and_then(|f| f.as_str())
                    .unwrap_or("");
                println!(
                    "\u{2551}  [{severity}] {finding}",
                    finding = truncate(finding, 28)
                );
            }
        }
    }

    // Overall result
    println!("\u{2560}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2563}");
    let overall = if primary_results.passed {
        "PASS \u{2705}"
    } else {
        "FAIL \u{274c}"
    };
    println!("\u{2551}  Overall: {overall:<27} \u{2551}");
    if let Some(summary) = &primary_results.summary {
        println!("\u{2551}  {summary}", summary = truncate(summary, 36));
    }
    println!("\u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}");
}

/// Build JSON output for --json mode.
pub fn evaluation_json(
    primary_results: &EvaluateResultParams,
    adversarial_results: Option<&serde_json::Value>,
    tier: u8,
) -> serde_json::Value {
    let scores: Vec<serde_json::Value> = primary_results
        .scores
        .iter()
        .map(criterion_to_json)
        .collect();

    let mut output = serde_json::json!({
        "tier": tier,
        "passed": primary_results.passed,
        "scores": scores,
    });

    if let Some(summary) = &primary_results.summary {
        output["summary"] = serde_json::json!(summary);
    }

    if let Some(adversarial) = adversarial_results {
        output["adversarial"] = adversarial.clone();
    }

    output
}

fn criterion_to_json(score: &CriterionScore) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "name": score.name,
        "score": score.score,
        "threshold": score.threshold,
        "passed": score.passed,
    });
    if let Some(findings) = &score.findings {
        obj["findings"] = serde_json::json!(findings);
    }
    obj
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_json_basic() {
        let results = EvaluateResultParams {
            session_id: "eval-1".to_string(),
            scores: vec![CriterionScore {
                name: "Tests pass".to_string(),
                score: 8,
                threshold: 7,
                passed: true,
                findings: Some("All tests pass".to_string()),
            }],
            passed: true,
            summary: Some("All criteria met".to_string()),
        };
        let json = evaluation_json(&results, None, 2);
        assert_eq!(json["tier"], 2);
        assert_eq!(json["passed"], true);
        assert_eq!(json["scores"][0]["name"], "Tests pass");
        assert_eq!(json["summary"], "All criteria met");
        assert!(json.get("adversarial").is_none());
    }

    #[test]
    fn evaluation_json_with_adversarial() {
        let results = EvaluateResultParams {
            session_id: "eval-1".to_string(),
            scores: vec![],
            passed: true,
            summary: None,
        };
        let adversarial = serde_json::json!({
            "designChallenges": [{"severity": "consider", "finding": "test"}],
        });
        let json = evaluation_json(&results, Some(&adversarial), 3);
        assert!(json.get("adversarial").is_some());
    }

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string() {
        let result = truncate("this is a very long string that should be truncated", 20);
        assert!(result.chars().count() <= 20);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_multibyte_utf8() {
        // Should not panic on multibyte characters
        let result = truncate("日本語のテスト文字列です", 8);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 8);
    }
}
