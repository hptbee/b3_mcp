use super::*;

pub(super) fn annotate_react_components(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &mut [ExtractedSymbol],
) {
    let mut candidates = Vec::new();
    collect_react_component_candidates(root, input, &mut candidates);
    for (name, node, metadata) in candidates {
        if let Some(symbol) = symbols.iter_mut().find(|symbol| {
            symbol.name == name
                && symbol.start_byte <= node.start_byte()
                && symbol.end_byte >= node.end_byte()
                && matches!(symbol.kind, NodeKind::Function | NodeKind::Class)
        }) {
            symbol.visibility = merge_visibility(
                symbol.visibility.take(),
                encode_component_metadata(&metadata),
            );
        }
    }
}

fn collect_react_component_candidates<'a>(
    node: Node<'a>,
    input: &ParseInput,
    candidates: &mut Vec<(String, Node<'a>, ComponentMetadata)>,
) {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) =
                    react_function_component_metadata(&name, node, input, "FunctionDeclaration")
                {
                    candidates.push((name, node, metadata));
                }
            }
        }
        "class_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) = react_class_component_metadata(&name, node, input) {
                    candidates.push((name, node, metadata));
                }
            }
        }
        "variable_declarator" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) = react_variable_component_metadata(&name, node, input) {
                    candidates.push((name, node, metadata));
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_react_component_candidates(child, input, candidates);
    }
}

fn react_function_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
    source_kind: &str,
) -> Option<ComponentMetadata> {
    if !is_pascal_case(name) || !node_contains_jsx(node, &input.source) {
        return None;
    }
    let text = node_text(node, &input.source);
    let boundary = nextjs_component_boundary(&input.source, &input.path);
    Some(ComponentMetadata {
        framework: boundary
            .as_deref()
            .map(|_| "nextjs")
            .unwrap_or("react")
            .to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: boundary.unwrap_or_else(|| "function".to_string()),
        props_type_name: props_type_from_function_text(text),
        hooks: detect_hook_names(text),
        usages: detect_jsx_component_usages(text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_500,
        source_kind: source_kind.to_string(),
    })
}

fn react_class_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
) -> Option<ComponentMetadata> {
    let text = node_text(node, &input.source);
    if !is_pascal_case(name)
        || !(text.contains("React.Component") || text.contains("Component<"))
        || !node_contains_jsx(node, &input.source)
    {
        return None;
    }
    let boundary = nextjs_component_boundary(&input.source, &input.path);
    Some(ComponentMetadata {
        framework: boundary
            .as_deref()
            .map(|_| "nextjs")
            .unwrap_or("react")
            .to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: boundary.unwrap_or_else(|| "class".to_string()),
        props_type_name: type_between_after(text, "Component<"),
        hooks: Vec::new(),
        usages: detect_jsx_component_usages(text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_000,
        source_kind: "ClassComponent".to_string(),
    })
}

fn react_variable_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
) -> Option<ComponentMetadata> {
    if !is_pascal_case(name) {
        return None;
    }
    let text = node_text(node, &input.source);
    let value = node.child_by_field_name("value")?;
    let value_text = node_text(value, &input.source);
    let (component_kind, source_kind, confidence) = if value.kind() == "call_expression"
        && text.contains("memo")
        && node_contains_jsx(node, &input.source)
    {
        ("memo", "ReactMemo", 8_500)
    } else if value.kind() == "call_expression"
        && text.contains("forwardRef")
        && node_contains_jsx(node, &input.source)
    {
        ("forward_ref", "ReactForwardRef", 8_500)
    } else if matches!(
        value.kind(),
        "arrow_function" | "function" | "function_expression"
    ) && node_contains_jsx(value, &input.source)
    {
        ("arrow_function", "ArrowFunction", 9_500)
    } else {
        return None;
    };
    let boundary = nextjs_component_boundary(&input.source, &input.path);

    Some(ComponentMetadata {
        framework: boundary
            .as_deref()
            .map(|_| "nextjs")
            .unwrap_or("react")
            .to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: boundary.unwrap_or_else(|| component_kind.to_string()),
        props_type_name: props_type_from_variable_text(text),
        hooks: detect_hook_names(value_text),
        usages: detect_jsx_component_usages(value_text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence,
        source_kind: source_kind.to_string(),
    })
}

pub(super) fn export_kind_for_node(node: Node<'_>, source: &str) -> Option<String> {
    if has_parent_kind(node, "export_statement") {
        let parent_text = ancestor_text(node, source, "export_statement").unwrap_or_default();
        if parent_text.trim_start().starts_with("export default") {
            Some("default".to_string())
        } else {
            Some("named".to_string())
        }
    } else {
        None
    }
}

fn nextjs_component_boundary(source: &str, path: &Path) -> Option<String> {
    if !path_has_segment(path, "app") {
        return None;
    }
    match top_of_file_directive(source).as_deref() {
        Some("use client") => Some("client_component".to_string()),
        Some("use server") => Some("server_component".to_string()),
        _ => Some("server_component".to_string()),
    }
}

fn top_of_file_directive(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let directive = trimmed.trim_end_matches(';');
        if directive == "\"use client\"" || directive == "'use client'" {
            return Some("use client".to_string());
        }
        if directive == "\"use server\"" || directive == "'use server'" {
            return Some("use server".to_string());
        }
        return None;
    }
    None
}

