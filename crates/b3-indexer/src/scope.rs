//! Scoped indexing parser, validator, planner, and dry-run preview.

use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Component, Path, PathBuf},
};

use b3_core::{IndexScope, IndexScopeKind, ScopeError, ScopePreview};

use crate::{hash_file, language_from_path, relative_path, stable_id, DiscoveredFile, IgnoreRules};
use b3_core::FileId;

const SAMPLE_LIMIT: usize = 20;
const TARGET_LOOKUP_LIMIT: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopePlan {
    pub scope: IndexScope,
    pub files: Vec<DiscoveredFile>,
    pub preview: ScopePreview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeTarget {
    pub label: String,
    pub file_path: String,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub estimated_symbols: usize,
}

pub trait ScopeTargetProvider {
    fn targets(
        &self,
        scope: &IndexScope,
        project_id: &str,
        branch_id: &str,
        limit: usize,
    ) -> Result<Vec<ScopeTarget>, ScopeError>;
}

#[derive(Debug, Clone, Default)]
pub struct EmptyScopeTargetProvider;

impl ScopeTargetProvider for EmptyScopeTargetProvider {
    fn targets(
        &self,
        _scope: &IndexScope,
        _project_id: &str,
        _branch_id: &str,
        _limit: usize,
    ) -> Result<Vec<ScopeTarget>, ScopeError> {
        Ok(Vec::new())
    }
}

pub fn parse_scope(input: &str) -> Result<IndexScope, ScopeError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ScopeError::new("invalid_scope", "scope must not be empty"));
    }
    if input == "project" || input == "project:" {
        return Ok(IndexScope::project());
    }

    let (kind_text, value) = input.split_once(':').ok_or_else(|| {
        ScopeError::new(
            "invalid_scope",
            "scope must be 'project' or '<kind>:<value>'",
        )
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(ScopeError::new(
            "invalid_scope",
            "scope value must not be empty",
        ));
    }

    let (kind, normalized_value) = match kind_text.trim() {
        "path" => (IndexScopeKind::Path, value.to_string()),
        "file" => (IndexScopeKind::File, value.to_string()),
        "glob" => (IndexScopeKind::Glob, value.to_string()),
        "language" => (IndexScopeKind::Language, normalize_key(value)),
        "framework" => (IndexScopeKind::Framework, normalize_key(value)),
        "route" => (IndexScopeKind::Route, value.to_string()),
        "component" => (IndexScopeKind::Component, value.to_string()),
        "module" => (IndexScopeKind::Module, value.to_string()),
        "data_access" => (IndexScopeKind::DataAccess, normalize_key(value)),
        "realtime" => (IndexScopeKind::Realtime, normalize_key(value)),
        "messaging.topic" => (
            IndexScopeKind::Messaging,
            format!("topic={}", value.to_string()),
        ),
        "messaging.queue" => (
            IndexScopeKind::Messaging,
            format!("queue={}", value.to_string()),
        ),
        "messaging.routing_key" => (
            IndexScopeKind::Messaging,
            format!("routing_key={}", value.to_string()),
        ),
        "messaging.exchange" => (
            IndexScopeKind::Messaging,
            format!("exchange={}", value.to_string()),
        ),
        "messaging.pattern" => (
            IndexScopeKind::Messaging,
            format!("pattern={}", value.to_string()),
        ),
        "infrastructure" => (IndexScopeKind::Infrastructure, normalize_key(value)),
        other => {
            return Err(ScopeError::new(
                "unsupported_scope",
                format!("unsupported scope kind: {other}"),
            ))
        }
    };

    Ok(IndexScope::new(kind, Some(normalized_value)))
}

pub fn validate_scope(root: &Path, scope: &IndexScope) -> Result<(), ScopeError> {
    match scope.kind {
        IndexScopeKind::Project => Ok(()),
        IndexScopeKind::Path | IndexScopeKind::File => {
            let value = required_value(scope)?;
            let candidate = scoped_path(root, value)?;
            ensure_under_root(root, &candidate)?;
            Ok(())
        }
        IndexScopeKind::Glob => validate_glob(required_value(scope)?),
        IndexScopeKind::Language => validate_known_value(
            required_value(scope)?,
            &known_languages(),
            "unsupported_language",
            "language",
        ),
        IndexScopeKind::Framework => validate_known_value(
            required_value(scope)?,
            &known_frameworks(),
            "unsupported_framework",
            "framework",
        ),
        IndexScopeKind::DataAccess => validate_known_value(
            required_value(scope)?,
            &known_data_access(),
            "unsupported_data_access",
            "data_access",
        ),
        IndexScopeKind::Realtime => validate_known_value(
            required_value(scope)?,
            &known_realtime(),
            "unsupported_realtime",
            "realtime",
        ),
        IndexScopeKind::Infrastructure => validate_known_value(
            required_value(scope)?,
            &known_infrastructure(),
            "unsupported_infrastructure",
            "infrastructure",
        ),
        IndexScopeKind::Messaging => validate_messaging_value(required_value(scope)?),
        IndexScopeKind::Route | IndexScopeKind::Component | IndexScopeKind::Module => {
            required_value(scope)?;
            Ok(())
        }
    }
}

