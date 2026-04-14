//! Workflow validation — schema, triggers, cross-references, models.
//!
//! Validation is split into focused functions so the caller can reuse parts
//! (e.g., `validate_schema_only` is called during loader parse; the full suite
//! is called by the `pice validate` CLI). All functions collect every error
//! they find before returning — the CLI prints them all at once.

use serde::Serialize;

use crate::layers::LayersConfig;
use crate::workflow::schema::WorkflowConfig;
use crate::workflow::trigger;
use crate::workflow::SCHEMA_VERSION;

/// A single validation error with a field path and a human-readable message.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

/// Same shape as an error, but non-fatal — printed but exit code stays 0.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
}

/// Aggregate report — serializable for `pice validate --json`.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn extend(&mut self, other: ValidationReport) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

// ─── Schema-only checks ─────────────────────────────────────────────────────

/// Check `schema_version`, required-field presence, and value ranges.
pub fn validate_schema_only(cfg: &WorkflowConfig) -> ValidationReport {
    let mut report = ValidationReport::default();

    if cfg.schema_version != SCHEMA_VERSION {
        report.errors.push(ValidationError {
            field: "schema_version".into(),
            message: format!(
                "schema_version \"{}\" is not supported; expected \"{}\"",
                cfg.schema_version, SCHEMA_VERSION
            ),
            line: None,
            column: None,
        });
    }

    if !(0.0..=1.0).contains(&cfg.defaults.min_confidence) {
        report.errors.push(ValidationError {
            field: "defaults.min_confidence".into(),
            message: format!(
                "min_confidence must be between 0 and 1; got {}",
                cfg.defaults.min_confidence
            ),
            line: None,
            column: None,
        });
    }

    if !(1..=3).contains(&cfg.defaults.tier) {
        report.errors.push(ValidationError {
            field: "defaults.tier".into(),
            message: format!("tier must be 1, 2, or 3; got {}", cfg.defaults.tier),
            line: None,
            column: None,
        });
    }

    if cfg.defaults.budget_usd < 0.0 {
        report.errors.push(ValidationError {
            field: "defaults.budget_usd".into(),
            message: format!(
                "budget_usd must be non-negative; got {}",
                cfg.defaults.budget_usd
            ),
            line: None,
            column: None,
        });
    }

    if cfg.defaults.max_passes == 0 {
        report.errors.push(ValidationError {
            field: "defaults.max_passes".into(),
            message: "max_passes must be ≥ 1".into(),
            line: None,
            column: None,
        });
    }

    // Per-layer override sanity: tier range, confidence range.
    for (layer, o) in &cfg.layer_overrides {
        if let Some(t) = o.tier {
            if !(1..=3).contains(&t) {
                report.errors.push(ValidationError {
                    field: format!("layer_overrides.{layer}.tier"),
                    message: format!("tier must be 1, 2, or 3; got {t}"),
                    line: None,
                    column: None,
                });
            }
        }
        if let Some(mc) = o.min_confidence {
            if !(0.0..=1.0).contains(&mc) {
                report.errors.push(ValidationError {
                    field: format!("layer_overrides.{layer}.min_confidence"),
                    message: format!("min_confidence must be between 0 and 1; got {mc}"),
                    line: None,
                    column: None,
                });
            }
        }
        if let Some(b) = o.budget_usd {
            if b < 0.0 {
                report.errors.push(ValidationError {
                    field: format!("layer_overrides.{layer}.budget_usd"),
                    message: format!("budget_usd must be non-negative; got {b}"),
                    line: None,
                    column: None,
                });
            }
        }
    }

    report
}

// ─── Trigger checks ─────────────────────────────────────────────────────────

/// Parse every trigger expression in the config; collect syntax errors.
pub fn validate_triggers(cfg: &WorkflowConfig) -> ValidationReport {
    let mut report = ValidationReport::default();

    if let Some(review) = cfg.review.as_ref() {
        if let Some(expr) = review.trigger.as_deref() {
            if let Err(e) = trigger::parse(expr) {
                report.errors.push(ValidationError {
                    field: "review.trigger".into(),
                    message: e.message.clone(),
                    line: Some(e.line),
                    column: Some(e.column),
                });
            }
        }
    }

    if let Some(review) = cfg.phases.review.as_ref() {
        if let Some(expr) = review.trigger.as_deref() {
            if let Err(e) = trigger::parse(expr) {
                report.errors.push(ValidationError {
                    field: "phases.review.trigger".into(),
                    message: e.message.clone(),
                    line: Some(e.line),
                    column: Some(e.column),
                });
            }
        }
    }

    for (layer, o) in &cfg.layer_overrides {
        if let Some(expr) = o.trigger.as_deref() {
            if let Err(e) = trigger::parse(expr) {
                report.errors.push(ValidationError {
                    field: format!("layer_overrides.{layer}.trigger"),
                    message: e.message.clone(),
                    line: Some(e.line),
                    column: Some(e.column),
                });
            }
        }
    }

    report
}

// ─── Cross-reference checks ─────────────────────────────────────────────────

