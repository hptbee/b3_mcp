//! Shared language backend contracts and local language detection.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LanguageId(pub String);

impl LanguageId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LanguageName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileExtension(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LanguageBackendId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageBackendKind {
    TreeSitter,
    Lsp,
    StaticConfig,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageSupportLevel {
    Unsupported,
    Basic,
    Good,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageBackendCapability {
    DetectFile,
    Parse,
    ExtractSymbols,
    ExtractImports,
    ExtractRelationships,
    ExtractRoutes,
    ExtractTests,
    FindDefinition,
    FindReferences,
    FindImplementations,
    Diagnostics,
    Rename,
    SymbolicEdit,
    Format,
    SemanticTokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendSelectionPolicy {
    PreferTreeSitter,
    PreferLsp,
    Hybrid,
    TreeSitterOnly,
    LspOnly,
}

impl Default for BackendSelectionPolicy {
    fn default() -> Self {
        Self::PreferTreeSitter
    }
}

impl std::str::FromStr for BackendSelectionPolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "prefer_tree_sitter" => Ok(Self::PreferTreeSitter),
            "prefer_lsp" => Ok(Self::PreferLsp),
            "hybrid" => Ok(Self::Hybrid),
            "tree_sitter_only" => Ok(Self::TreeSitterOnly),
            "lsp_only" => Ok(Self::LspOnly),
            _ => Err(format!(
                "invalid backend selection policy: {value}; supported values: prefer_tree_sitter, prefer_lsp, hybrid, tree_sitter_only, lsp_only"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageFeatureSupport {
    pub capability: LanguageBackendCapability,
    pub supported: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageBackendMetadata {
    pub backend_id: LanguageBackendId,
    pub language_id: LanguageId,
    pub language_name: LanguageName,
    pub kind: LanguageBackendKind,
    pub support_level: LanguageSupportLevel,
    pub capabilities: Vec<LanguageBackendCapability>,
    pub available: bool,
    pub notes: Vec<String>,
}

pub trait LanguageBackend {
    fn metadata(&self) -> LanguageBackendMetadata;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageDetectionResult {
    pub language_id: Option<LanguageId>,
    pub language_name: Option<LanguageName>,
    pub support_level: LanguageSupportLevel,
    pub matched_by: String,
    pub backend_ids: Vec<LanguageBackendId>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageBackendSelection {
    pub language_id: LanguageId,
    pub selected_backend: Option<LanguageBackendId>,
    pub policy: BackendSelectionPolicy,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageBackendError {
    pub message: String,
}

impl LanguageBackendError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageBackendRegistry {
    pub backends: Vec<LanguageBackendMetadata>,
    pub known_languages: Vec<LanguageDetectionResult>,
    pub selection_policy: BackendSelectionPolicy,
    pub lsp_enabled: bool,
    pub experimental_languages_enabled: bool,
}

pub fn default_language_backend_registry() -> LanguageBackendRegistry {
    let backends = vec![
        rust_tree_sitter_backend_metadata(),
        csharp_static_backend_metadata(),
        go_static_backend_metadata(),
        planned_detect_only_backend("typescript", "TypeScript", "planned-lsp-typescript"),
        planned_detect_only_backend("tsx", "TSX", "planned-lsp-typescript-react"),
        planned_detect_only_backend("javascript", "JavaScript", "planned-lsp-javascript"),
        planned_detect_only_backend("jsx", "JSX", "planned-lsp-javascript-react"),
        planned_detect_only_backend("html", "HTML", "planned-static-html"),
        planned_detect_only_backend("css", "CSS", "planned-static-css"),
        planned_detect_only_backend("scss", "SCSS", "planned-static-scss"),
        planned_detect_only_backend("json", "JSON", "planned-static-json"),
        planned_detect_only_backend("yaml", "YAML", "planned-static-yaml"),
        planned_detect_only_backend("sql", "SQL", "planned-static-sql"),
        planned_detect_only_backend("dockerfile", "Dockerfile", "planned-static-dockerfile"),
        planned_detect_only_backend("docker-compose", "Docker Compose", "planned-static-compose"),
        planned_detect_only_backend("ksql", "ksqlDB", "planned-static-ksql"),
        planned_detect_only_backend("xaml", "XAML", "planned-static-xaml"),
        planned_detect_only_backend("python", "Python", "planned-lsp-python"),
        planned_detect_only_backend("java", "Java", "planned-lsp-java"),
        planned_detect_only_backend("php", "PHP", "planned-lsp-php"),
        planned_detect_only_backend("ruby", "Ruby", "planned-lsp-ruby"),
        planned_detect_only_backend("c", "C", "planned-lsp-c"),
        planned_detect_only_backend("cpp", "C++", "planned-lsp-cpp"),
        planned_detect_only_backend("swift", "Swift", "planned-lsp-swift"),
        planned_detect_only_backend("kotlin", "Kotlin", "planned-lsp-kotlin"),
        planned_detect_only_backend("toml", "TOML", "planned-static-toml"),
        planned_detect_only_backend("xml", "XML", "planned-static-xml"),
    ];
    let known_languages = known_language_samples();
    LanguageBackendRegistry {
        backends,
        known_languages,
        selection_policy: BackendSelectionPolicy::PreferTreeSitter,
        lsp_enabled: false,
        experimental_languages_enabled: false,
    }
}

pub fn rust_tree_sitter_backend_metadata() -> LanguageBackendMetadata {
    LanguageBackendMetadata {
        backend_id: LanguageBackendId("tree-sitter-rust".to_string()),
        language_id: LanguageId("rust".to_string()),
        language_name: LanguageName("Rust".to_string()),
        kind: LanguageBackendKind::TreeSitter,
        support_level: LanguageSupportLevel::Good,
        capabilities: vec![
            LanguageBackendCapability::DetectFile,
            LanguageBackendCapability::Parse,
            LanguageBackendCapability::ExtractSymbols,
            LanguageBackendCapability::ExtractImports,
            LanguageBackendCapability::ExtractRelationships,
        ],
        available: true,
        notes: vec!["Existing Rust tree-sitter indexing path.".to_string()],
    }
}

pub fn csharp_static_backend_metadata() -> LanguageBackendMetadata {
    LanguageBackendMetadata {
        backend_id: LanguageBackendId("static-csharp".to_string()),
        language_id: LanguageId("csharp".to_string()),
        language_name: LanguageName("C#".to_string()),
        kind: LanguageBackendKind::StaticConfig,
        support_level: LanguageSupportLevel::Basic,
        capabilities: vec![
            LanguageBackendCapability::DetectFile,
            LanguageBackendCapability::Parse,
            LanguageBackendCapability::ExtractSymbols,
            LanguageBackendCapability::ExtractRoutes,
        ],
        available: true,
        notes: vec![
            "Basic local static C# extraction for ASP.NET Core controllers, route attributes, action methods, and constructor dependency type names.".to_string(),
            "No Roslyn, dotnet CLI, language server, build, restore, runtime execution, or package registry access is required.".to_string(),
            "Full semantic analysis, full DI graph, EF/Dapper analysis, and WPF/XAML intelligence are deferred.".to_string(),
        ],
    }
}

pub fn go_static_backend_metadata() -> LanguageBackendMetadata {
    LanguageBackendMetadata {
        backend_id: LanguageBackendId("static-go".to_string()),
        language_id: LanguageId("go".to_string()),
        language_name: LanguageName("Go".to_string()),
        kind: LanguageBackendKind::StaticConfig,
        support_level: LanguageSupportLevel::Basic,
        capabilities: vec![
            LanguageBackendCapability::DetectFile,
            LanguageBackendCapability::Parse,
            LanguageBackendCapability::ExtractSymbols,
            LanguageBackendCapability::ExtractImports,
            LanguageBackendCapability::ExtractRelationships,
            LanguageBackendCapability::ExtractRoutes,
        ],
        available: true,
        notes: vec![
            "Basic local static Go extraction for packages, imports, functions, methods, structs, interfaces, type declarations, const/var declarations, and conservative route hints.".to_string(),
            "No Go toolchain, go command, compiler/type checker, module download, package registry, runtime execution, or external API is required.".to_string(),
            "Full Go semantic analysis, interface implementation analysis, deep framework intelligence, and gRPC intelligence are deferred.".to_string(),
        ],
    }
}

pub fn detect_language_for_path(path: &Path) -> LanguageDetectionResult {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(result) = detect_by_filename(&file_name) {
        return result;
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    detect_by_extension(&extension).unwrap_or(LanguageDetectionResult {
        language_id: None,
        language_name: None,
        support_level: LanguageSupportLevel::Unsupported,
        matched_by: "unknown".to_string(),
        backend_ids: Vec::new(),
        notes: vec!["No known local backend or detection rule.".to_string()],
    })
}

pub fn select_language_backend(
    language_id: &LanguageId,
    policy: BackendSelectionPolicy,
    registry: &LanguageBackendRegistry,
) -> LanguageBackendSelection {
    let candidates = registry
        .backends
        .iter()
        .filter(|backend| backend.language_id == *language_id && backend.available)
        .collect::<Vec<_>>();
    let selected = match policy {
        BackendSelectionPolicy::PreferTreeSitter
        | BackendSelectionPolicy::Hybrid
        | BackendSelectionPolicy::TreeSitterOnly => candidates
            .iter()
            .find(|backend| backend.kind == LanguageBackendKind::TreeSitter)
            .or_else(|| {
                (policy != BackendSelectionPolicy::TreeSitterOnly)
                    .then(|| candidates.first())
                    .flatten()
            }),
        BackendSelectionPolicy::PreferLsp | BackendSelectionPolicy::LspOnly => candidates
            .iter()
            .find(|backend| backend.kind == LanguageBackendKind::Lsp)
            .or_else(|| {
                (policy != BackendSelectionPolicy::LspOnly)
                    .then(|| candidates.first())
                    .flatten()
            }),
    };
    LanguageBackendSelection {
        language_id: language_id.clone(),
        selected_backend: selected.map(|backend| backend.backend_id.clone()),
        policy,
        reason: selected
            .map(|backend| format!("selected available backend {}", backend.backend_id.0))
            .unwrap_or_else(|| "no available backend for language/policy".to_string()),
    }
}

fn detect_by_filename(file_name: &str) -> Option<LanguageDetectionResult> {
    match file_name {
        "dockerfile" => Some(detect_only(
            "dockerfile",
            "Dockerfile",
            "filename",
            "planned-static-dockerfile",
        )),
        "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml" => {
            Some(detect_only(
                "docker-compose",
                "Docker Compose",
                "filename",
                "planned-static-compose",
            ))
        }
        "go.mod" => Some(LanguageDetectionResult {
            language_id: Some(LanguageId("go".to_string())),
            language_name: Some(LanguageName("Go".to_string())),
            support_level: LanguageSupportLevel::Basic,
            matched_by: "filename".to_string(),
            backend_ids: vec![LanguageBackendId("static-go".to_string())],
            notes: vec![
                "go.mod is parsed statically for module, require, and replace metadata; no go command is executed.".to_string(),
            ],
        }),
        "go.sum" | "go.work" => Some(LanguageDetectionResult {
            language_id: Some(LanguageId("go".to_string())),
            language_name: Some(LanguageName("Go".to_string())),
            support_level: LanguageSupportLevel::Basic,
            matched_by: "filename".to_string(),
            backend_ids: vec![LanguageBackendId("static-go".to_string())],
            notes: vec!["Detected as Go project metadata; registry/module resolution is not performed.".to_string()],
        }),
        _ => None,
    }
}

fn detect_by_extension(extension: &str) -> Option<LanguageDetectionResult> {
    let (id, name, backend) = match extension {
        "rs" => {
            return Some(LanguageDetectionResult {
                language_id: Some(LanguageId("rust".to_string())),
                language_name: Some(LanguageName("Rust".to_string())),
                support_level: LanguageSupportLevel::Good,
                matched_by: "extension".to_string(),
                backend_ids: vec![LanguageBackendId("tree-sitter-rust".to_string())],
                notes: vec!["Rust tree-sitter backend is implemented.".to_string()],
            });
        }
        "cs" => {
            return Some(LanguageDetectionResult {
                language_id: Some(LanguageId("csharp".to_string())),
                language_name: Some(LanguageName("C#".to_string())),
                support_level: LanguageSupportLevel::Basic,
                matched_by: "extension".to_string(),
                backend_ids: vec![LanguageBackendId("static-csharp".to_string())],
                notes: vec![
                    "C# files are parsed with basic local static extraction for ASP.NET Core Web API symbols and routes.".to_string(),
                ],
            });
        }
        "go" => {
            return Some(LanguageDetectionResult {
                language_id: Some(LanguageId("go".to_string())),
                language_name: Some(LanguageName("Go".to_string())),
                support_level: LanguageSupportLevel::Basic,
                matched_by: "extension".to_string(),
                backend_ids: vec![LanguageBackendId("static-go".to_string())],
                notes: vec![
                    "Go files are parsed with basic local static extraction; no Go toolchain or module download is required.".to_string(),
                ],
            });
        }
        "ts" => ("typescript", "TypeScript", "planned-lsp-typescript"),
        "tsx" => ("tsx", "TSX", "planned-lsp-typescript-react"),
        "js" => ("javascript", "JavaScript", "planned-lsp-javascript"),
        "jsx" => ("jsx", "JSX", "planned-lsp-javascript-react"),
        "html" => ("html", "HTML", "planned-static-html"),
        "css" => ("css", "CSS", "planned-static-css"),
        "scss" => ("scss", "SCSS", "planned-static-scss"),
        "json" => ("json", "JSON", "planned-static-json"),
        "yaml" | "yml" => ("yaml", "YAML", "planned-static-yaml"),
        "sql" => ("sql", "SQL", "planned-static-sql"),
        "ksql" => ("ksql", "ksqlDB", "planned-static-ksql"),
        "xaml" => ("xaml", "XAML", "planned-static-xaml"),
        "py" => ("python", "Python", "planned-lsp-python"),
        "java" => ("java", "Java", "planned-lsp-java"),
        "php" => ("php", "PHP", "planned-lsp-php"),
        "rb" => ("ruby", "Ruby", "planned-lsp-ruby"),
        "c" | "h" => ("c", "C", "planned-lsp-c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => ("cpp", "C++", "planned-lsp-cpp"),
        "swift" => ("swift", "Swift", "planned-lsp-swift"),
        "kt" | "kts" => ("kotlin", "Kotlin", "planned-lsp-kotlin"),
        "toml" => ("toml", "TOML", "planned-static-toml"),
        "xml" => ("xml", "XML", "planned-static-xml"),
        _ => return None,
    };
    Some(detect_only(id, name, "extension", backend))
}

fn detect_only(
    id: &str,
    name: &str,
    matched_by: &str,
    backend_id: &str,
) -> LanguageDetectionResult {
    LanguageDetectionResult {
        language_id: Some(LanguageId(id.to_string())),
        language_name: Some(LanguageName(name.to_string())),
        support_level: LanguageSupportLevel::Basic,
        matched_by: matched_by.to_string(),
        backend_ids: vec![LanguageBackendId(backend_id.to_string())],
        notes: vec!["Detected locally; parser/backend implementation is planned.".to_string()],
    }
}

fn planned_detect_only_backend(id: &str, name: &str, backend_id: &str) -> LanguageBackendMetadata {
    LanguageBackendMetadata {
        backend_id: LanguageBackendId(backend_id.to_string()),
        language_id: LanguageId(id.to_string()),
        language_name: LanguageName(name.to_string()),
        kind: if backend_id.contains("lsp") {
            LanguageBackendKind::Lsp
        } else {
            LanguageBackendKind::StaticConfig
        },
        support_level: LanguageSupportLevel::Basic,
        capabilities: vec![LanguageBackendCapability::DetectFile],
        available: false,
        notes: vec![
            "Detection rule exists; full backend implementation is deferred.".to_string(),
            "LSP is disabled until Phase 9.1.".to_string(),
        ],
    }
}

fn known_language_samples() -> Vec<LanguageDetectionResult> {
    [
        "lib.rs",
        "Program.cs",
        "app.ts",
        "component.tsx",
        "index.js",
        "view.jsx",
        "template.html",
        "style.css",
        "style.scss",
        "package.json",
        "config.yaml",
        "query.sql",
        "Dockerfile",
        "docker-compose.yml",
        "stream.ksql",
        "MainWindow.xaml",
        "script.py",
        "Main.java",
        "main.go",
        "go.mod",
        "index.php",
        "app.rb",
        "lib.c",
        "lib.cpp",
        "App.swift",
        "Main.kt",
        "Cargo.toml",
        "layout.xml",
    ]
    .into_iter()
    .map(|sample| detect_language_for_path(Path::new(sample)))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn detects_languages_by_extension() {
        let rust = detect_language_for_path(Path::new("src/lib.rs"));
        let csharp = detect_language_for_path(Path::new("Program.cs"));
        let typescript = detect_language_for_path(Path::new("app.ts"));

        assert_eq!(rust.language_id.unwrap().as_str(), "rust");
        assert_eq!(rust.support_level, LanguageSupportLevel::Good);
        assert_eq!(csharp.language_id.unwrap().as_str(), "csharp");
        assert_eq!(typescript.language_id.unwrap().as_str(), "typescript");
    }

    #[test]
    fn detects_docker_filenames() {
        let dockerfile = detect_language_for_path(Path::new("Dockerfile"));
        let compose = detect_language_for_path(Path::new("docker-compose.yaml"));

        assert_eq!(dockerfile.language_id.unwrap().as_str(), "dockerfile");
        assert_eq!(compose.language_id.unwrap().as_str(), "docker-compose");
    }

    #[test]
    fn reports_rust_backend_metadata_honestly() {
        let metadata = rust_tree_sitter_backend_metadata();

        assert_eq!(metadata.backend_id.0, "tree-sitter-rust");
        assert_eq!(metadata.kind, LanguageBackendKind::TreeSitter);
        assert_eq!(metadata.support_level, LanguageSupportLevel::Good);
        assert!(metadata
            .capabilities
            .contains(&LanguageBackendCapability::Parse));
        assert!(metadata.available);
    }

    #[test]
    fn planned_languages_remain_honest_and_csharp_is_basic_static() {
        let registry = default_language_backend_registry();
        let csharp = registry
            .backends
            .iter()
            .find(|backend| backend.language_id.as_str() == "csharp")
            .expect("csharp");

        assert_eq!(csharp.support_level, LanguageSupportLevel::Basic);
        assert!(csharp.available);
        assert!(csharp
            .capabilities
            .contains(&LanguageBackendCapability::ExtractRoutes));
        assert!(csharp.notes.iter().any(|note| note.contains("No Roslyn")));
        let go = registry
            .backends
            .iter()
            .find(|backend| backend.language_id.as_str() == "go")
            .expect("go");
        assert_eq!(go.support_level, LanguageSupportLevel::Basic);
        assert!(go.available);
        assert!(go
            .capabilities
            .contains(&LanguageBackendCapability::ExtractImports));
        assert!(go.notes.iter().any(|note| note.contains("No Go toolchain")));
    }

    #[test]
    fn unsupported_language_fallback_is_explicit() {
        let detection = detect_language_for_path(Path::new("unknown.weird"));

        assert!(detection.language_id.is_none());
        assert_eq!(detection.support_level, LanguageSupportLevel::Unsupported);
    }

    #[test]
    fn backend_selection_policy_parses() {
        assert_eq!(
            BackendSelectionPolicy::from_str("hybrid"),
            Ok(BackendSelectionPolicy::Hybrid)
        );
        assert!(BackendSelectionPolicy::from_str("cloud").is_err());
    }

    #[test]
    fn lsp_is_disabled_by_default() {
        let registry = default_language_backend_registry();

        assert!(!registry.lsp_enabled);
        assert_eq!(
            registry.selection_policy,
            BackendSelectionPolicy::PreferTreeSitter
        );
    }

    #[test]
    fn no_external_network_dependency_is_required() {
        let registry = default_language_backend_registry();
        let text = format!("{registry:?}");

        assert!(!text.contains("http://"));
        assert!(!text.contains("https://"));
    }
}