pub fn plan_scope(
    root: &Path,
    project_id: &str,
    branch_id: &str,
    scope: IndexScope,
    ignore: &IgnoreRules,
    provider: &dyn ScopeTargetProvider,
) -> Result<ScopePlan, ScopeError> {
    validate_scope(root, &scope)?;

    let mut warnings = Vec::new();
    let mut metadata_targets = Vec::new();
    let mut estimated_symbols = None;
    let mut files = match scope.kind {
        IndexScopeKind::Project => discover_project(root, project_id, ignore)?,
        IndexScopeKind::Path => discover_path(root, project_id, required_value(&scope)?, ignore)?,
        IndexScopeKind::File => discover_file(root, project_id, required_value(&scope)?, ignore)?,
        IndexScopeKind::Glob => discover_glob(root, project_id, required_value(&scope)?, ignore)?,
        IndexScopeKind::Language => {
            discover_by_language(root, project_id, required_value(&scope)?, ignore)?
        }
        IndexScopeKind::Framework => {
            discover_by_framework(root, project_id, required_value(&scope)?, ignore)?
        }
        IndexScopeKind::Route
        | IndexScopeKind::Component
        | IndexScopeKind::Module
        | IndexScopeKind::DataAccess
        | IndexScopeKind::Realtime
        | IndexScopeKind::Messaging
        | IndexScopeKind::Infrastructure => {
            let targets = provider.targets(&scope, project_id, branch_id, TARGET_LOOKUP_LIMIT)?;
            if targets.is_empty() {
                warnings.push(
                    "target scope matched no existing metadata; index a broader scope first if this project has not been indexed"
                        .to_string(),
                );
            }
            estimated_symbols = Some(targets.iter().map(|target| target.estimated_symbols).sum());
            metadata_targets = targets.iter().map(|target| target.label.clone()).collect();
            discover_target_files(root, project_id, ignore, &targets)?
        }
    };

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files.dedup_by(|left, right| left.relative_path == right.relative_path);
    if let Some(limit) = scope.limit {
        files.truncate(limit);
    }

    let mut matched_languages = BTreeSet::new();
    let mut matched_frameworks = BTreeSet::new();
    let mut skipped_reasons = Vec::new();
    for file in &files {
        if let Some(language) = language_from_path(Path::new(&file.relative_path)) {
            matched_languages.insert(language);
        }
        if let Ok(source) = fs::read_to_string(&file.path) {
            for framework in detect_frameworks_for_file(&file.relative_path, &source) {
                matched_frameworks.insert(framework);
            }
        } else {
            skipped_reasons.push(format!("could not read {}", file.relative_path));
        }
    }

    Ok(ScopePlan {
        preview: ScopePreview {
            scope: scope.display(),
            matched_files: files.len(),
            sample_files: files
                .iter()
                .take(SAMPLE_LIMIT)
                .map(|file| file.relative_path.clone())
                .collect(),
            matched_languages: matched_languages.into_iter().collect(),
            matched_frameworks: matched_frameworks.into_iter().collect(),
            estimated_symbols_affected: estimated_symbols,
            existing_metadata_targets: metadata_targets,
            warnings,
            skipped_reasons,
        },
        scope,
        files,
    })
}

fn discover_project(
    root: &Path,
    project_id: &str,
    ignore: &IgnoreRules,
) -> Result<Vec<DiscoveredFile>, ScopeError> {
    let mut files = Vec::new();
    discover_inner(root, root, project_id, ignore, &mut files)?;
    Ok(files)
}

