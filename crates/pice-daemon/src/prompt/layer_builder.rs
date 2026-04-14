//! Layer-specific evaluation prompt builder.
//!
//! Wraps `pice_core::layers::filter::build_layer_prompt()` with
//! daemon-specific adversarial evaluation instructions. The core function
//! handles context isolation (layer contract + filtered diff + CLAUDE.md);
//! this module adds the scoring format the evaluation pipeline expects.

use pice_core::layers::filter::build_layer_prompt as core_build_layer_prompt;

/// Build a context-isolated evaluation prompt for a single layer,
/// including daemon-specific evaluation instructions.
///
/// The base prompt (from `pice-core`) ensures the evaluator sees ONLY:
/// - The layer's contract
/// - The git diff filtered to the layer's tagged files
/// - `CLAUDE.md`
///
/// This wrapper appends structured scoring instructions that the
/// `run_stack_loops` orchestrator expects in the evaluation result.
pub fn build_layer_evaluation_prompt(
    layer_name: &str,
    contract_toml: &str,
    filtered_diff: &str,
    claude_md: &str,
) -> String {
    let base = core_build_layer_prompt(layer_name, contract_toml, filtered_diff, claude_md);
    format!(
        "{base}\n\n\
         ## Evaluation Instructions\n\n\
         For EACH criterion in the contract:\n\
         1. Read the code changes for the {layer_name} layer\n\
         2. Try to find failures — you are an adversarial evaluator\n\
         3. Score 1-10 with evidence\n\n\
         Output structured JSON with scores for each criterion."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_layer_name() {
        let prompt = build_layer_evaluation_prompt(
            "backend",
            "[criteria]\nperformance = \"fast\"",
            "+fn handler() {}",
            "# Project Rules",
        );
        assert!(prompt.contains("\"backend\" layer ONLY"));
        assert!(prompt.contains("Evaluation Instructions"));
        assert!(prompt.contains("adversarial evaluator"));
        assert!(prompt.contains("backend layer"));
    }

    #[test]
    fn prompt_includes_contract_and_diff() {
        let prompt = build_layer_evaluation_prompt(
            "api",
            "[criteria]\nerror_handling = \"structured errors\"",
            "+fn new_route() { Ok(json!({})) }",
            "# CLAUDE.md",
        );
        assert!(prompt.contains("structured errors"));
        assert!(prompt.contains("new_route"));
        assert!(prompt.contains("# CLAUDE.md"));
    }

    #[test]
    fn prompt_isolation_no_cross_layer_content() {
        // Build prompt for backend — verify no frontend content leaks in
        let prompt = build_layer_evaluation_prompt(
            "backend",
            "[criteria]\nbackend_ok = true",
            "+fn backend_handler() {}",
            "# Rules",
        );
        assert!(prompt.contains("backend_handler"));
        assert!(!prompt.contains("frontend"));
        assert!(prompt.contains("ONLY the backend layer"));
    }
}
