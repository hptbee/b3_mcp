use b3_core::NodeKind;

use crate::{ExtractedSymbol, ParseInput};

use super::{file_path, line_of, local_type_name, metadata, symbol, wpf};

pub(crate) fn extract_xaml_symbols(input: &ParseInput) -> Vec<ExtractedSymbol> {
    let Some(root) = root_element(&input.source) else {
        return Vec::new();
    };
    let kind = wpf::xaml_kind(&root);
    if kind == "Unknown"
        && !input
            .path
            .file_name()
            .and_then(|v| v.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("app.xaml"))
    {
        return Vec::new();
    }

    let x_class = attr_value(&input.source, "x:Class");
    let name = x_class
        .as_deref()
        .map(local_type_name)
        .or_else(|| {
            input
                .path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| kind.to_string());
    let code_behind = code_behind_path(input);
    let data_context = data_context(&input.source);
    let binding_paths = binding_paths(&input.source);
    let command_bindings = command_bindings(&input.source);
    let resource_keys = resource_keys(&input.source);
    let resource_sources = resource_sources(&input.source);
    let view_model = data_context
        .clone()
        .or_else(|| view_model_hint(x_class.as_deref(), &name));
    let source_kind = match kind {
        "Application" => "XamlApplication",
        "Window" => "XamlWindow",
        "UserControl" => "XamlUserControl",
        "Page" => "XamlPage",
        "ResourceDictionary" => "XamlResourceDictionary",
        _ => "XamlClass",
    };
    let line = x_class
        .as_deref()
        .map(|value| line_of(&input.source, value))
        .unwrap_or(1);

    vec![symbol(
        input,
        name.clone(),
        NodeKind::Endpoint,
        source_kind,
        line,
        line,
        metadata(&[
            ("technology", "wpf".to_string()),
            ("kind", kind.to_string()),
            ("name", name),
            ("x_class", x_class.unwrap_or_default()),
            ("code_behind", code_behind.unwrap_or_default()),
            ("view_model", view_model.unwrap_or_default()),
            ("binding_paths", binding_paths.join(",")),
            ("command_bindings", command_bindings.join(",")),
            ("resource_keys", resource_keys.join(",")),
            ("resource_sources", resource_sources.join(",")),
            ("data_context", data_context.unwrap_or_default()),
            ("file", file_path(input)),
            ("line_start", line.to_string()),
            ("line_end", line.to_string()),
            ("confidence", "9000".to_string()),
            ("source", source_kind.to_string()),
        ]),
    )]
}

pub(crate) fn local_name(value: &str) -> &str {
    value.rsplit(':').next().unwrap_or(value)
}

fn root_element(source: &str) -> Option<String> {
    let bytes = source.as_bytes();
    let mut index = 0;
    while let Some(offset) = source[index..].find('<') {
        index += offset + 1;
        if index >= bytes.len() {
            return None;
        }
        let rest = &source[index..];
        if rest.starts_with('?') || rest.starts_with('!') || rest.starts_with('/') {
            continue;
        }
        let name = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == ':' || *ch == '_')
            .collect::<String>();
        return (!name.is_empty()).then_some(name);
    }
    None
}

fn attr_value(source: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{attr}={quote}");
        if let Some(start) = source.find(&needle) {
            let value_start = start + needle.len();
            let value = &source[value_start..];
            let end = value.find(quote)?;
            return Some(value[..end].trim().to_string());
        }
    }
    None
}

fn data_context(source: &str) -> Option<String> {
    if let Some(value) = attr_value(source, "DataContext") {
        if let Some(binding) = parse_binding_path(&value) {
            return Some(binding);
        }
    }
    let marker = ".DataContext>";
    let start = source.find(marker)?;
    let after = &source[start + marker.len()..];
    let tag_start = after.find('<')? + 1;
    let tag = after[tag_start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == ':' || *ch == '_')
        .collect::<String>();
    (!tag.is_empty()).then(|| local_name(&tag).to_string())
}

fn binding_paths(source: &str) -> Vec<String> {
    markup_extensions(source, "{Binding")
        .into_iter()
        .filter_map(|value| parse_binding_path(&value))
        .filter(|value| !value.ends_with("Command"))
        .collect()
}

fn command_bindings(source: &str) -> Vec<String> {
    let mut commands = Vec::new();
    for attr in ["Command", "CommandParameter"] {
        let mut rest = source;
        let needle = format!("{attr}=");
        while let Some(index) = rest.find(&needle) {
            rest = &rest[index + needle.len()..];
            let Some(quote) = rest.chars().next().filter(|ch| *ch == '"' || *ch == '\'') else {
                continue;
            };
            rest = &rest[1..];
            let Some(end) = rest.find(quote) else {
                break;
            };
            let value = &rest[..end];
            if let Some(path) = parse_binding_path(value) {
                commands.push(path);
            }
            rest = &rest[end + 1..];
        }
    }
    commands.sort();
    commands.dedup();
    commands
}

fn resource_keys(source: &str) -> Vec<String> {
    let mut keys = Vec::new();
    keys.extend(markup_extensions(source, "{StaticResource"));
    keys.extend(markup_extensions(source, "{DynamicResource"));
    keys.extend(attr_values(source, "x:Key"));
    keys.into_iter()
        .filter_map(|value| value.split_whitespace().next().map(str::to_string))
        .collect()
}

fn resource_sources(source: &str) -> Vec<String> {
    attr_values(source, "Source")
        .into_iter()
        .filter(|value| value.ends_with(".xaml") || value.contains(".xaml#"))
        .collect()
}

fn markup_extensions(source: &str, prefix: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = source;
    while let Some(index) = rest.find(prefix) {
        rest = &rest[index + prefix.len()..];
        let Some(end) = rest.find('}') else {
            break;
        };
        values.push(rest[..end].trim().to_string());
        rest = &rest[end + 1..];
    }
    values
}

fn attr_values(source: &str, attr: &str) -> Vec<String> {
    let mut values = Vec::new();
    for quote in ['"', '\''] {
        let needle = format!("{attr}={quote}");
        let mut rest = source;
        while let Some(index) = rest.find(&needle) {
            rest = &rest[index + needle.len()..];
            let Some(end) = rest.find(quote) else {
                break;
            };
            values.push(rest[..end].trim().to_string());
            rest = &rest[end + 1..];
        }
    }
    values
}

fn parse_binding_path(value: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    let value = value.strip_prefix("Binding").unwrap_or(value).trim();
    if value.is_empty() {
        return None;
    }
    for part in value.split(',').map(str::trim) {
        if let Some(path) = part.strip_prefix("Path=") {
            return Some(path.trim().to_string());
        }
        if !part.contains('=') {
            return Some(part.to_string());
        }
    }
    None
}

fn code_behind_path(input: &ParseInput) -> Option<String> {
    let path = input.path.with_extension("xaml.cs");
    Some(path.to_string_lossy().replace('\\', "/"))
}

fn view_model_hint(x_class: Option<&str>, name: &str) -> Option<String> {
    if let Some(x_class) = x_class {
        if x_class.contains(".Views.") {
            return Some(format!(
                "{}ViewModel",
                local_name(x_class).trim_end_matches("View")
            ));
        }
    }
    if matches!(name, "Application" | "ResourceDictionary") {
        None
    } else {
        Some(format!("{}ViewModel", name.trim_end_matches("View")))
    }
}