fn discover_path(
    root: &Path,
    project_id: &str,
    value: &str,
    ignore: &IgnoreRules,
) -> Result<Vec<DiscoveredFile>, ScopeError> {
    let path = scoped_path(root, value)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    if path.is_file() {
        return discover_file_path(root, project_id, &path, ignore);
    }
    let mut files = Vec::new();
    discover_inner(root, &path, project_id, ignore, &mut files)?;
    Ok(files)
}

fn discover_file(
    root: &Path,
    project_id: &str,
    value: &str,
    ignore: &IgnoreRules,
) -> Result<Vec<DiscoveredFile>, ScopeError> {
    let path = scoped_path(root, value)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    discover_file_path(root, project_id, &path, ignore)
}

fn discover_glob(
    root: &Path,
    project_id: &str,
    pattern: &str,
    ignore: &IgnoreRules,
) -> Result<Vec<DiscoveredFile>, ScopeError> {
    Ok(discover_project(root, project_id, ignore)?
        .into_iter()
        .filter(|file| wildcard_match(pattern, &file.relative_path))
        .collect())
}

fn discover_by_language(
    root: &Path,
    project_id: &str,
    language: &str,
    ignore: &IgnoreRules,
) -> Result<Vec<DiscoveredFile>, ScopeError> {
    let expected = indexed_language_id(language);
    Ok(discover_project(root, project_id, ignore)?
        .into_iter()
        .filter(|file| {
            language_from_path(Path::new(&file.relative_path)).as_deref() == Some(expected)
        })
        .collect())
}

fn discover_by_framework(
    root: &Path,
    project_id: &str,
    framework: &str,
    ignore: &IgnoreRules,
) -> Result<Vec<DiscoveredFile>, ScopeError> {
    let mut files = Vec::new();
    for file in discover_project(root, project_id, ignore)? {
        if let Ok(source) = fs::read_to_string(&file.path) {
            if detect_frameworks_for_file(&file.relative_path, &source)
                .iter()
                .any(|detected| detected == framework)
            {
                files.push(file);
            }
        }
    }
    Ok(files)
}

fn discover_target_files(
    root: &Path,
    project_id: &str,
    ignore: &IgnoreRules,
    targets: &[ScopeTarget],
) -> Result<Vec<DiscoveredFile>, ScopeError> {
    let mut files = Vec::new();
    for target in targets {
        let path = scoped_path(root, &target.file_path)?;
        files.extend(discover_file_path(root, project_id, &path, ignore)?);
    }
    Ok(files)
}

fn discover_inner(
    root: &Path,
    current: &Path,
    project_id: &str,
    ignore: &IgnoreRules,
    files: &mut Vec<DiscoveredFile>,
) -> Result<(), ScopeError> {
    if let Some(_reason) = ignore.should_skip(current) {
        return Ok(());
    }
    for entry in fs::read_dir(current).map_err(scope_io_error)? {
        let entry = entry.map_err(scope_io_error)?;
        let path = entry.path();
        if let Some(_reason) = ignore.should_skip(&path) {
            continue;
        }
        let metadata = entry.metadata().map_err(scope_io_error)?;
        if metadata.is_dir() {
            discover_inner(root, &path, project_id, ignore, files)?;
        } else if metadata.is_file() {
            files.extend(discover_file_path(root, project_id, &path, ignore)?);
        }
    }
    Ok(())
}

fn discover_file_path(
    root: &Path,
    _project_id: &str,
    path: &Path,
    ignore: &IgnoreRules,
) -> Result<Vec<DiscoveredFile>, ScopeError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    if let Some(_reason) = ignore.should_skip(path) {
        return Ok(Vec::new());
    }
    let metadata = fs::metadata(path).map_err(scope_io_error)?;
    if !metadata.is_file() {
        return Ok(Vec::new());
    }
    let relative_path = relative_path(root, path);
    Ok(vec![DiscoveredFile {
        id: FileId::new(stable_id("file", &relative_path)),
        path: path.to_path_buf(),
        relative_path,
        content_hash: hash_file(path).map_err(|error| {
            ScopeError::new(
                "scope_io_error",
                format!("failed to hash scoped file: {error}"),
            )
        })?,
        size_bytes: metadata.len(),
    }])
}