fn path_has_segment(path: &Path, segment: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_string_lossy() == segment)
}

fn ancestor_text<'a>(node: Node<'a>, source: &'a str, kind: &str) -> Option<&'a str> {
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.kind() == kind {
            return Some(node_text(current, source));
        }
        parent = current.parent();
    }
    None
}

fn node_contains_jsx(node: Node<'_>, source: &str) -> bool {
    let text = node_text(node, source);
    text.contains("</")
        || text.contains("/>")
        || text.contains("React.createElement")
        || text.contains("jsx(")
}

fn is_pascal_case(name: &str) -> bool {
    name.chars()
        .next()
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false)
}

fn props_type_from_function_text(text: &str) -> Option<String> {
    text.split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(params, _)| props_type_from_params(params))
}

fn props_type_from_variable_text(text: &str) -> Option<String> {
    type_between_after(text, "React.FC<")
        .or_else(|| type_between_after(text, "FC<"))
        .or_else(|| {
            text.split_once("=>")
                .and_then(|(before_arrow, _)| before_arrow.rsplit_once('('))
                .and_then(|(_, params)| props_type_from_params(params.trim_end_matches(')')))
        })
}

fn props_type_from_params(params: &str) -> Option<String> {
    params
        .split(':')
        .nth(1)
        .map(|value| {
            value
                .split(|character: char| {
                    character == ','
                        || character == '='
                        || character == ')'
                        || character.is_whitespace()
                })
                .find(|part| !part.is_empty())
                .unwrap_or_default()
                .trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn type_between_after(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find('>')?;
    let value = rest[..end]
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    (!value.is_empty()).then_some(value)
}

fn detect_hook_names(text: &str) -> Vec<String> {
    let mut hooks = Vec::new();
    for token in text.split(|character: char| !character.is_alphanumeric() && character != '_') {
        if is_hook_name(token) && !hooks.iter().any(|hook| hook == token) {
            hooks.push(token.to_string());
        }
    }
    hooks
}

fn is_hook_name(token: &str) -> bool {
    matches!(
        token,
        "useState"
            | "useEffect"
            | "useMemo"
            | "useCallback"
            | "useRef"
            | "useContext"
            | "useReducer"
    ) || token
        .strip_prefix("use")
        .and_then(|rest| rest.chars().next())
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false)
}

fn detect_jsx_component_usages(text: &str) -> Vec<String> {
    let mut usages = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while let Some(offset) = text[index..].find('<') {
        index += offset + 1;
        if index >= bytes.len() || bytes[index] == b'/' || bytes[index] == b'>' {
            continue;
        }
        let start = index;
        while index < bytes.len()
            && ((bytes[index] as char).is_ascii_alphanumeric()
                || bytes[index] == b'_'
                || bytes[index] == b'.')
        {
            index += 1;
        }
        let tag = &text[start..index];
        if tag
            .chars()
            .next()
            .map(|character| character.is_ascii_uppercase())
            .unwrap_or(false)
            && !usages.iter().any(|usage| usage == tag)
        {
            usages.push(tag.to_string());
        }
    }
    usages
}