/// Validate that every `layer_overrides` key exists in `layers.toml`, and
/// that every `seams` boundary references known layers.
pub fn validate_cross_references(cfg: &WorkflowConfig, layers: &LayersConfig) -> ValidationReport {
    let mut report = ValidationReport::default();

    let known_layers: Vec<&str> = layers.layers.order.iter().map(|s| s.as_str()).collect();
    let known_layers_set: std::collections::HashSet<&str> = known_layers.iter().copied().collect();

    for layer in cfg.layer_overrides.keys() {
        if !known_layers_set.contains(layer.as_str()) {
            report.errors.push(ValidationError {
                field: format!("layer_overrides.{layer}"),
                message: format!(
                    "unknown layer '{layer}'; known layers in .pice/layers.toml: {}",
                    known_layers.join(", ")
                ),
                line: None,
                column: None,
            });
        }
    }

    for (layer, model) in &cfg.phases.evaluate.model_override {
        if !known_layers_set.contains(layer.as_str()) {
            report.errors.push(ValidationError {
                field: format!("phases.evaluate.model_override.{layer}"),
                message: format!(
                    "unknown layer '{layer}' in model_override (set to '{model}'); known layers: {}",
                    known_layers.join(", ")
                ),
                line: None,
                column: None,
            });
        }
    }

    if let Some(seams) = cfg.seams.as_ref() {
        for boundary in seams.keys() {
            if !seam_boundary_references_known_layers(boundary, &known_layers_set) {
                report.errors.push(ValidationError {
                    field: format!("seams.{boundary}"),
                    message: format!(
                        "seam boundary '{boundary}' does not reference known layers; \
                         use 'A-B' or 'A->B' where both A and B appear in layers.order ({})",
                        known_layers.join(", ")
                    ),
                    line: None,
                    column: None,
                });
            }
        }
    }

    report
}

fn seam_boundary_references_known_layers(
    boundary: &str,
    known: &std::collections::HashSet<&str>,
) -> bool {
    let parts: Vec<&str> = boundary
        .split(['-', '>', '<', '↔', '→'])
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return false;
    }
    parts.iter().all(|p| known.contains(p))
}

// ─── Model checks ───────────────────────────────────────────────────────────

/// Validate model names against a provider's capability list. If `known_models`
/// is `None`, emit warnings instead of errors (the validator didn't get a
/// chance to query the provider).
pub fn validate_models(cfg: &WorkflowConfig, known_models: Option<&[String]>) -> ValidationReport {
    let mut report = ValidationReport::default();

    let check = |field: String, model: &str, report: &mut ValidationReport| match known_models {
        Some(list) if !list.iter().any(|m| m == model) => {
            report.errors.push(ValidationError {
                field,
                message: format!(
                    "unknown model '{model}'; provider reports: {}",
                    list.join(", ")
                ),
                line: None,
                column: None,
            });
        }
        None => {
            report.warnings.push(ValidationWarning {
                field,
                message: format!(
                    "cannot verify model '{model}' — provider capability list unavailable"
                ),
            });
        }
        _ => {}
    };

    check("defaults.model".into(), &cfg.defaults.model, &mut report);
    for (layer, model) in &cfg.phases.evaluate.model_override {
        check(
            format!("phases.evaluate.model_override.{layer}"),
            model,
            &mut report,
        );
    }

    report
}

// ─── Umbrella ───────────────────────────────────────────────────────────────

