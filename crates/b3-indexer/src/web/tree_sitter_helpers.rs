use super::*;

pub(super) fn symbol_from_node(
    input: &ParseInput,
    node: Node<'_>,
    name: String,
    kind: NodeKind,
    visibility: Option<String>,
) -> ExtractedSymbol {
    let start = node.start_position();
    let end = node.end_position();
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{kind:?}:{name}:{}:{}",
                input.file_id.as_str(),
                node.start_byte(),
                node.end_byte()
            ),
        )),
        file_id: input.file_id.clone(),
        name,
        kind,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_line: one_based_row(start),
        start_column: start.column,
        end_line: one_based_row(end),
        end_column: end.column,
        visibility,
    }
}

pub(super) fn first_child_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let value = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind);
    value
}

pub(super) fn object_property_string(object_text: &str, key: &str) -> Option<String> {
    let key_position = object_text.find(key)?;
    let after_key = &object_text[key_position + key.len()..];
    let colon_position = after_key.find(':')?;
    let after_colon = after_key[colon_position + 1..].trim_start();
    let quote = after_colon
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_colon.split_once(quote)?.1;
    Some(after_quote.split_once(quote)?.0.to_string())
}

pub(super) fn object_property_identifier(object_text: &str, key: &str) -> Option<String> {
    let key_position = object_text.find(key)?;
    let after_key = &object_text[key_position + key.len()..];
    let colon_position = after_key.find(':')?;
    let after_colon = after_key[colon_position + 1..].trim_start();
    let value = after_colon
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '.'
        })
        .next()
        .unwrap_or_default();
    (!value.is_empty()).then(|| value.to_string())
}

pub(super) fn decorator_argument(text: &str, decorator_name: &str) -> Option<String> {
    let needle = format!("@{decorator_name}");
    let position = text.find(&needle)?;
    let after = &text[position + needle.len()..];
    let open = after.find('(')?;
    let after_open = after[open + 1..].trim_start();
    if after_open.starts_with(')') {
        return Some(String::new());
    }
    let quote = after_open
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_open.split_once(quote)?.1;
    Some(after_quote.split_once(quote)?.0.to_string())
}

pub(super) fn leading_decorator_text(node: Node<'_>, source: &str) -> String {
    let mut parts = Vec::new();
    let mut sibling = node.prev_named_sibling();
    while let Some(value) = sibling {
        let text = node_text(value, source).trim();
        if !text.starts_with('@') {
            break;
        }
        parts.push(text.to_string());
        sibling = value.prev_named_sibling();
    }
    if parts.is_empty() {
        if let Some(parent) = node.parent() {
            let mut parent_sibling = parent.prev_named_sibling();
            while let Some(value) = parent_sibling {
                let text = node_text(value, source).trim();
                if !text.starts_with('@') {
                    break;
                }
                parts.push(text.to_string());
                parent_sibling = value.prev_named_sibling();
            }
        }
    }
    parts.reverse();
    parts.join("\n")
}

pub(super) fn compact_member_text(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

pub(super) fn first_string_child(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let value = node
        .named_children(&mut cursor)
        .find_map(|child| string_literal_value(child, source));
    value
}

pub(super) fn string_literal_value(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(node.kind(), "string" | "string_fragment") {
        return None;
    }
    let text = node_text(node, source).trim();
    Some(
        text.trim_matches('"')
            .trim_matches('\'')
            .trim_matches('`')
            .to_string(),
    )
}
