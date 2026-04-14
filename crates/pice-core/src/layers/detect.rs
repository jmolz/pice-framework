//! Six-level heuristic detection engine for project architectural layers.
//!
//! Pure function module — zero async, zero network. Scans the filesystem at
//! `project_root` to detect layers via manifest files, directory conventions,
//! framework signals, config files, import graph analysis, and user overrides.
//!
//! Detection levels run in order 1–6; level 6 (`.pice/layers.toml` override)
//! short-circuits — if the override file exists, all other detection is skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{LayerDef, LayersConfig, LayersTable};

// ─── Public types ──────────────────────────────────────────────────────────

/// Result of running the six-level heuristic detection engine.
#[derive(Debug, Clone)]
pub struct DetectedLayers {
    pub layers: Vec<DetectedLayer>,
    pub stacks: Option<Vec<DetectedStack>>,
}

/// A single detected layer with provenance metadata.
#[derive(Debug, Clone)]
pub struct DetectedLayer {
    pub name: String,
    pub paths: Vec<String>,
    pub detected_by: Vec<DetectionLevel>,
    pub always_run: bool,
    pub depends_on: Vec<String>,
}

/// Which heuristic level contributed to a layer's detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionLevel {
    Manifest,
    Directory,
    Framework,
    Config,
    ImportGraph,
    Override,
}

/// A detected monorepo stack (workspace package).
#[derive(Debug, Clone)]
pub struct DetectedStack {
    pub name: String,
    pub root: String,
    pub layers: Vec<DetectedLayer>,
    pub detected_by: MonorepoTool,
}

/// Which monorepo tool was used to discover stacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonorepoTool {
    Nx,
    Turborepo,
    PnpmWorkspace,
    DirectoryConvention,
}

/// Known framework presets for layer inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameworkPreset {
    NextJs,
    Remix,
    SvelteKit,
    FastApi,
    Rails,
    Express,
    RustCli,
}

// ─── Internal helpers ──────────────────────────────────────────────────────

/// Signals extracted from manifest files (Level 1).
#[derive(Debug, Default)]
struct ManifestSignals {
    /// Dependency names found in package.json, Cargo.toml, etc.
    deps: BTreeSet<String>,
    /// Which manifest files were found.
    manifests_found: Vec<String>,
}

/// Accumulated layer candidates keyed by canonical layer name.
type LayerMap = BTreeMap<String, LayerCandidate>;

#[derive(Debug, Default)]
struct LayerCandidate {
    paths: BTreeSet<String>,
    detected_by: BTreeSet<DetectionLevelOrd>,
    always_run: bool,
    depends_on: BTreeSet<String>,
}

/// Wrapper for DetectionLevel that supports Ord for BTreeSet storage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)] // ImportGraph used in Phase 2+; Override used via load_override path.
enum DetectionLevelOrd {
    Manifest,
    Directory,
    Framework,
    Config,
    ImportGraph,
    Override,
}

impl From<DetectionLevelOrd> for DetectionLevel {
    fn from(val: DetectionLevelOrd) -> Self {
        match val {
            DetectionLevelOrd::Manifest => DetectionLevel::Manifest,
            DetectionLevelOrd::Directory => DetectionLevel::Directory,
            DetectionLevelOrd::Framework => DetectionLevel::Framework,
            DetectionLevelOrd::Config => DetectionLevel::Config,
            DetectionLevelOrd::ImportGraph => DetectionLevel::ImportGraph,
            DetectionLevelOrd::Override => DetectionLevel::Override,
        }
    }
}

// ─── Directory → layer lookup table ────────────────────────────────────────

/// Static mapping from directory names/paths to layer candidates.
const DIR_LAYER_MAP: &[(&str, &str, &[&str])] = &[
    // (directory, layer_name, glob_patterns)
    ("src/server", "backend", &["src/server/**"]),
    ("lib", "backend", &["lib/**"]),
    ("api", "api", &["api/**"]),
    ("src/server/routes", "api", &["src/server/routes/**"]),
    ("pages/api", "api", &["pages/api/**"]),
    ("src/api", "api", &["src/api/**"]),
    ("pages", "frontend", &["pages/**"]),
    ("app", "frontend", &["app/**"]),
    ("src/client", "frontend", &["src/client/**"]),
    ("src/components", "frontend", &["src/components/**"]),
    ("prisma", "database", &["prisma/**"]),
    ("migrations", "database", &["migrations/**"]),
    ("src/models", "database", &["src/models/**"]),
    ("terraform", "infrastructure", &["terraform/**"]),
    ("infra", "infrastructure", &["infra/**"]),
    ("pulumi", "infrastructure", &["pulumi/**"]),
    ("cdk", "infrastructure", &["cdk/**"]),
    ("deploy", "deployment", &["deploy/**"]),
    ("helm", "deployment", &["helm/**"]),
    (".github/workflows", "deployment", &[".github/workflows/**"]),
    (".circleci", "deployment", &[".circleci/**"]),
    ("monitoring", "observability", &["monitoring/**"]),
    ("otel", "observability", &["otel/**"]),
    ("prometheus", "observability", &["prometheus/**"]),
    ("grafana", "observability", &["grafana/**"]),
    // Rails conventions
    ("app/controllers", "api", &["app/controllers/**"]),
    ("app/models", "database", &["app/models/**"]),
    ("app/views", "frontend", &["app/views/**"]),
    ("db", "database", &["db/**"]),
];

// ─── Config file → layer lookup table ──────────────────────────────────────

const CONFIG_LAYER_MAP: &[(&str, &str, &[&str])] = &[
    ("Dockerfile", "deployment", &["Dockerfile"]),
    ("docker-compose.yml", "deployment", &["docker-compose.yml"]),
    (
        "docker-compose.yaml",
        "deployment",
        &["docker-compose.yaml"],
    ),
    ("vercel.json", "deployment", &["vercel.json"]),
    ("netlify.toml", "deployment", &["netlify.toml"]),
    ("fly.toml", "deployment", &["fly.toml"]),
    (
        ".github/workflows/ci.yml",
        "deployment",
        &[".github/workflows/**"],
    ),
    (
        ".github/workflows/ci.yaml",
        "deployment",
        &[".github/workflows/**"],
    ),
    (
        ".github/workflows/deploy.yml",
        "deployment",
        &[".github/workflows/**"],
    ),
    (
        ".github/workflows/deploy.yaml",
        "deployment",
        &[".github/workflows/**"],
    ),
];