fn detect_frameworks_for_file(relative_path: &str, source: &str) -> Vec<String> {
    let mut frameworks = BTreeSet::new();
    let path = relative_path.replace('\\', "/").to_ascii_lowercase();
    let source_lower = source.to_ascii_lowercase();
    let is_js_family = path.ends_with(".js")
        || path.ends_with(".mjs")
        || path.ends_with(".cjs")
        || path.ends_with(".jsx")
        || path.ends_with(".ts")
        || path.ends_with(".tsx")
        || path.ends_with("package.json")
        || path.ends_with("next.config.mjs")
        || path.ends_with("next.config.js")
        || path.ends_with("angular.json");
    let is_csharp_family = path.ends_with(".cs") || path.ends_with(".csproj");
    let is_xaml_family = path.ends_with(".xaml") || path.ends_with(".xaml.cs");

    if path.ends_with("dockerfile") {
        frameworks.insert("docker".to_string());
    }
    if path.ends_with("docker-compose.yml")
        || path.ends_with("docker-compose.yaml")
        || path.ends_with("compose.yml")
        || path.ends_with("compose.yaml")
    {
        frameworks.insert("docker_compose".to_string());
    }
    if path.ends_with(".tf") {
        frameworks.insert("terraform".to_string());
    }
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        if source_lower.contains("apiversion:")
            && (source_lower.contains("kind: deployment")
                || source_lower.contains("kind: service")
                || source_lower.contains("kind: pod"))
        {
            frameworks.insert("kubernetes".to_string());
        }
        if source_lower.contains("gke") || source_lower.contains("container.googleapis.com") {
            frameworks.insert("gke".to_string());
            frameworks.insert("gcp".to_string());
        }
    }
    if is_js_family {
        if source_lower.contains("express")
            || source_lower.contains(".get(")
            || source_lower.contains(".post(")
        {
            frameworks.insert("express".to_string());
        }
        if source_lower.contains("@nestjs") || source_lower.contains("@controller") {
            frameworks.insert("nestjs".to_string());
        }
        if source_lower.contains("fastify") {
            frameworks.insert("fastify".to_string());
        }
        if source_lower.contains("react") || path.ends_with(".tsx") || path.ends_with(".jsx") {
            frameworks.insert("react".to_string());
        }
        if path.contains("/app/") || path.contains("/pages/") || source_lower.contains("next/") {
            frameworks.insert("nextjs".to_string());
        }
        if source_lower.contains("@angular/")
            || source_lower.contains("@component")
            || source_lower.contains("@ngmodule")
        {
            frameworks.insert("angular".to_string());
        }
        for (needle, id) in [
            ("prisma", "prisma"),
            ("typeorm", "typeorm"),
            ("sequelize", "sequelize"),
            ("websocket", "websocket"),
            ("socket.io", "socketio"),
            ("rsocket", "rsocket"),
            ("rabbitmq", "rabbitmq"),
            ("kafka", "kafka"),
            ("pubsub", "google_pubsub"),
        ] {
            if source_lower.contains(needle) {
                frameworks.insert(id.to_string());
            }
        }
    }

    if is_csharp_family {
        if source_lower.contains("<usewpf>true</usewpf>")
            || source_lower.contains("microsoft.net.sdk.windowsdesktop")
            || source_lower.contains("presentationframework")
            || source_lower.contains("windowsbase")
            || source_lower.contains(": window")
            || source_lower.contains(": usercontrol")
            || source_lower.contains(": page")
        {
            frameworks.insert("wpf".to_string());
            frameworks.insert("dotnet_desktop".to_string());
        }
        if source_lower.contains("microsoft.aspnetcore") || source_lower.contains("[apicontroller]")
        {
            frameworks.insert("aspnetcore".to_string());
        }
        for (needle, id) in [
            ("entityframeworkcore", "ef_core"),
            ("dapper", "dapper"),
            ("signalr", "signalr"),
            ("rabbitmq", "rabbitmq"),
            ("kafka", "kafka"),
            ("pubsub", "google_pubsub"),
        ] {
            if source_lower.contains(needle) {
                frameworks.insert(id.to_string());
            }
        }
    }
    if is_xaml_family
        && (source_lower.contains("x:class")
            || source_lower.contains("<window")
            || source_lower.contains("<usercontrol")
            || source_lower.contains("<page")
            || source_lower.contains("<application")
            || source_lower.contains("<resourcedictionary"))
    {
        frameworks.insert("wpf".to_string());
        frameworks.insert("dotnet_desktop".to_string());
    }

    frameworks.into_iter().collect()
}

fn scoped_path(root: &Path, value: &str) -> Result<PathBuf, ScopeError> {
    if value.contains('\0') {
        return Err(ScopeError::new("invalid_path", "path contains NUL byte"));
    }
    let value_path = PathBuf::from(value);
    for component in value_path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(ScopeError::new(
                "path_traversal",
                "scope path must not contain '..'",
            ));
        }
    }
    let candidate = if value_path.is_absolute() {
        value_path
    } else {
        root.join(value_path)
    };
    ensure_under_root(root, &candidate)?;
    Ok(candidate)
}

