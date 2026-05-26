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
    DetectOnly,
    Basic,
    Good,
    Advanced,
    Experimental,
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
        web_tree_sitter_backend_metadata(
            "typescript",
            "TypeScript",
            "tree-sitter-typescript",
            "Basic local TypeScript symbols, imports, relationships, Node REST routes, React/Next/Angular metadata, and Three.js/WebGL hints are indexed without node/npm/tsc execution.",
        ),
        web_tree_sitter_backend_metadata(
            "tsx",
            "TSX",
            "tree-sitter-typescript",
            "Basic local TSX component-like symbols, imports, React/Next metadata, and Three.js/WebGL hints are indexed without browser, node/npm, or compiler execution.",
        ),
        web_tree_sitter_backend_metadata(
            "javascript",
            "JavaScript",
            "tree-sitter-javascript",
            "Basic local JavaScript symbols, imports, relationships, Node REST routes, Next.js metadata, and Three.js/WebGL hints are indexed without node/npm execution.",
        ),
        web_tree_sitter_backend_metadata(
            "jsx",
            "JSX",
            "tree-sitter-javascript",
            "Basic local JSX component-like symbols, imports, React metadata, and static route/template hints are indexed without browser or runtime execution.",
        ),
        csharp_static_backend_metadata(),
        go_static_backend_metadata(),
        backend_static_metadata("python", "Python", "static-python"),
        backend_static_metadata("java", "Java", "static-java"),
        backend_static_metadata("kotlin", "Kotlin", "static-kotlin"),
        backend_static_metadata("php", "PHP", "static-php"),
        backend_static_metadata("ruby", "Ruby", "static-ruby"),
        backend_static_metadata("c", "C", "static-c"),
        backend_static_metadata("cpp", "C++", "static-cpp"),
        backend_static_metadata("swift", "Swift", "static-swift"),
        backend_static_metadata("objective_c", "Objective-C", "static-objective-c"),
        backend_static_metadata("dart", "Dart", "static-dart"),
        backend_static_metadata("yaml", "YAML", "static-yaml"),
        backend_static_metadata("json", "JSON", "static-json"),
        backend_static_metadata("toml", "TOML", "static-toml"),
        backend_static_metadata("xml", "XML", "static-xml"),
        backend_static_metadata("html", "HTML", "static-html"),
        backend_static_metadata("css", "CSS", "static-css"),
        backend_static_metadata("scss", "SCSS", "static-scss"),
        backend_static_metadata("ksql", "ksqlDB", "static-ksql"),
        backend_static_metadata("sql", "SQL", "static-sql"),
        backend_static_metadata("env", "Env", "static-env"),
        backend_static_metadata_with_capabilities(
            "dockerfile",
            "Dockerfile",
            "static-dockerfile",
            vec![
                LanguageBackendCapability::DetectFile,
                LanguageBackendCapability::Parse,
                LanguageBackendCapability::ExtractRelationships,
            ],
            "Basic local static Dockerfile extraction records infrastructure image, port, environment-key, command, and entrypoint hints without Docker execution.",
        ),
        backend_static_metadata_with_capabilities(
            "docker-compose",
            "Docker Compose",
            "static-compose",
            vec![
                LanguageBackendCapability::DetectFile,
                LanguageBackendCapability::Parse,
                LanguageBackendCapability::ExtractRelationships,
            ],
            "Basic local static Docker Compose extraction records service, image, port, environment-key, and dependency hints without Docker Compose execution.",
        ),
        backend_static_metadata_with_capabilities(
            "xaml",
            "XAML",
            "static-xaml",
            vec![
                LanguageBackendCapability::DetectFile,
                LanguageBackendCapability::Parse,
                LanguageBackendCapability::ExtractSymbols,
                LanguageBackendCapability::ExtractRelationships,
            ],
            "Basic local static XAML extraction records WPF roots, resources, bindings, commands, namespace hints, and code-behind links without Visual Studio, MSBuild, dotnet, or XAML compiler execution.",
        ),
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

pub fn web_tree_sitter_backend_metadata(
    language_id: &str,
    language_name: &str,
    backend_id: &str,
    note: &str,
) -> LanguageBackendMetadata {
    LanguageBackendMetadata {
        backend_id: LanguageBackendId(backend_id.to_string()),
        language_id: LanguageId(language_id.to_string()),
        language_name: LanguageName(language_name.to_string()),
        kind: LanguageBackendKind::TreeSitter,
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
            note.to_string(),
            "No package manager, compiler, runtime execution, browser, WebGL runtime, language server, external API, or internet access is required.".to_string(),
            "Full framework semantics, browser/runtime behavior, and IDE-grade type analysis are deferred.".to_string(),
        ],
    }
}