/// Run every validation pass and aggregate the results.
pub fn validate_all(
    cfg: &WorkflowConfig,
    layers: Option<&LayersConfig>,
    known_models: Option<&[String]>,
) -> ValidationReport {
    let mut report = ValidationReport::default();
    report.extend(validate_schema_only(cfg));
    report.extend(validate_triggers(cfg));
    if let Some(layers) = layers {
        report.extend(validate_cross_references(cfg, layers));
    } else {
        report.warnings.push(ValidationWarning {
            field: "layers.toml".into(),
            message: ".pice/layers.toml not found; skipping cross-reference validation".into(),
        });
    }
    report.extend(validate_models(cfg, known_models));
    report
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::{LayerDef, LayersConfig, LayersTable};
    use crate::workflow::loader::embedded_defaults;
    use crate::workflow::schema::{LayerOverride, ReviewConfig};
    use std::collections::BTreeMap;

    fn sample_layers() -> LayersConfig {
        let mut defs = BTreeMap::new();
        defs.insert(
            "backend".into(),
            LayerDef {
                paths: vec!["src/**".into()],
                always_run: false,
                contract: None,
                depends_on: vec![],
                layer_type: None,
                environment_variants: None,
            },
        );
        defs.insert(
            "frontend".into(),
            LayerDef {
                paths: vec!["web/**".into()],
                always_run: false,
                contract: None,
                depends_on: vec![],
                layer_type: None,
                environment_variants: None,
            },
        );
        defs.insert(
            "infrastructure".into(),
            LayerDef {
                paths: vec!["infra/**".into()],
                always_run: true,
                contract: None,
                depends_on: vec![],
                layer_type: None,
                environment_variants: None,
            },
        );
        defs.insert(
            "deployment".into(),
            LayerDef {
                paths: vec!["deploy/**".into()],
                always_run: true,
                contract: None,
                depends_on: vec![],
                layer_type: None,
                environment_variants: None,
            },
        );
        LayersConfig {
            layers: LayersTable {
                order: vec![
                    "backend".into(),
                    "frontend".into(),
                    "infrastructure".into(),
                    "deployment".into(),
                ],
                defs,
            },
            seams: None,
            external_contracts: None,
            stacks: None,
        }
    }

    #[test]
    fn valid_workflow_passes() {
        let cfg = embedded_defaults();
        let layers = sample_layers();
        let report = validate_all(&cfg, Some(&layers), Some(&["sonnet".into()]));
        assert!(report.is_ok(), "unexpected errors: {:?}", report.errors);
    }

    #[test]
    fn bad_schema_version_errors() {
        let mut cfg = embedded_defaults();
        cfg.schema_version = "0.1".into();
        let report = validate_schema_only(&cfg);
        assert!(!report.is_ok());
        assert!(report.errors.iter().any(|e| e.field == "schema_version"));
    }

    #[test]
    fn trigger_syntax_error_has_line_col() {
        let mut cfg = embedded_defaults();
        cfg.review = Some(ReviewConfig {
            enabled: true,
            trigger: Some("tier ==".into()),
            ..Default::default()
        });
        let report = validate_triggers(&cfg);
        assert!(!report.is_ok());
        let err = &report.errors[0];
        assert_eq!(err.field, "review.trigger");
        assert!(err.line.is_some());
        assert!(err.column.is_some());
    }

    #[test]
    fn unknown_layer_in_overrides_errors() {
        let mut cfg = embedded_defaults();
        cfg.layer_overrides.insert(
            "ghost_layer".into(),
            LayerOverride {
                tier: Some(3),
                ..Default::default()
            },
        );
        let layers = sample_layers();
        let report = validate_cross_references(&cfg, &layers);
        assert!(!report.is_ok());
        let err = &report.errors[0];
        assert!(err.message.contains("ghost_layer"));
        assert!(err.message.contains("backend"));
    }

    #[test]
    fn unknown_seam_boundary_errors() {
        let mut cfg = embedded_defaults();
        let mut seams = BTreeMap::new();
        seams.insert("backend-nonexistent".into(), vec!["check1".into()]);
        cfg.seams = Some(seams);
        let layers = sample_layers();
        let report = validate_cross_references(&cfg, &layers);
        assert!(!report.is_ok());
        assert!(report.errors.iter().any(|e| e.field.starts_with("seams.")));
    }

    #[test]
    fn known_seam_boundary_passes() {
        let mut cfg = embedded_defaults();
        let mut seams = BTreeMap::new();
        seams.insert("backend-frontend".into(), vec!["check1".into()]);
        cfg.seams = Some(seams);
        let layers = sample_layers();
        let report = validate_cross_references(&cfg, &layers);
        assert!(report.is_ok(), "{:?}", report.errors);
    }

    #[test]
    fn unknown_model_with_known_list_errors() {
        let mut cfg = embedded_defaults();
        cfg.defaults.model = "ghost".into();
        let report = validate_models(&cfg, Some(&["sonnet".into(), "opus".into()]));
        assert!(!report.is_ok());
        assert!(report.errors[0].message.contains("ghost"));
    }

    #[test]
    fn unknown_model_without_list_warns() {
        let mut cfg = embedded_defaults();
        cfg.defaults.model = "ghost".into();
        let report = validate_models(&cfg, None);
        assert!(report.is_ok());
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn multiple_errors_collected() {
        let mut cfg = embedded_defaults();
        cfg.schema_version = "0.1".into();
        cfg.defaults.min_confidence = 2.0;
        cfg.defaults.tier = 9;
        let report = validate_schema_only(&cfg);
        assert!(report.errors.len() >= 3);
    }

    #[test]
    fn all_presets_valid() {
        use crate::workflow::loader;

        let fixture_layers = sample_layers();
        let preset_names = ["greenfield", "brownfield", "ci", "strict", "permissive"];
        // Resolve preset directory relative to the crate root at compile time.
        let crate_dir = env!("CARGO_MANIFEST_DIR");
        let preset_dir =
            std::path::Path::new(crate_dir).join("../../templates/pice/workflow-presets");

        for name in preset_names {
            let path = preset_dir.join(format!("{name}.yaml"));
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("preset {name} not found at {}", path.display()));
            let cfg: WorkflowConfig = serde_yaml::from_str(&content)
                .unwrap_or_else(|e| panic!("preset {name} failed to parse: {e}"));

            // Framework → project is a simple overlay (no floor).
            let framework = loader::embedded_defaults();
            let merged = crate::workflow::merge::overlay(framework, cfg.clone());

            let report = validate_all(
                &merged,
                Some(&fixture_layers),
                Some(&["sonnet".into(), "opus".into(), "haiku".into()]),
            );
            assert!(
                report.is_ok(),
                "preset {name} failed validation: {:?}",
                report.errors
            );
        }
    }
}