fn ensure_under_root(root: &Path, candidate: &Path) -> Result<(), ScopeError> {
    let root = root.canonicalize().map_err(scope_io_error)?;
    let candidate = if candidate.exists() {
        candidate.canonicalize().map_err(scope_io_error)?
    } else {
        normalize_existing_parent(candidate)?
    };
    if candidate.starts_with(&root) {
        Ok(())
    } else {
        Err(ScopeError::new(
            "path_outside_project",
            "scope path must stay under the project root",
        ))
    }
}

fn normalize_existing_parent(candidate: &Path) -> Result<PathBuf, ScopeError> {
    let parent = candidate.parent().unwrap_or(candidate);
    let parent = parent.canonicalize().map_err(scope_io_error)?;
    Ok(parent.join(candidate.file_name().unwrap_or_default()))
}

fn validate_glob(value: &str) -> Result<(), ScopeError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(ScopeError::new("invalid_glob", "glob must not be empty"));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(ScopeError::new(
            "invalid_glob",
            "glob scope must be relative to the project root",
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ScopeError::new(
            "path_traversal",
            "glob scope must not contain '..'",
        ));
    }
    Ok(())
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let value = value.replace('\\', "/");
    wildcard_bytes(pattern.as_bytes(), value.as_bytes())
}

fn wildcard_bytes(pattern: &[u8], value: &[u8]) -> bool {
    let mut p = 0;
    let mut v = 0;
    let mut star = None;
    let mut match_after_star = 0;
    while v < value.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == value[v]) {
            p += 1;
            v += 1;
        } else if p + 1 < pattern.len() && pattern[p] == b'*' && pattern[p + 1] == b'*' {
            star = Some(p);
            p += 2;
            match_after_star = v;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            match_after_star = v;
        } else if let Some(star_pos) = star {
            p = if star_pos + 1 < pattern.len() && pattern[star_pos + 1] == b'*' {
                star_pos + 2
            } else {
                star_pos + 1
            };
            match_after_star += 1;
            v = match_after_star;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

fn validate_known_value(
    value: &str,
    known: &BTreeSet<&'static str>,
    code: &str,
    label: &str,
) -> Result<(), ScopeError> {
    if known.contains(value) {
        Ok(())
    } else {
        Err(ScopeError::new(
            code,
            format!("unsupported {label}: {value}"),
        ))
    }
}

fn validate_messaging_value(value: &str) -> Result<(), ScopeError> {
    let Some((field, target)) = value.split_once('=') else {
        return validate_known_value(
            value,
            &known_messaging(),
            "unsupported_messaging",
            "messaging",
        );
    };
    if !["topic", "queue", "routing_key", "exchange", "pattern"].contains(&field) {
        return Err(ScopeError::new(
            "unsupported_messaging",
            format!("unsupported messaging selector: {field}"),
        ));
    }
    if target.trim().is_empty() {
        return Err(ScopeError::new(
            "invalid_scope",
            "messaging target must not be empty",
        ));
    }
    Ok(())
}

fn required_value(scope: &IndexScope) -> Result<&str, ScopeError> {
    scope.value.as_deref().ok_or_else(|| {
        ScopeError::new(
            "invalid_scope",
            format!("scope {:?} requires a value", scope.kind),
        )
    })
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

fn indexed_language_id(language: &str) -> &str {
    match language {
        "rust" => "rs",
        "dockerfile" => "dockerfile",
        "terraform" => "tf",
        other => other,
    }
}

fn known_languages() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "rust",
        "rs",
        "typescript",
        "javascript",
        "tsx",
        "jsx",
        "csharp",
        "csproj",
        "xaml",
        "go",
        "gomod",
        "yaml",
        "dockerfile",
        "terraform",
    ])
}

fn known_frameworks() -> BTreeSet<&'static str> {
    let mut values = known_data_access();
    values.extend(known_realtime());
    values.extend(known_messaging());
    values.extend(known_infrastructure());
    values.extend([
        "express",
        "nestjs",
        "fastify",
        "react",
        "nextjs",
        "angular",
        "aspnetcore",
        "wpf",
        "dotnet_desktop",
        "gcp",
        "gke",
    ]);
    values
}