pub fn backend_static_metadata(
    language_id: &str,
    language_name: &str,
    backend_id: &str,
) -> LanguageBackendMetadata {
    backend_static_metadata_with_capabilities(
        language_id,
        language_name,
        backend_id,
        vec![
            LanguageBackendCapability::DetectFile,
            LanguageBackendCapability::Parse,
            LanguageBackendCapability::ExtractSymbols,
            LanguageBackendCapability::ExtractImports,
            LanguageBackendCapability::ExtractRelationships,
            LanguageBackendCapability::ExtractRoutes,
        ],
        &format!("Basic local static {language_name} backend/application extraction is available."),
    )
}

pub fn backend_static_metadata_with_capabilities(
    language_id: &str,
    language_name: &str,
    backend_id: &str,
    capabilities: Vec<LanguageBackendCapability>,
    primary_note: &str,
) -> LanguageBackendMetadata {
    LanguageBackendMetadata {
        backend_id: LanguageBackendId(backend_id.to_string()),
        language_id: LanguageId(language_id.to_string()),
        language_name: LanguageName(language_name.to_string()),
        kind: LanguageBackendKind::StaticConfig,
        support_level: LanguageSupportLevel::Basic,
        capabilities,
        available: true,
        notes: vec![
            primary_note.to_string(),
            "No package manager, compiler, runtime execution, language server, external API, or internet access is required.".to_string(),
            "Compiler-grade semantics and deep framework analysis are deferred.".to_string(),
        ],
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
    if file_name.starts_with(".env.") {
        return Some(static_basic_detection(
            "env",
            "Env",
            "filename",
            "static-env",
            "Env-like files are parsed locally for key names; non-example env values are redacted.",
        ));
    }
    match file_name {
        "dockerfile" => Some(static_basic_detection(
            "dockerfile",
            "Dockerfile",
            "filename",
            "static-dockerfile",
            "Dockerfile files are parsed statically for infrastructure hints; Docker is not executed.",
        )),
        "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml" => {
            Some(static_basic_detection(
                "docker-compose",
                "Docker Compose",
                "filename",
                "static-compose",
                "Docker Compose files are parsed statically for service/image/port/env/dependency hints; Docker Compose is not executed.",
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
        "pyproject.toml" | "requirements.txt" | "setup.py" | "setup.cfg" | "pipfile"
        | "poetry.lock" | "uv.lock" => Some(static_basic_detection(
            "python",
            "Python",
            "filename",
            "static-python",
            "Detected Python project metadata; package installation is not performed.",
        )),
        "pom.xml" | "build.gradle" | "settings.gradle" => Some(static_basic_detection(
            "java",
            "Java",
            "filename",
            "static-java",
            "Detected Java project metadata; Maven/Gradle execution is not performed.",
        )),
        "build.gradle.kts" | "settings.gradle.kts" => Some(static_basic_detection(
            "kotlin",
            "Kotlin",
            "filename",
            "static-kotlin",
            "Detected Kotlin project metadata; Gradle/JVM execution is not performed.",
        )),
        "composer.json" | "composer.lock" => Some(static_basic_detection(
            "php",
            "PHP",
            "filename",
            "static-php",
            "Detected PHP project metadata; composer execution is not performed.",
        )),
        "gemfile" | "gemfile.lock" => Some(static_basic_detection(
            "ruby",
            "Ruby",
            "filename",
            "static-ruby",
            "Detected Ruby project metadata; bundle execution is not performed.",
        )),
        "package.swift" => Some(static_basic_detection(
            "swift",
            "Swift",
            "filename",
            "static-swift",
            "Detected Swift package metadata; swift and xcodebuild execution are not performed.",
        )),
        "pubspec.yaml" | "analysis_options.yaml" => Some(static_basic_detection(
            "dart",
            "Dart",
            "filename",
            "static-dart",
            "Detected Dart/Flutter project metadata; dart and flutter execution are not performed.",
        )),
        "cmakelists.txt" | "makefile" | "compile_commands.json" => Some(static_basic_detection(
            "cpp",
            "C++",
            "filename",
            "static-cpp",
            "Detected C/C++ build metadata; build tools, compilers, and package managers are not executed.",
        )),
        ".env" | ".env.example" | ".env.sample" | ".env.defaults" | ".env.template"
        | "example.env" | "sample.env" => Some(static_basic_detection(
            "env",
            "Env",
            "filename",
            "static-env",
            "Env-like files are parsed locally for key names and safe example/default values only; real secret env values are redacted.",
        )),
        _ => None,
    }
}

fn detect_by_extension(extension: &str) -> Option<LanguageDetectionResult> {
    match extension {
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
        "ts" => {
            return Some(static_basic_detection(
                "typescript",
                "TypeScript",
                "extension",
                "tree-sitter-typescript",
                "TypeScript files are parsed locally for basic symbols/imports/routes/components; no node, npm, tsc, or language server is required.",
            ));
        }
        "tsx" => {
            return Some(static_basic_detection(
                "tsx",
                "TSX",
                "extension",
                "tree-sitter-typescript",
                "TSX files are parsed locally for basic symbols/imports/components/routes; no browser, node, npm, tsc, or language server is required.",
            ));
        }
        "js" => {
            return Some(static_basic_detection(
                "javascript",
                "JavaScript",
                "extension",
                "tree-sitter-javascript",
                "JavaScript files are parsed locally for basic symbols/imports/routes; no node, npm, or language server is required.",
            ));
        }
        "jsx" => {
            return Some(static_basic_detection(
                "jsx",
                "JSX",
                "extension",
                "tree-sitter-javascript",
                "JSX files are parsed locally for basic symbols/imports/components; no browser, node, npm, or language server is required.",
            ));
        }
        "html" | "htm" | "cshtml" | "erb" | "ejs" | "hbs" => {
            return Some(static_basic_detection(
                "html",
                "HTML",
                "extension",
                "static-html",
                "HTML/template files are parsed with basic local static extraction; browser execution and external fetching are not performed.",
            ));
        }
        "css" => {
            return Some(static_basic_detection(
                "css",
                "CSS",
                "extension",
                "static-css",
                "CSS files are parsed with basic local static selector and asset-reference extraction.",
            ));
        }
        "scss" | "sass" => {
            return Some(static_basic_detection(
                "scss",
                "SCSS",
                "extension",
                "static-scss",
                "SCSS/Sass files are parsed with basic local static selector, variable, mixin, and asset-reference extraction.",
            ));
        }
        "json" => {
            return Some(static_basic_detection(
                "json",
                "JSON",
                "extension",
                "static-json",
                "JSON files are parsed statically for key paths and safe package/config metadata; sensitive values are not exposed.",
            ));
        }
        "yaml" | "yml" => {
            return Some(static_basic_detection(
                "yaml",
                "YAML",
                "extension",
                "static-yaml",
                "YAML files are parsed statically for key paths and safe config metadata; sensitive values are not exposed.",
            ));
        }
        "sql" => {
            return Some(static_basic_detection(
                "sql",
                "SQL",
                "extension",
                "static-sql",
                "SQL files are parsed statically for basic table/view/procedure definitions and table references without database connections.",
            ));
        }
        "ksql" => {
            return Some(static_basic_detection(
                "ksql",
                "ksqlDB",
                "extension",
                "static-ksql",
                "ksqlDB files are parsed statically for streams, tables, connectors, topics, and dependencies without broker or ksqlDB connections.",
            ));
        }
        "xaml" => {
            return Some(static_basic_detection(
                "xaml",
                "XAML",
                "extension",
                "static-xaml",
                "XAML files are parsed statically for WPF metadata, resources, bindings, and code-behind hints without Visual Studio, MSBuild, dotnet, or a XAML compiler.",
            ));
        }
        "py" => {
            return Some(static_basic_detection(
                "python",
                "Python",
                "extension",
                "static-python",
                "Python files are parsed with basic local static backend extraction.",
            ));
        }
        "java" => {
            return Some(static_basic_detection(
                "java",
                "Java",
                "extension",
                "static-java",
                "Java files are parsed with basic local static backend extraction.",
            ));
        }
        "php" => {
            return Some(static_basic_detection(
                "php",
                "PHP",
                "extension",
                "static-php",
                "PHP files are parsed with basic local static backend extraction.",
            ));
        }
        "rb" => {
            return Some(static_basic_detection(
                "ruby",
                "Ruby",
                "extension",
                "static-ruby",
                "Ruby files are parsed with basic local static backend extraction.",
            ));
        }
        "c" | "h" => {
            return Some(static_basic_detection(
                "c",
                "C",
                "extension",
                "static-c",
                "C files are parsed with basic local static extraction for includes, macros, declarations, structs, enums, and obvious functions.",
            ));
        }
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => {
            return Some(static_basic_detection(
                "cpp",
                "C++",
                "extension",
                "static-cpp",
                "C++ files are parsed with basic local static extraction for includes, namespaces, classes, methods, structs, enums, and obvious functions.",
            ));
        }
        "m" | "mm" => {
            return Some(static_basic_detection(
                "objective_c",
                "Objective-C",
                "extension",
                "static-objective-c",
                "Objective-C files are parsed with basic local static extraction for imports, interfaces, implementations, protocols, properties, and methods.",
            ));
        }
        "swift" => {
            return Some(static_basic_detection(
                "swift",
                "Swift",
                "extension",
                "static-swift",
                "Swift files are parsed with basic local static extraction for imports, types, extensions, functions, SwiftUI hints, and URLSession literals.",
            ));
        }
        "dart" => {
            return Some(static_basic_detection(
                "dart",
                "Dart",
                "extension",
                "static-dart",
                "Dart/Flutter files are parsed with basic local static extraction for imports, classes, widgets, build methods, route literals, and HTTP literals.",
            ));
        }
        "kt" | "kts" => {
            return Some(static_basic_detection(
                "kotlin",
                "Kotlin",
                "extension",
                "static-kotlin",
                "Kotlin files are parsed with basic local static backend extraction.",
            ));
        }
        "toml" => {
            return Some(static_basic_detection(
                "toml",
                "TOML",
                "extension",
                "static-toml",
                "TOML files are parsed statically for tables, key paths, and package/dependency names.",
            ));
        }
        "xml" => {
            return Some(static_basic_detection(
                "xml",
                "XML",
                "extension",
                "static-xml",
                "XML files are parsed statically for elements, attributes, and safe package/config metadata without schema fetching or entity expansion.",
            ));
        }
        _ => None,
    }
}

fn static_basic_detection(
    id: &str,
    name: &str,
    matched_by: &str,
    backend_id: &str,
    note: &str,
) -> LanguageDetectionResult {
    LanguageDetectionResult {
        language_id: Some(LanguageId(id.to_string())),
        language_name: Some(LanguageName(name.to_string())),
        support_level: LanguageSupportLevel::Basic,
        matched_by: matched_by.to_string(),
        backend_ids: vec![LanguageBackendId(backend_id.to_string())],
        notes: vec![note.to_string()],
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
        ".env.example",
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
        "ViewController.m",
        "App.swift",
        "main.dart",
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
    fn backend_languages_are_basic_static_and_offline() {
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
        for language in ["python", "java", "kotlin", "php", "ruby"] {
            let backend = registry
                .backends
                .iter()
                .find(|backend| backend.language_id.as_str() == language)
                .expect("phase 14 backend");
            assert_eq!(backend.support_level, LanguageSupportLevel::Basic);
            assert_eq!(backend.kind, LanguageBackendKind::StaticConfig);
            assert!(backend.available);
            assert!(backend
                .capabilities
                .contains(&LanguageBackendCapability::ExtractRoutes));
            assert!(backend
                .notes
                .iter()
                .any(|note| note.contains("No package manager")));
        }
    }

    #[test]
    fn phase17_support_matrix_keeps_implemented_web_and_xaml_backends_honest() {
        let registry = default_language_backend_registry();
        for (language, backend_id, kind) in [
            (
                "typescript",
                "tree-sitter-typescript",
                LanguageBackendKind::TreeSitter,
            ),
            (
                "javascript",
                "tree-sitter-javascript",
                LanguageBackendKind::TreeSitter,
            ),
            (
                "tsx",
                "tree-sitter-typescript",
                LanguageBackendKind::TreeSitter,
            ),
            (
                "jsx",
                "tree-sitter-javascript",
                LanguageBackendKind::TreeSitter,
            ),
            ("xaml", "static-xaml", LanguageBackendKind::StaticConfig),
            (
                "dockerfile",
                "static-dockerfile",
                LanguageBackendKind::StaticConfig,
            ),
            (
                "docker-compose",
                "static-compose",
                LanguageBackendKind::StaticConfig,
            ),
        ] {
            let backend = registry
                .backends
                .iter()
                .find(|backend| backend.language_id.as_str() == language)
                .expect("implemented backend");
            assert_eq!(backend.backend_id.0, backend_id);
            assert_eq!(backend.kind, kind);
            assert_eq!(backend.support_level, LanguageSupportLevel::Basic);
            assert!(backend.available);
            assert!(backend
                .notes
                .iter()
                .any(|note| note.contains("No") || note.contains("not executed")));
            if matches!(language, "dockerfile" | "docker-compose" | "xaml") {
                assert!(!backend
                    .capabilities
                    .contains(&LanguageBackendCapability::ExtractRoutes));
            }
        }
    }

    #[test]
    fn detect_only_support_level_is_distinct_from_basic() {
        let detection = LanguageDetectionResult {
            language_id: Some(LanguageId("future".to_string())),
            language_name: Some(LanguageName("Future".to_string())),
            support_level: LanguageSupportLevel::DetectOnly,
            matched_by: "test".to_string(),
            backend_ids: vec![LanguageBackendId("future-backend".to_string())],
            notes: vec!["Detected locally; parser/backend implementation is planned.".to_string()],
        };

        assert_eq!(detection.support_level, LanguageSupportLevel::DetectOnly);
        assert!(detection.notes.iter().any(|note| note.contains("planned")));
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
