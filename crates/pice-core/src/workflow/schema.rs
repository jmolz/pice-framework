//! `WorkflowConfig` struct hierarchy — mirrors PRDv2 lines 845–901.
//!
//! All maps use `BTreeMap` for deterministic serialization order (YAML
//! roundtrips and error messages benefit). All enums use `snake_case` on the
//! wire to match the PRDv2 YAML examples.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level `.pice/workflow.yaml` configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowConfig {
    pub schema_version: String,
    pub defaults: Defaults,
    #[serde(default)]
    pub phases: Phases,
    #[serde(default)]
    pub layer_overrides: BTreeMap<String, LayerOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seams: Option<BTreeMap<String, Vec<String>>>,
}

/// Pipeline-wide defaults applied when no layer override specifies otherwise.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Defaults {
    pub tier: u8,
    pub min_confidence: f64,
    pub max_passes: u32,
    pub model: String,
    pub budget_usd: f64,
    #[serde(default)]
    pub cost_cap_behavior: CostCapBehavior,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_parallelism: Option<u32>,
}

/// What to do when a per-layer evaluation would exceed `budget_usd`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostCapBehavior {
    #[default]
    Halt,
    Warn,
    Continue,
}

/// Phase configuration — plan / execute / evaluate.
///
/// Review gates live at the top level (`WorkflowConfig.review`), NOT inside
/// `phases`. Prior to Phase 2 GA this struct carried a duplicate
/// `phases.review` field; it was dead at runtime and created a latent
/// floor-bypass surface (floor merge only covered the top-level field).
/// Removed to keep one canonical location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Phases {
    #[serde(default)]
    pub plan: PhaseConfig,
    #[serde(default)]
    pub execute: ExecutePhase,
    #[serde(default)]
    pub evaluate: EvaluatePhase,
}

/// Generic phase descriptor — only used for `plan` today.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PhaseConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// `phases.execute` — implementation phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutePhase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub parallel: bool,
    #[serde(default)]
    pub worktree_isolation: bool,
    #[serde(default)]
    pub retry: RetryConfig,
}

impl Default for ExecutePhase {
    fn default() -> Self {
        Self {
            description: None,
            parallel: true,
            worktree_isolation: true,
            retry: RetryConfig::default(),
        }
    }
}

/// Retry policy for the execute phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryConfig {
    pub max_attempts: u32,
    #[serde(default)]
    pub fresh_context: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            fresh_context: true,
        }
    }
}

/// `phases.evaluate` — dual-model scoring phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluatePhase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub parallel: bool,
    #[serde(default)]
    pub seam_checks: bool,
    #[serde(default)]
    pub adaptive_algorithm: AdaptiveAlgo,
    #[serde(default)]
    pub model_override: BTreeMap<String, String>,
}

impl Default for EvaluatePhase {
    fn default() -> Self {
        Self {
            description: None,
            parallel: true,
            seam_checks: true,
            adaptive_algorithm: AdaptiveAlgo::default(),
            model_override: BTreeMap::new(),
        }
    }
}

/// Which adaptive algorithm governs pass-count selection during evaluation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveAlgo {
    #[default]
    BayesianSprt,
    Adts,
    Vec,
    None,
}

/// Per-layer override. All fields are optional — absent fields inherit from defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LayerOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_passes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_review: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
}

/// Review gate configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    #[serde(default = "default_timeout_hours")]
    pub timeout_hours: u32,
    #[serde(default)]
    pub on_timeout: OnTimeout,
    #[serde(default = "default_notification")]
    pub notification: String,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            trigger: None,
            timeout_hours: default_timeout_hours(),
            on_timeout: OnTimeout::default(),
            notification: default_notification(),
        }
    }
}

fn default_timeout_hours() -> u32 {
    24
}

fn default_notification() -> String {
    "stdout".to_string()
}

/// What to do when a review gate times out.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnTimeout {
    #[default]
    Reject,
    Approve,
    Skip,
}
