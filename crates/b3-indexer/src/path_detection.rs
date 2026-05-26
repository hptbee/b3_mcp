use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use b3_core::ContractResult;
use sha2::{Digest, Sha256};

use crate::to_contract_error;

pub(crate) fn hash_file(path: &Path) -> ContractResult<String> {
    let bytes = fs::read(path).map_err(to_contract_error)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn stable_id(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{prefix}-{:x}", hasher.finalize())
}

pub(crate) fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn language_from_path(path: &Path) -> Option<String> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if let Some(file_name) = file_name.as_deref() {
        if file_name.starts_with(".env.") {
            return Some("env".to_string());
        }
        match file_name {
            "go.mod" => return Some("gomod".to_string()),
            "go.sum" => return Some("gosum".to_string()),
            "go.work" => return Some("gowork".to_string()),
            "pyproject.toml" | "requirements.txt" | "setup.cfg" | "pipfile" | "poetry.lock"
            | "uv.lock" => return Some("python_project".to_string()),
            "pom.xml" | "build.gradle" | "settings.gradle" => {
                return Some("java_project".to_string())
            }
            "build.gradle.kts" | "settings.gradle.kts" => {
                return Some("kotlin_project".to_string())
            }
            "composer.json" | "composer.lock" => return Some("php_project".to_string()),
            "gemfile" | "gemfile.lock" => return Some("ruby_project".to_string()),
            "cmakelists.txt" => return Some("cmake".to_string()),
            "makefile" => return Some("makefile".to_string()),
            "compile_commands.json" => return Some("compile_commands".to_string()),
            "package.swift" => return Some("swift_project".to_string()),
            "pubspec.yaml" | "analysis_options.yaml" => return Some("dart_project".to_string()),
            ".env" | ".env.example" | ".env.sample" | ".env.defaults" | ".env.template"
            | "example.env" | "sample.env" => return Some("env".to_string()),
            _ => {}
        }
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)?;
    Some(
        match extension.as_str() {
            "js" | "mjs" | "cjs" => "javascript",
            "jsx" => "jsx",
            "ts" | "mts" | "cts" => "typescript",
            "tsx" => "tsx",
            "cs" => "csharp",
            "csproj" => "csproj",
            "xaml" => "xaml",
            "py" => "python",
            "java" => "java",
            "kt" | "kts" => "kotlin",
            "php" => "php",
            "rb" => "ruby",
            "c" => "c",
            "h" => "c_header",
            "cpp" | "cc" | "cxx" => "cpp",
            "hpp" | "hh" => "cpp_header",
            "m" => "objective_c",
            "mm" => "objective_cpp",
            "swift" => "swift",
            "dart" => "dart",
            "yaml" | "yml" => "yaml",
            "json" => "json",
            "toml" => "toml",
            "xml" => "xml",
            "html" | "htm" | "cshtml" | "erb" | "ejs" | "hbs" => "html",
            "css" => "css",
            "scss" | "sass" => "scss",
            "ksql" => "ksql",
            "sql"
                if path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.to_ascii_lowercase().ends_with(".ksql.sql")) =>
            {
                "ksql"
            }
            "sql" => "sql",
            other => other,
        }
        .to_string(),
    )
}

pub(crate) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