// ─── Main entry point ──────────────────────────────────────────────────────

/// Detect project layers by scanning `project_root` through a six-level
/// heuristic stack. Returns detected layers and optional monorepo stacks.
///
/// If `.pice/layers.toml` exists at `project_root`, detection is skipped
/// entirely — the override file is loaded and converted to `DetectedLayers`
/// with `DetectionLevel::Override` on every layer.
pub fn detect_layers(project_root: &Path) -> Result<DetectedLayers> {
    // Level 6: Override — short-circuits all other detection.
    let override_path = project_root.join(".pice").join("layers.toml");
    if override_path.is_file() {
        return load_override(&override_path);
    }

    let mut layers: LayerMap = BTreeMap::new();

    // Level 1: Manifest files
    let signals = scan_manifests(project_root, &mut layers)?;

    // Level 2: Directory conventions
    scan_directories(project_root, &mut layers);

    // Level 3: Framework presets (combines Level 1 deps + Level 2 dirs)
    apply_framework_presets(project_root, &signals, &mut layers);

    // Level 4: Config files
    scan_config_files(project_root, &mut layers);

    // Level 5: Import graph — stubbed for Phase 1
    #[allow(dead_code)]
    /// Phase 1 stub — real import graph analysis deferred to Phase 2+.
    fn _scan_import_graph(_project_root: &Path, _layers: &mut LayerMap) {
        // No-op: import graph analysis is deferred.
    }

    // Monorepo detection
    let stacks = detect_monorepo(project_root)?;

    // Convert to output types
    let detected = layers
        .into_iter()
        .map(|(name, candidate)| {
            let always_run = candidate.always_run || is_always_run_layer(&name);
            DetectedLayer {
                name,
                paths: candidate.paths.into_iter().collect(),
                detected_by: candidate
                    .detected_by
                    .into_iter()
                    .map(DetectionLevel::from)
                    .collect(),
                always_run,
                depends_on: candidate.depends_on.into_iter().collect(),
            }
        })
        .collect();

    Ok(DetectedLayers {
        layers: detected,
        stacks,
    })
}

// ─── Level 1: Manifest scanning ────────────────────────────────────────────

fn scan_manifests(project_root: &Path, layers: &mut LayerMap) -> Result<ManifestSignals> {
    let mut signals = ManifestSignals::default();

    // package.json
    let pkg_path = project_root.join("package.json");
    if pkg_path.is_file() {
        signals.manifests_found.push("package.json".to_string());
        if let Ok(content) = std::fs::read_to_string(&pkg_path) {
            parse_package_json(&content, &mut signals, layers);
        }
    }

    // Cargo.toml
    let cargo_path = project_root.join("Cargo.toml");
    if cargo_path.is_file() {
        signals.manifests_found.push("Cargo.toml".to_string());
        if let Ok(content) = std::fs::read_to_string(&cargo_path) {
            parse_cargo_toml(&content, &mut signals, layers);
        }
    }

    // pyproject.toml
    let pyproject_path = project_root.join("pyproject.toml");
    if pyproject_path.is_file() {
        signals.manifests_found.push("pyproject.toml".to_string());
        if let Ok(content) = std::fs::read_to_string(&pyproject_path) {
            parse_pyproject_toml(&content, &mut signals, layers);
        }
    }

    // go.mod
    let gomod_path = project_root.join("go.mod");
    if gomod_path.is_file() {
        signals.manifests_found.push("go.mod".to_string());
        // go.mod signals a Go project — add backend layer candidate
        let candidate = layers.entry("backend".to_string()).or_default();
        candidate.paths.insert("**/*.go".to_string());
        candidate.detected_by.insert(DetectionLevelOrd::Manifest);
    }

    // Gemfile
    let gemfile_path = project_root.join("Gemfile");
    if gemfile_path.is_file() {
        signals.manifests_found.push("Gemfile".to_string());
        if let Ok(content) = std::fs::read_to_string(&gemfile_path) {
            parse_gemfile(&content, &mut signals, layers);
        }
    }

    Ok(signals)
}

fn parse_package_json(content: &str, signals: &mut ManifestSignals, layers: &mut LayerMap) {
    #[derive(Deserialize)]
    struct PackageJson {
        #[serde(default)]
        dependencies: BTreeMap<String, serde_json::Value>,
        #[serde(default, rename = "devDependencies")]
        dev_dependencies: BTreeMap<String, serde_json::Value>,
    }

    let pkg: PackageJson = match serde_json::from_str(content) {
        Ok(p) => p,
        Err(_) => return,
    };

    for dep_name in pkg.dependencies.keys().chain(pkg.dev_dependencies.keys()) {
        signals.deps.insert(dep_name.clone());
    }

    // Infer layers from known dependency signals
    let all_deps: BTreeSet<&str> = pkg
        .dependencies
        .keys()
        .chain(pkg.dev_dependencies.keys())
        .map(String::as_str)
        .collect();

    if all_deps.contains("next") {
        let candidate = layers.entry("frontend".to_string()).or_default();
        candidate.detected_by.insert(DetectionLevelOrd::Manifest);
    }
    if all_deps.contains("react") || all_deps.contains("vue") || all_deps.contains("svelte") {
        let candidate = layers.entry("frontend".to_string()).or_default();
        candidate.detected_by.insert(DetectionLevelOrd::Manifest);
    }
    if all_deps.contains("express")
        || all_deps.contains("fastify")
        || all_deps.contains("koa")
        || all_deps.contains("hono")
    {
        let candidate = layers.entry("api".to_string()).or_default();
        candidate.detected_by.insert(DetectionLevelOrd::Manifest);
        let backend_candidate = layers.entry("backend".to_string()).or_default();
        backend_candidate
            .detected_by
            .insert(DetectionLevelOrd::Manifest);
    }
    if all_deps.contains("prisma") || all_deps.contains("@prisma/client") {
        let candidate = layers.entry("database".to_string()).or_default();
        candidate.detected_by.insert(DetectionLevelOrd::Manifest);
    }
    if all_deps.contains("@sveltejs/kit") {
        let candidate = layers.entry("frontend".to_string()).or_default();
        candidate.detected_by.insert(DetectionLevelOrd::Manifest);
    }
}