fn known_data_access() -> BTreeSet<&'static str> {
    BTreeSet::from(["ef_core", "dapper", "prisma", "typeorm", "sequelize"])
}

fn known_realtime() -> BTreeSet<&'static str> {
    BTreeSet::from(["websocket", "socketio", "signalr", "rsocket"])
}

fn known_messaging() -> BTreeSet<&'static str> {
    BTreeSet::from(["rabbitmq", "amqp", "kafka", "google_pubsub", "nestjs"])
}

fn known_infrastructure() -> BTreeSet<&'static str> {
    BTreeSet::from(["docker", "docker_compose", "kubernetes", "terraform"])
}

fn scope_io_error(error: impl std::fmt::Display) -> ScopeError {
    ScopeError::new("scope_io_error", error.to_string())
}

pub fn target_field_map(value: &str) -> HashMap<&str, &str> {
    value
        .split(';')
        .filter_map(|part| part.split_once('='))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_supported_scope_forms() {
        assert_eq!(
            parse_scope("project").unwrap().kind,
            IndexScopeKind::Project
        );
        assert_eq!(
            parse_scope("path:src/orders").unwrap().kind,
            IndexScopeKind::Path
        );
        assert_eq!(
            parse_scope("file:src/main.go").unwrap().kind,
            IndexScopeKind::File
        );
        assert_eq!(
            parse_scope("glob:**/*.controller.ts").unwrap().kind,
            IndexScopeKind::Glob
        );
        assert_eq!(
            parse_scope("language:typescript").unwrap().value.as_deref(),
            Some("typescript")
        );
        assert_eq!(
            parse_scope("messaging.topic:order.created")
                .unwrap()
                .value
                .as_deref(),
            Some("topic=order.created")
        );
    }

    #[test]
    fn rejects_invalid_and_traversal_scope() {
        assert!(parse_scope("").is_err());
        assert!(parse_scope("unknown:value").is_err());
        let root = std::env::temp_dir();
        assert!(validate_scope(&root, &parse_scope("path:../outside").unwrap()).is_err());
        assert!(validate_scope(&root, &parse_scope("glob:../*.rs").unwrap()).is_err());
        assert!(validate_scope(&root, &parse_scope("language:brainfuck").unwrap()).is_err());
    }

    #[test]
    fn dry_run_preview_for_path_glob_language_framework_and_empty_target() {
        let root = std::env::temp_dir().join(format!("b3-scope-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src").join("orders")).unwrap();
        fs::write(
            root.join("src").join("orders").join("order.controller.ts"),
            "import express from 'express';",
        )
        .unwrap();
        fs::write(
            root.join("src").join("orders").join("OrderList.tsx"),
            "import React from 'react'; export function OrderList(){ return <div/> }",
        )
        .unwrap();

        let path = plan_scope(
            &root,
            "project",
            "main",
            parse_scope("path:src/orders").unwrap(),
            &IgnoreRules::default(),
            &EmptyScopeTargetProvider,
        )
        .unwrap();
        assert_eq!(path.preview.matched_files, 2);

        let glob = plan_scope(
            &root,
            "project",
            "main",
            parse_scope("glob:**/*.controller.ts").unwrap(),
            &IgnoreRules::default(),
            &EmptyScopeTargetProvider,
        )
        .unwrap();
        assert_eq!(glob.preview.matched_files, 1);

        let language = plan_scope(
            &root,
            "project",
            "main",
            parse_scope("language:tsx").unwrap(),
            &IgnoreRules::default(),
            &EmptyScopeTargetProvider,
        )
        .unwrap();
        assert_eq!(language.preview.matched_files, 1);

        let framework = plan_scope(
            &root,
            "project",
            "main",
            parse_scope("framework:react").unwrap(),
            &IgnoreRules::default(),
            &EmptyScopeTargetProvider,
        )
        .unwrap();
        assert_eq!(framework.preview.matched_files, 1);

        let target = plan_scope(
            &root,
            "project",
            "main",
            parse_scope("route:/missing").unwrap(),
            &IgnoreRules::default(),
            &EmptyScopeTargetProvider,
        )
        .unwrap();
        assert_eq!(target.preview.matched_files, 0);
        assert!(!target.preview.warnings.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wildcard_supports_recursive_controller_match() {
        assert!(wildcard_match(
            "**/*.controller.ts",
            "src/orders/order.controller.ts"
        ));
        assert!(!wildcard_match(
            "**/*.controller.ts",
            "src/orders/order.service.ts"
        ));
    }
}
