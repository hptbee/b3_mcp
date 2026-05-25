use std::path::Path;

use b3_core::NodeKind;

use crate::{
    DetectedTechnology, ParseInput, TechnologyCapability, TechnologyKind, TechnologySupportLevel,
};

use super::{file_path, line_of, metadata, symbol, xaml};

pub(crate) fn is_dotnet_desktop_file(path: &Path, source: &str) -> bool {
    super::is_xaml_file(path)
        || (super::is_csproj_file(path) && is_wpf_project(source))
        || (super::is_csharp_file(path) && is_wpf_csharp(source))
}

pub fn detect_wpf_project_technologies(source: &str) -> Result<Vec<DetectedTechnology>, String> {
    if is_wpf_project(source) {
        Ok(vec![
            DetectedTechnology {
                id: "wpf".to_string(),
                name: "WPF".to_string(),
                kind: TechnologyKind::WebFrontend,
                support_level: TechnologySupportLevel::Basic,
                capabilities: vec![
                    TechnologyCapability::DetectPackage,
                    TechnologyCapability::ExtractComponents,
                ],
                source: "csproj".to_string(),
            },
            DetectedTechnology {
                id: "dotnet_desktop".to_string(),
                name: ".NET Desktop".to_string(),
                kind: TechnologyKind::Runtime,
                support_level: TechnologySupportLevel::Basic,
                capabilities: vec![TechnologyCapability::DetectPackage],
                source: "csproj".to_string(),
            },
        ])
    } else {
        Ok(Vec::new())
    }
}

pub(crate) fn extract_project_symbols(input: &ParseInput) -> Vec<crate::ExtractedSymbol> {
    if !is_wpf_project(&input.source) {
        return Vec::new();
    }
    let source_kind = if contains_ci(&input.source, "UseWPF") {
        "WpfProjectUseWpf"
    } else {
        "WpfProjectPresentationFramework"
    };
    let name = input
        .path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("WpfProject");
    vec![symbol(
        input,
        name,
        NodeKind::ConfigKey,
        source_kind,
        line_of(&input.source, "UseWPF"),
        1,
        metadata(&[
            ("technology", "wpf".to_string()),
            ("kind", "Project".to_string()),
            ("name", name.to_string()),
            ("file", file_path(input)),
            ("line_start", "1".to_string()),
            ("line_end", "1".to_string()),
            ("confidence", "9000".to_string()),
            ("source", source_kind.to_string()),
        ]),
    )]
}

pub(crate) fn is_wpf_project(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    (lower.contains("<usewpf>true</usewpf>")
        || lower.contains("microsoft.net.sdk.windowsdesktop")
        || lower.contains("<outputtype>winexe</outputtype>")
            && lower.contains("-windows")
            && lower.contains("<usewpf>"))
        || (lower.contains("presentationframework")
            && lower.contains("windowsbase")
            && (lower.contains("<page include=") || lower.contains("<applicationdefinition")))
}

pub(crate) fn is_wpf_csharp(source: &str) -> bool {
    (source.contains("partial class")
        && (source.contains(": Window")
            || source.contains(": UserControl")
            || source.contains(": Page")
            || source.contains("DataContext = new")))
        || source.contains("ViewModel")
        || source.contains("INotifyPropertyChanged")
        || source.contains("ICommand")
}

fn contains_ci(source: &str, needle: &str) -> bool {
    source
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

#[cfg(test)]
pub(crate) fn wpf_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("wpf.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.trim().to_string())
    })
}

pub(crate) fn xaml_kind(root: &str) -> &'static str {
    match xaml::local_name(root) {
        "Application" => "Application",
        "Window" | "NavigationWindow" => "Window",
        "UserControl" => "UserControl",
        "Page" => "Page",
        "ResourceDictionary" => "ResourceDictionary",
        _ => "Unknown",
    }
}