fn parse_cargo_toml(content: &str, signals: &mut ManifestSignals, layers: &mut LayerMap) {
    // Parse as generic TOML to extract dependency names
    let table: toml::Table = match content.parse() {
        Ok(t) => t,
        Err(_) => return,
    };

    if let Some(deps) = table.get("dependencies").and_then(|v| v.as_table()) {
        for dep_name in deps.keys() {
            signals.deps.insert(dep_name.clone());
        }
    }

    // Cargo.toml generally means a Rust project — backend or CLI
    let candidate = layers.entry("backend".to_string()).or_default();
    candidate.paths.insert("src/**".to_string());
    candidate.detected_by.insert(DetectionLevelOrd::Manifest);
}

fn parse_pyproject_toml(content: &str, signals: &mut ManifestSignals, layers: &mut LayerMap) {
    let table: toml::Table = match content.parse() {
        Ok(t) => t,
        Err(_) => return,
    };

    // Extract deps from [project.dependencies] (PEP 621)
    if let Some(project) = table.get("project").and_then(|v| v.as_table()) {
        if let Some(deps) = project.get("dependencies").and_then(|v| v.as_array()) {
            for dep in deps {
                if let Some(dep_str) = dep.as_str() {
                    // Extract package name before version specifier
                    let name = dep_str
                        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                        .next()
                        .unwrap_or(dep_str)
                        .to_lowercase();
                    signals.deps.insert(name);
                }
            }
        }
    }

    // Also check [tool.poetry.dependencies] for Poetry projects
    if let Some(tool) = table.get("tool").and_then(|v| v.as_table()) {
        if let Some(poetry) = tool.get("poetry").and_then(|v| v.as_table()) {
            if let Some(deps) = poetry.get("dependencies").and_then(|v| v.as_table()) {
                for dep_name in deps.keys() {
                    signals.deps.insert(dep_name.clone());
                }
            }
        }
    }

    // Python project signals
    if signals.deps.contains("fastapi") {
        let api = layers.entry("api".to_string()).or_default();
        api.detected_by.insert(DetectionLevelOrd::Manifest);
        let backend = layers.entry("backend".to_string()).or_default();
        backend.detected_by.insert(DetectionLevelOrd::Manifest);
    }
    if signals.deps.contains("django") {
        let api = layers.entry("api".to_string()).or_default();
        api.detected_by.insert(DetectionLevelOrd::Manifest);
        let backend = layers.entry("backend".to_string()).or_default();
        backend.detected_by.insert(DetectionLevelOrd::Manifest);
    }
}

fn parse_gemfile(content: &str, signals: &mut ManifestSignals, layers: &mut LayerMap) {
    // Scan line by line for gem declarations
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(gem_name) = extract_gem_name(trimmed) {
            signals.deps.insert(gem_name.to_string());
        }
    }

    if signals.deps.contains("rails") {
        let api = layers.entry("api".to_string()).or_default();
        api.detected_by.insert(DetectionLevelOrd::Manifest);
        let backend = layers.entry("backend".to_string()).or_default();
        backend.detected_by.insert(DetectionLevelOrd::Manifest);
        let database = layers.entry("database".to_string()).or_default();
        database.detected_by.insert(DetectionLevelOrd::Manifest);
    }
}

/// Extract gem name from a Gemfile line like `gem 'rails'` or `gem "rails", "~> 7"`.
fn extract_gem_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("gem")?;
    // Must be followed by whitespace or quote
    let rest = rest.strip_prefix(|c: char| c.is_whitespace())?;
    let rest = rest.trim_start();
    // Expect a quoted string
    let (quote, rest) = if let Some(stripped) = rest.strip_prefix('\'') {
        ('\'', stripped)
    } else if let Some(stripped) = rest.strip_prefix('"') {
        ('"', stripped)
    } else {
        return None;
    };
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

// ─── Level 2: Directory scanning ───────────────────────────────────────────

fn scan_directories(project_root: &Path, layers: &mut LayerMap) {
    for &(dir, layer_name, patterns) in DIR_LAYER_MAP {
        if project_root.join(dir).is_dir() {
            let candidate = layers.entry(layer_name.to_string()).or_default();
            for &pattern in patterns {
                candidate.paths.insert(pattern.to_string());
            }
            candidate.detected_by.insert(DetectionLevelOrd::Directory);
        }
    }
}

// ─── Level 3: Framework presets ────────────────────────────────────────────

