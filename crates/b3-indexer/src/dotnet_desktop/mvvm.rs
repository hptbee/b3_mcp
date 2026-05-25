use b3_core::NodeKind;

use crate::{ExtractedSymbol, ParseInput};

use super::{file_path, line_of, metadata, symbol};

pub(crate) fn extract_csharp_symbols(input: &ParseInput) -> Vec<ExtractedSymbol> {
    let mut symbols = Vec::new();
    symbols.extend(code_behind_symbol(input));
    symbols.extend(view_model_symbol(input));
    symbols
}

fn code_behind_symbol(input: &ParseInput) -> Option<ExtractedSymbol> {
    if !input.source.contains("partial class") {
        return None;
    }
    let class_name = class_name_after(&input.source, "partial class")?;
    let base_type = base_type_for(&input.source, &class_name).unwrap_or_default();
    if !matches!(
        base_type.as_str(),
        "Window" | "UserControl" | "Page" | "Application"
    ) {
        return None;
    }
    let line = line_of(&input.source, &class_name);
    Some(symbol(
        input,
        class_name.clone(),
        NodeKind::Class,
        "CodeBehindPartialClass",
        line,
        line,
        metadata(&[
            ("technology", "wpf".to_string()),
            ("kind", "CodeBehind".to_string()),
            ("name", class_name),
            ("code_behind", file_path(input)),
            (
                "data_context",
                data_context_assignment(&input.source).unwrap_or_default(),
            ),
            ("file", file_path(input)),
            ("line_start", line.to_string()),
            ("line_end", line.to_string()),
            ("confidence", "8500".to_string()),
            ("source", "CodeBehindPartialClass".to_string()),
        ]),
    ))
}

fn view_model_symbol(input: &ParseInput) -> Option<ExtractedSymbol> {
    let class_name = class_name_after(&input.source, "class ")?;
    let looks_like_vm = class_name.ends_with("ViewModel")
        || input.source.contains("INotifyPropertyChanged")
        || input.source.contains("ICommand");
    if !looks_like_vm {
        return None;
    }
    let commands = command_properties(&input.source);
    let line = line_of(&input.source, &class_name);
    Some(symbol(
        input,
        class_name.clone(),
        NodeKind::Class,
        "ViewModelNamingHint",
        line,
        line,
        metadata(&[
            ("technology", "wpf".to_string()),
            ("kind", "ViewModel".to_string()),
            ("name", class_name.clone()),
            ("view_model", class_name),
            ("command_bindings", commands.join(",")),
            ("file", file_path(input)),
            ("line_start", line.to_string()),
            ("line_end", line.to_string()),
            ("confidence", "7500".to_string()),
            ("source", "ViewModelNamingHint".to_string()),
        ]),
    ))
}

fn class_name_after(source: &str, marker: &str) -> Option<String> {
    let index = source.find(marker)? + marker.len();
    let rest = source[index..].trim_start();
    let name = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn base_type_for(source: &str, class_name: &str) -> Option<String> {
    let index = source.find(class_name)? + class_name.len();
    let rest = source[index..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    Some(
        rest.chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '.')
            .collect::<String>()
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_string(),
    )
}

fn data_context_assignment(source: &str) -> Option<String> {
    let marker = "DataContext = new ";
    let index = source.find(marker)? + marker.len();
    let rest = &source[index..];
    let name = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn command_properties(source: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut rest = source;
    while let Some(index) = rest.find("ICommand ") {
        rest = &rest[index + "ICommand ".len()..];
        let name = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if !name.is_empty() {
            commands.push(name);
        }
    }
    commands.sort();
    commands.dedup();
    commands
}