fn apply_framework_presets(project_root: &Path, signals: &ManifestSignals, layers: &mut LayerMap) {
    let detected_presets = detect_framework_presets(project_root, signals);

    for preset in &detected_presets {
        match preset {
            FrameworkPreset::NextJs => {
                // Next.js → frontend + api
                let frontend = layers.entry("frontend".to_string()).or_default();
                frontend.detected_by.insert(DetectionLevelOrd::Framework);
                if frontend.paths.is_empty() {
                    frontend.paths.insert("pages/**".to_string());
                    frontend.paths.insert("app/**".to_string());
                    frontend.paths.insert("src/client/**".to_string());
                }

                let api = layers.entry("api".to_string()).or_default();
                api.detected_by.insert(DetectionLevelOrd::Framework);
                if api.paths.is_empty() {
                    api.paths.insert("pages/api/**".to_string());
                    api.paths.insert("app/api/**".to_string());
                }
                api.depends_on.insert("backend".to_string());

                // frontend depends_on api
                let frontend = layers.entry("frontend".to_string()).or_default();
                frontend.depends_on.insert("api".to_string());

                // If Prisma detected, api depends_on database
                if signals.deps.contains("prisma") || signals.deps.contains("@prisma/client") {
                    let api = layers.entry("api".to_string()).or_default();
                    api.depends_on.insert("database".to_string());
                }
            }
            FrameworkPreset::Express => {
                let api = layers.entry("api".to_string()).or_default();
                api.detected_by.insert(DetectionLevelOrd::Framework);
                if api.paths.is_empty() {
                    api.paths.insert("src/server/routes/**".to_string());
                    api.paths.insert("routes/**".to_string());
                }
                api.depends_on.insert("backend".to_string());

                let backend = layers.entry("backend".to_string()).or_default();
                backend.detected_by.insert(DetectionLevelOrd::Framework);
                if backend.paths.is_empty() {
                    backend.paths.insert("src/server/**".to_string());
                    backend.paths.insert("src/**".to_string());
                }
            }
            FrameworkPreset::FastApi => {
                let api = layers.entry("api".to_string()).or_default();
                api.detected_by.insert(DetectionLevelOrd::Framework);
                if api.paths.is_empty() {
                    api.paths.insert("app/**".to_string());
                    api.paths.insert("src/**".to_string());
                }
                api.depends_on.insert("backend".to_string());

                let backend = layers.entry("backend".to_string()).or_default();
                backend.detected_by.insert(DetectionLevelOrd::Framework);
                if backend.paths.is_empty() {
                    backend.paths.insert("app/**".to_string());
                    backend.paths.insert("src/**".to_string());
                }
            }
            FrameworkPreset::Rails => {
                let api = layers.entry("api".to_string()).or_default();
                api.detected_by.insert(DetectionLevelOrd::Framework);
                if api.paths.is_empty() {
                    api.paths.insert("app/controllers/**".to_string());
                    api.paths.insert("config/routes.rb".to_string());
                }
                api.depends_on.insert("backend".to_string());
                api.depends_on.insert("database".to_string());

                let backend = layers.entry("backend".to_string()).or_default();
                backend.detected_by.insert(DetectionLevelOrd::Framework);
                if backend.paths.is_empty() {
                    backend.paths.insert("app/**".to_string());
                    backend.paths.insert("lib/**".to_string());
                }

                let database = layers.entry("database".to_string()).or_default();
                database.detected_by.insert(DetectionLevelOrd::Framework);
                if database.paths.is_empty() {
                    database.paths.insert("db/**".to_string());
                    database.paths.insert("app/models/**".to_string());
                }
            }
            FrameworkPreset::SvelteKit => {
                let frontend = layers.entry("frontend".to_string()).or_default();
                frontend.detected_by.insert(DetectionLevelOrd::Framework);
                if frontend.paths.is_empty() {
                    frontend.paths.insert("src/routes/**".to_string());
                    frontend.paths.insert("src/lib/**".to_string());
                }
                frontend.depends_on.insert("api".to_string());

                let api = layers.entry("api".to_string()).or_default();
                api.detected_by.insert(DetectionLevelOrd::Framework);
                if api.paths.is_empty() {
                    api.paths.insert("src/routes/api/**".to_string());
                }
            }
            FrameworkPreset::Remix => {
                let frontend = layers.entry("frontend".to_string()).or_default();
                frontend.detected_by.insert(DetectionLevelOrd::Framework);
                if frontend.paths.is_empty() {
                    frontend.paths.insert("app/routes/**".to_string());
                    frontend.paths.insert("app/components/**".to_string());
                }
                frontend.depends_on.insert("api".to_string());

                let api = layers.entry("api".to_string()).or_default();
                api.detected_by.insert(DetectionLevelOrd::Framework);
                if api.paths.is_empty() {
                    api.paths.insert("app/routes/**".to_string());
                }
            }
            FrameworkPreset::RustCli => {
                let backend = layers.entry("backend".to_string()).or_default();
                backend.detected_by.insert(DetectionLevelOrd::Framework);
                if backend.paths.is_empty() {
                    backend.paths.insert("src/**".to_string());
                }
            }
        }
    }
}

fn detect_framework_presets(
    project_root: &Path,
    signals: &ManifestSignals,
) -> Vec<FrameworkPreset> {
    let mut presets = Vec::new();

    let has_pages = project_root.join("pages").is_dir();
    let has_app = project_root.join("app").is_dir();

    // Next.js: `next` in deps + `app/` or `pages/` directory
    if signals.deps.contains("next") && (has_pages || has_app) {
        presets.push(FrameworkPreset::NextJs);
    }

    // SvelteKit: `@sveltejs/kit` in deps
    if signals.deps.contains("@sveltejs/kit") {
        presets.push(FrameworkPreset::SvelteKit);
    }

    // Remix: `@remix-run/react` in deps
    if signals.deps.contains("@remix-run/react") {
        presets.push(FrameworkPreset::Remix);
    }

    // Express: `express` in deps (only if not Next.js — Next.js may have express)
    if signals.deps.contains("express") && !presets.contains(&FrameworkPreset::NextJs) {
        presets.push(FrameworkPreset::Express);
    }

    // FastAPI: `fastapi` in deps
    if signals.deps.contains("fastapi") {
        presets.push(FrameworkPreset::FastApi);
    }

    // Rails: `rails` in deps
    if signals.deps.contains("rails") {
        presets.push(FrameworkPreset::Rails);
    }

    // Rust CLI: Cargo.toml + clap dep
    if signals.manifests_found.contains(&"Cargo.toml".to_string()) && signals.deps.contains("clap")
    {
        presets.push(FrameworkPreset::RustCli);
    }

    presets
}

// ─── Level 4: Config files ─────────────────────────────────────────────────

fn scan_config_files(project_root: &Path, layers: &mut LayerMap) {
    for &(config_file, layer_name, patterns) in CONFIG_LAYER_MAP {
        if project_root.join(config_file).is_file() {
            let candidate = layers.entry(layer_name.to_string()).or_default();
            for &pattern in patterns {
                candidate.paths.insert(pattern.to_string());
            }
            candidate.detected_by.insert(DetectionLevelOrd::Config);
        }
    }
}

// ─── Level 6: Override ─────────────────────────────────────────────────────

fn load_override(path: &Path) -> Result<DetectedLayers> {
    let config = LayersConfig::load(path)
        .with_context(|| format!("failed to load override layers from {}", path.display()))?;

    let layers = config
        .layers
        .order
        .iter()
        .filter_map(|name| {
            config.layers.defs.get(name).map(|def| DetectedLayer {
                name: name.clone(),
                paths: def.paths.clone(),
                detected_by: vec![DetectionLevel::Override],
                always_run: def.always_run,
                depends_on: def.depends_on.clone(),
            })
        })
        .collect();

    Ok(DetectedLayers {
        layers,
        stacks: None,
    })
}

// ─── always_run inference ──────────────────────────────────────────────────

fn is_always_run_layer(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("infrastructure")
        || lower.contains("deployment")
        || lower.contains("observability")
}

// ─── Monorepo detection ────────────────────────────────────────────────────

fn detect_monorepo(project_root: &Path) -> Result<Option<Vec<DetectedStack>>> {
    // Nx
    let nx_path = project_root.join("nx.json");
    if nx_path.is_file() {
        let stacks = detect_nx_stacks(project_root)?;
        if !stacks.is_empty() {
            return Ok(Some(stacks));
        }
    }

    // Turborepo
    let turbo_path = project_root.join("turbo.json");
    if turbo_path.is_file() {
        let stacks = detect_turbo_stacks(project_root)?;
        if !stacks.is_empty() {
            return Ok(Some(stacks));
        }
    }

    // pnpm-workspace.yaml
    let pnpm_path = project_root.join("pnpm-workspace.yaml");
    if pnpm_path.is_file() {
        let stacks = detect_pnpm_stacks(project_root)?;
        if !stacks.is_empty() {
            return Ok(Some(stacks));
        }
    }

    // Fallback: directory convention — multiple package.json or Cargo.toml in subdirs
    let stacks = detect_directory_convention_stacks(project_root)?;
    if !stacks.is_empty() {
        return Ok(Some(stacks));
    }

    Ok(None)
}

fn detect_nx_stacks(project_root: &Path) -> Result<Vec<DetectedStack>> {
    let mut stacks = Vec::new();

    // Try parsing nx.json for explicit project paths
    let nx_content =
        std::fs::read_to_string(project_root.join("nx.json")).context("failed to read nx.json")?;

    #[derive(Deserialize)]
    struct NxJson {
        #[serde(default)]
        projects: Option<serde_json::Value>,
    }

    let nx: NxJson = serde_json::from_str(&nx_content).unwrap_or(NxJson { projects: None });

    // Collect project roots from nx.json projects field or scan packages/
    let mut project_roots: Vec<(String, String)> = Vec::new();

    if let Some(projects) = nx.projects {
        match projects {
            serde_json::Value::Object(map) => {
                for (name, val) in map {
                    if let Some(root) = val.as_str() {
                        project_roots.push((name, root.to_string()));
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for val in arr {
                    if let Some(path) = val.as_str() {
                        let name = Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path)
                            .to_string();
                        project_roots.push((name, path.to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    // Fallback: scan packages/*/package.json
    if project_roots.is_empty() {
        project_roots = scan_packages_dir(project_root, "packages")?;
    }

    // Sort for deterministic output
    project_roots.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, root) in project_roots {
        let full_path = project_root.join(&root);
        if full_path.is_dir() {
            let sub_result = detect_layers_for_subdir(&full_path)?;
            stacks.push(DetectedStack {
                name,
                root,
                layers: sub_result,
                detected_by: MonorepoTool::Nx,
            });
        }
    }

    Ok(stacks)
}

fn detect_turbo_stacks(project_root: &Path) -> Result<Vec<DetectedStack>> {
    let mut stacks = Vec::new();

    // Turbo uses packages/ directory convention
    let package_roots = scan_packages_dir(project_root, "packages")?;

    for (name, root) in package_roots {
        let full_path = project_root.join(&root);
        if full_path.is_dir() {
            let sub_result = detect_layers_for_subdir(&full_path)?;
            stacks.push(DetectedStack {
                name,
                root,
                layers: sub_result,
                detected_by: MonorepoTool::Turborepo,
            });
        }
    }

    Ok(stacks)
}

fn detect_pnpm_stacks(project_root: &Path) -> Result<Vec<DetectedStack>> {
    let mut stacks = Vec::new();

    let content = std::fs::read_to_string(project_root.join("pnpm-workspace.yaml"))
        .context("failed to read pnpm-workspace.yaml")?;

    // Simple YAML parsing — look for packages: field with glob patterns
    let workspace_globs = parse_pnpm_workspace_packages(&content);

    for glob_pattern in &workspace_globs {
        // Expand glob manually: strip trailing /* and scan that directory
        let base = glob_pattern.trim_end_matches("/*").trim_end_matches("/**");
        let base_path = project_root.join(base);
        if base_path.is_dir() {
            let entries = std::fs::read_dir(&base_path)
                .with_context(|| format!("failed to read directory {}", base_path.display()))?;

            let mut sub_entries: Vec<_> = Vec::new();
            for entry in entries {
                let entry = entry
                    .with_context(|| format!("failed to read entry in {}", base_path.display()))?;
                sub_entries.push(entry);
            }
            // Sort for deterministic output
            sub_entries.sort_by_key(|e| e.file_name());

            for entry in sub_entries {
                let path = entry.path();
                if path.is_dir()
                    && (path.join("package.json").is_file() || path.join("Cargo.toml").is_file())
                {
                    let name = entry.file_name().to_str().unwrap_or("unknown").to_string();
                    let root = format!(
                        "{}/{}",
                        base,
                        entry.file_name().to_str().unwrap_or("unknown")
                    );
                    let sub_result = detect_layers_for_subdir(&path)?;
                    stacks.push(DetectedStack {
                        name,
                        root,
                        layers: sub_result,
                        detected_by: MonorepoTool::PnpmWorkspace,
                    });
                }
            }
        }
    }

    Ok(stacks)
}

/// Parse pnpm-workspace.yaml packages field. Simple line-based parser.
fn parse_pnpm_workspace_packages(content: &str) -> Vec<String> {
    let mut packages = Vec::new();
    let mut in_packages = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }
        if in_packages {
            if trimmed.starts_with('-') {
                let val = trimmed
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !val.is_empty() {
                    packages.push(val.to_string());
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                // Hit a new top-level key, stop
                break;
            }
        }
    }

    packages
}

fn detect_directory_convention_stacks(project_root: &Path) -> Result<Vec<DetectedStack>> {
    let mut stacks = Vec::new();

    // Look for packages/ or apps/ directories with package.json or Cargo.toml
    for dir_name in &["packages", "apps", "services"] {
        let package_roots = scan_packages_dir(project_root, dir_name)?;
        for (name, root) in package_roots {
            let full_path = project_root.join(&root);
            if full_path.is_dir() {
                let sub_result = detect_layers_for_subdir(&full_path)?;
                stacks.push(DetectedStack {
                    name,
                    root,
                    layers: sub_result,
                    detected_by: MonorepoTool::DirectoryConvention,
                });
            }
        }
    }

    Ok(stacks)
}

/// Scan a directory (e.g., `packages/`) for subdirectories containing manifests.
fn scan_packages_dir(project_root: &Path, dir_name: &str) -> Result<Vec<(String, String)>> {
    let mut results = Vec::new();
    let packages_dir = project_root.join(dir_name);

    if !packages_dir.is_dir() {
        return Ok(results);
    }

    let entries = std::fs::read_dir(&packages_dir)
        .with_context(|| format!("failed to read {}", packages_dir.display()))?;

    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", packages_dir.display()))?;
        let path = entry.path();
        if path.is_dir()
            && (path.join("package.json").is_file() || path.join("Cargo.toml").is_file())
        {
            let name = entry.file_name().to_str().unwrap_or("unknown").to_string();
            let root = format!(
                "{}/{}",
                dir_name,
                entry.file_name().to_str().unwrap_or("unknown")
            );
            results.push((name, root));
        }
    }

    // Sort for deterministic output
    results.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(results)
}

/// Run detection on a subdirectory (used for monorepo stacks).
/// Does NOT recurse into monorepo detection to avoid infinite nesting.
fn detect_layers_for_subdir(subdir: &Path) -> Result<Vec<DetectedLayer>> {
    let mut layers: LayerMap = BTreeMap::new();

    let signals = scan_manifests(subdir, &mut layers)?;
    scan_directories(subdir, &mut layers);
    apply_framework_presets(subdir, &signals, &mut layers);
    scan_config_files(subdir, &mut layers);

    let detected = layers
        .into_iter()
        .map(|(name, candidate)| {
            let always_run = candidate.always_run || is_always_run_layer(&name);
            DetectedLayer {
                name,
                paths: candidate.paths.into_iter().collect(),
                detected_by: candidate
                    .detected_by
                    .into_iter()
                    .map(DetectionLevel::from)
                    .collect(),
                always_run,
                depends_on: candidate.depends_on.into_iter().collect(),
            }
        })
        .collect();

    Ok(detected)
}

// ─── Conversion to LayersConfig ────────────────────────────────────────────

impl DetectedLayers {
    /// Convert detection output to a committable `LayersConfig`.
    ///
    /// Builds the `order` from layer names (deterministic via BTreeMap), creates
    /// `LayerDef` entries, and infers contract paths as
    /// `.pice/contracts/{name}.toml`.
    pub fn to_layers_config(&self) -> LayersConfig {
        let mut order = Vec::new();
        let mut defs = BTreeMap::new();

        // Collect all detected layer names for dependency filtering
        let all_names: BTreeSet<&str> = self.layers.iter().map(|l| l.name.as_str()).collect();

        // Canonical order: backend, database, api, frontend, infrastructure,
        // deployment, observability, then any extras alphabetically.
        let canonical_order = [
            "backend",
            "database",
            "api",
            "frontend",
            "infrastructure",
            "deployment",
            "observability",
        ];

        // First pass: add layers in canonical order
        for &name in &canonical_order {
            if let Some(layer) = self.layers.iter().find(|l| l.name == name) {
                order.push(layer.name.clone());
                defs.insert(layer.name.clone(), layer_to_def(layer, &all_names));
            }
        }

        // Second pass: add remaining layers alphabetically
        let canonical_set: BTreeSet<&str> = canonical_order.iter().copied().collect();
        let mut extras: Vec<&DetectedLayer> = self
            .layers
            .iter()
            .filter(|l| !canonical_set.contains(l.name.as_str()))
            .collect();
        extras.sort_by_key(|l| &l.name);
        for layer in extras {
            order.push(layer.name.clone());
            defs.insert(layer.name.clone(), layer_to_def(layer, &all_names));
        }

        // Build stacks section if present
        let stacks = self.stacks.as_ref().map(|detected_stacks| {
            let mut stacks_map = BTreeMap::new();
            for stack in detected_stacks {
                let sub_config = DetectedLayers {
                    layers: stack.layers.clone(),
                    stacks: None,
                };
                stacks_map.insert(
                    stack.name.clone(),
                    super::StackDef {
                        root: stack.root.clone(),
                        layers: Some(sub_config.to_layers_config()),
                    },
                );
            }
            stacks_map
        });

        LayersConfig {
            layers: LayersTable { order, defs },
            seams: None,
            external_contracts: None,
            stacks,
        }
    }
}

fn layer_to_def(layer: &DetectedLayer, all_names: &BTreeSet<&str>) -> LayerDef {
    // Filter depends_on to only reference layers that actually exist in the
    // detection result — avoids validation failures from dangling references.
    let depends_on: Vec<String> = layer
        .depends_on
        .iter()
        .filter(|dep| all_names.contains(dep.as_str()))
        .cloned()
        .collect();

    LayerDef {
        paths: layer.paths.clone(),
        always_run: layer.always_run,
        contract: Some(format!(".pice/contracts/{}.toml", layer.name)),
        depends_on,
        layer_type: if layer.always_run && layer.name == "infrastructure" {
            Some(super::LayerType::Meta)
        } else {
            None
        },
        environment_variants: if layer.name == "deployment" {
            Some(vec!["staging".to_string(), "production".to_string()])
        } else {
            None
        },
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a temp dir and return its path.
    fn setup_fixture() -> tempfile::TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    /// Helper: write a file at a relative path under a root dir, creating
    /// intermediate directories as needed.
    fn write_file(root: &Path, relative_path: &str, content: &str) {
        let full = root.join(relative_path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("failed to create dirs");
        }
        std::fs::write(&full, content).expect("failed to write file");
    }

    /// Helper: create an empty directory at a relative path.
    fn make_dir(root: &Path, relative_path: &str) {
        std::fs::create_dir_all(root.join(relative_path)).expect("failed to create dir");
    }

    /// Helper: find a layer by name in the results.
    fn find_layer<'a>(layers: &'a [DetectedLayer], name: &str) -> Option<&'a DetectedLayer> {
        layers.iter().find(|l| l.name == name)
    }

    // ── Test 1: Next.js fixture ────────────────────────────────────────────

    #[test]
    fn next_js_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        write_file(
            root,
            "package.json",
            r#"{"dependencies": {"next": "14.0.0", "react": "18.0.0"}}"#,
        );
        make_dir(root, "pages");

        let result = detect_layers(root).unwrap();

        let frontend = find_layer(&result.layers, "frontend");
        assert!(
            frontend.is_some(),
            "Next.js should detect frontend layer; found: {:?}",
            result.layers.iter().map(|l| &l.name).collect::<Vec<_>>()
        );

        let api = find_layer(&result.layers, "api");
        assert!(
            api.is_some(),
            "Next.js should detect api layer; found: {:?}",
            result.layers.iter().map(|l| &l.name).collect::<Vec<_>>()
        );

        // Check detection levels include Framework
        let frontend = frontend.unwrap();
        assert!(
            frontend.detected_by.contains(&DetectionLevel::Framework),
            "frontend should be detected by Framework preset"
        );
    }

    // ── Test 2: Prisma fixture ─────────────────────────────────────────────

    #[test]
    fn prisma_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        make_dir(root, "prisma");
        write_file(root, "prisma/schema.prisma", "model User { id Int @id }");

        let result = detect_layers(root).unwrap();

        let database = find_layer(&result.layers, "database");
        assert!(
            database.is_some(),
            "Prisma dir should detect database layer"
        );
    }

    // ── Test 3: Terraform fixture ──────────────────────────────────────────

    #[test]
    fn terraform_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        make_dir(root, "terraform");

        let result = detect_layers(root).unwrap();

        let infra = find_layer(&result.layers, "infrastructure");
        assert!(
            infra.is_some(),
            "terraform/ dir should detect infrastructure layer"
        );
        let infra = infra.unwrap();
        assert!(
            infra.always_run,
            "infrastructure layer should have always_run = true"
        );
    }

    // ── Test 4: Express fixture ────────────────────────────────────────────

    #[test]
    fn express_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        write_file(
            root,
            "package.json",
            r#"{"dependencies": {"express": "4.18.0"}}"#,
        );

        let result = detect_layers(root).unwrap();

        let api = find_layer(&result.layers, "api");
        assert!(api.is_some(), "Express should detect api layer");

        let backend = find_layer(&result.layers, "backend");
        assert!(backend.is_some(), "Express should detect backend layer");

        // api depends_on backend
        let api = api.unwrap();
        assert!(
            api.depends_on.contains(&"backend".to_string()),
            "Express api should depend on backend"
        );
    }

    // ── Test 5: FastAPI fixture ────────────────────────────────────────────

    #[test]
    fn fastapi_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        write_file(
            root,
            "pyproject.toml",
            r#"
[project]
name = "myapp"
dependencies = ["fastapi>=0.100.0", "uvicorn"]
"#,
        );

        let result = detect_layers(root).unwrap();

        let api = find_layer(&result.layers, "api");
        assert!(api.is_some(), "FastAPI should detect api layer");

        let backend = find_layer(&result.layers, "backend");
        assert!(backend.is_some(), "FastAPI should detect backend layer");
    }

    // ── Test 6: Rails fixture ──────────────────────────────────────────────

    #[test]
    fn rails_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        write_file(
            root,
            "Gemfile",
            "source 'https://rubygems.org'\ngem 'rails', '~> 7.0'\ngem 'pg'\n",
        );
        make_dir(root, "app/controllers");

        let result = detect_layers(root).unwrap();

        let api = find_layer(&result.layers, "api");
        assert!(api.is_some(), "Rails should detect api layer");

        let backend = find_layer(&result.layers, "backend");
        assert!(backend.is_some(), "Rails should detect backend layer");

        let database = find_layer(&result.layers, "database");
        assert!(database.is_some(), "Rails should detect database layer");

        // api depends_on backend and database
        let api = api.unwrap();
        assert!(
            api.depends_on.contains(&"backend".to_string()),
            "Rails api should depend on backend"
        );
        assert!(
            api.depends_on.contains(&"database".to_string()),
            "Rails api should depend on database"
        );
    }

    // ── Test 7: SvelteKit fixture ──────────────────────────────────────────

    #[test]
    fn sveltekit_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        write_file(
            root,
            "package.json",
            r#"{"devDependencies": {"@sveltejs/kit": "2.0.0"}}"#,
        );

        let result = detect_layers(root).unwrap();

        let frontend = find_layer(&result.layers, "frontend");
        assert!(frontend.is_some(), "SvelteKit should detect frontend layer");

        let api = find_layer(&result.layers, "api");
        assert!(api.is_some(), "SvelteKit should detect api layer");

        // frontend depends_on api
        let frontend = frontend.unwrap();
        assert!(
            frontend.depends_on.contains(&"api".to_string()),
            "SvelteKit frontend should depend on api"
        );
    }

    // ── Test 8: Monorepo Nx fixture ────────────────────────────────────────

    #[test]
    fn monorepo_nx_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        write_file(root, "nx.json", r#"{"projects": {}}"#);
        write_file(root, "package.json", r#"{"name": "monorepo"}"#);

        // Create two packages
        write_file(
            root,
            "packages/web/package.json",
            r#"{"dependencies": {"next": "14.0.0"}}"#,
        );
        make_dir(root, "packages/web/pages");

        write_file(
            root,
            "packages/api/package.json",
            r#"{"dependencies": {"express": "4.18.0"}}"#,
        );

        let result = detect_layers(root).unwrap();

        assert!(result.stacks.is_some(), "Nx monorepo should detect stacks");
        let stacks = result.stacks.as_ref().unwrap();
        assert_eq!(stacks.len(), 2, "Should detect 2 stacks");

        // All stacks use Nx tool
        for stack in stacks {
            assert_eq!(stack.detected_by, MonorepoTool::Nx);
        }

        // Find the web stack
        let web_stack = stacks.iter().find(|s| s.name == "web");
        assert!(web_stack.is_some(), "Should find 'web' stack");

        // Find the api stack
        let api_stack = stacks.iter().find(|s| s.name == "api");
        assert!(api_stack.is_some(), "Should find 'api' stack");
    }

    // ── Test 9: Override fixture ───────────────────────────────────────────

    #[test]
    fn override_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        // Write a .pice/layers.toml override
        write_file(
            root,
            ".pice/layers.toml",
            r#"
[layers]
order = ["custom-backend", "custom-frontend"]

[layers.custom-backend]
paths = ["server/**"]
always_run = false

[layers.custom-frontend]
paths = ["client/**"]
always_run = false
depends_on = ["custom-backend"]
"#,
        );

        // Also write files that would normally be detected — these should be
        // IGNORED because the override is present.
        write_file(
            root,
            "package.json",
            r#"{"dependencies": {"next": "14.0.0"}}"#,
        );
        make_dir(root, "terraform");

        let result = detect_layers(root).unwrap();

        // Should only have the override layers
        assert_eq!(result.layers.len(), 2);

        let backend = find_layer(&result.layers, "custom-backend");
        assert!(backend.is_some(), "Override should produce custom-backend");
        let backend = backend.unwrap();
        assert_eq!(backend.detected_by, vec![DetectionLevel::Override]);
        assert_eq!(backend.paths, vec!["server/**".to_string()]);

        let frontend = find_layer(&result.layers, "custom-frontend");
        assert!(
            frontend.is_some(),
            "Override should produce custom-frontend"
        );
        let frontend = frontend.unwrap();
        assert_eq!(frontend.detected_by, vec![DetectionLevel::Override]);
        assert_eq!(frontend.depends_on, vec!["custom-backend".to_string()]);

        // No infrastructure should be detected (override skips all detection)
        assert!(
            find_layer(&result.layers, "infrastructure").is_none(),
            "Override should skip all auto-detection"
        );
    }

    // ── Test 10: Config file fixture ───────────────────────────────────────

    #[test]
    fn config_file_fixture() {
        let dir = setup_fixture();
        let root = dir.path();

        write_file(root, "Dockerfile", "FROM node:20\n");
        make_dir(root, ".github/workflows");
        write_file(root, ".github/workflows/ci.yml", "name: CI\n");

        let result = detect_layers(root).unwrap();

        let deployment = find_layer(&result.layers, "deployment");
        assert!(
            deployment.is_some(),
            "Dockerfile + workflows should detect deployment layer"
        );

        let deployment = deployment.unwrap();
        assert!(
            deployment.detected_by.contains(&DetectionLevel::Config),
            "deployment should be detected by Config level"
        );
        assert!(
            deployment.always_run,
            "deployment layer should have always_run = true"
        );
    }

    // ── Test 11: to_layers_config multi-layer tagging ──────────────────────

    #[test]
    fn multi_layer_tagging() {
        let dir = setup_fixture();
        let root = dir.path();

        // Next.js project with api routes
        write_file(
            root,
            "package.json",
            r#"{"dependencies": {"next": "14.0.0"}}"#,
        );
        make_dir(root, "pages/api");

        let result = detect_layers(root).unwrap();
        let config = result.to_layers_config();

        // Validate the config
        config.validate().unwrap();

        // pages/api/users.ts should match both api and frontend globs
        let layers = super::super::tag_file_to_layers(&config, "pages/api/users.ts");
        assert!(
            layers.contains(&"api".to_string()),
            "pages/api/users.ts should match api layer; layers config api paths: {:?}, all tagged: {:?}",
            config.layers.defs.get("api").map(|d| &d.paths),
            layers
        );
        assert!(
            layers.contains(&"frontend".to_string()),
            "pages/api/users.ts should match frontend layer; layers config frontend paths: {:?}, all tagged: {:?}",
            config.layers.defs.get("frontend").map(|d| &d.paths),
            layers
        );
    }

    // ── Additional edge-case tests ─────────────────────────────────────────

    #[test]
    fn empty_project_returns_no_layers() {
        let dir = setup_fixture();
        let result = detect_layers(dir.path()).unwrap();
        assert!(result.layers.is_empty());
        assert!(result.stacks.is_none());
    }

    #[test]
    fn gem_name_extraction() {
        assert_eq!(extract_gem_name("gem 'rails'"), Some("rails"));
        assert_eq!(extract_gem_name("gem \"rails\", \"~> 7\""), Some("rails"));
        assert_eq!(extract_gem_name("gem 'pg'"), Some("pg"));
        assert_eq!(extract_gem_name("# gem 'old'"), None);
        assert_eq!(extract_gem_name(""), None);
        assert_eq!(extract_gem_name("source 'rubygems'"), None);
    }

    #[test]
    fn pnpm_workspace_parsing() {
        let content = r#"
packages:
  - "packages/*"
  - "apps/*"
"#;
        let packages = parse_pnpm_workspace_packages(content);
        assert_eq!(packages, vec!["packages/*", "apps/*"]);
    }

    #[test]
    fn to_layers_config_canonical_order() {
        let detected = DetectedLayers {
            layers: vec![
                DetectedLayer {
                    name: "frontend".to_string(),
                    paths: vec!["app/**".to_string()],
                    detected_by: vec![DetectionLevel::Directory],
                    always_run: false,
                    depends_on: vec!["api".to_string()],
                },
                DetectedLayer {
                    name: "api".to_string(),
                    paths: vec!["api/**".to_string()],
                    detected_by: vec![DetectionLevel::Directory],
                    always_run: false,
                    depends_on: vec![],
                },
                DetectedLayer {
                    name: "infrastructure".to_string(),
                    paths: vec!["terraform/**".to_string()],
                    detected_by: vec![DetectionLevel::Directory],
                    always_run: true,
                    depends_on: vec![],
                },
            ],
            stacks: None,
        };

        let config = detected.to_layers_config();

        // Order follows canonical: api before frontend, infrastructure after
        assert_eq!(
            config.layers.order,
            vec!["api", "frontend", "infrastructure"]
        );

        // Contracts inferred
        let api_def = &config.layers.defs["api"];
        assert_eq!(
            api_def.contract,
            Some(".pice/contracts/api.toml".to_string())
        );

        // infrastructure gets meta type
        let infra_def = &config.layers.defs["infrastructure"];
        assert_eq!(infra_def.layer_type, Some(super::super::LayerType::Meta));
        assert!(infra_def.always_run);
    }

    #[test]
    fn next_js_with_prisma_adds_database_dependency() {
        let dir = setup_fixture();
        let root = dir.path();

        write_file(
            root,
            "package.json",
            r#"{"dependencies": {"next": "14.0.0", "@prisma/client": "5.0.0"}}"#,
        );
        make_dir(root, "pages");
        make_dir(root, "prisma");
        write_file(root, "prisma/schema.prisma", "model User { id Int @id }");

        let result = detect_layers(root).unwrap();

        let api = find_layer(&result.layers, "api").unwrap();
        assert!(
            api.depends_on.contains(&"database".to_string()),
            "Next.js + Prisma: api should depend on database"
        );
    }

    #[test]
    fn multiple_detection_levels_accumulate() {
        let dir = setup_fixture();
        let root = dir.path();

        // Express (Manifest + Framework) + src/server dir (Directory)
        write_file(
            root,
            "package.json",
            r#"{"dependencies": {"express": "4.18.0"}}"#,
        );
        make_dir(root, "src/server");

        let result = detect_layers(root).unwrap();

        let backend = find_layer(&result.layers, "backend").unwrap();
        assert!(
            backend.detected_by.contains(&DetectionLevel::Manifest),
            "backend should be detected by Manifest"
        );
        assert!(
            backend.detected_by.contains(&DetectionLevel::Directory),
            "backend should be detected by Directory"
        );
        assert!(
            backend.detected_by.contains(&DetectionLevel::Framework),
            "backend should be detected by Framework"
        );
    }
}
