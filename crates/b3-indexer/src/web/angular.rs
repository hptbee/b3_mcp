use super::routes::route_metadata_value;
use super::*;

const ANGULAR_METADATA_PREFIX: &str = "angular.";

#[derive(Debug, Clone)]
struct AngularMetadata {
    kind: &'static str,
    source_kind: &'static str,
    class_name: String,
    selector: Option<String>,
    template_url: Option<String>,
    style_urls: Vec<String>,
    inline_template_present: bool,
    standalone: Option<bool>,
    imports: Vec<String>,
    providers: Vec<String>,
    provided_in: Option<String>,
    dependencies: Vec<String>,
    declarations: Vec<String>,
    exports: Vec<String>,
    bootstrap: Vec<String>,
    pipe_name: Option<String>,
    line_start: usize,
    line_end: usize,
    confidence: u16,
}

pub(super) fn annotate_angular_symbols(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &mut [ExtractedSymbol],
) {
    let mut candidates = Vec::new();
    collect_angular_candidates(root, input, &mut candidates);
    for (class_name, node, metadata) in candidates {
        if let Some(symbol) = symbols.iter_mut().find(|symbol| {
            symbol.name == class_name
                && symbol.start_byte <= node.start_byte()
                && symbol.end_byte >= node.end_byte()
                && symbol.kind == NodeKind::Class
        }) {
            let mut visibility = symbol.visibility.take();
            if metadata.kind == "component" {
                visibility = merge_visibility(
                    visibility,
                    encode_component_metadata(&ComponentMetadata {
                        framework: "angular".to_string(),
                        export_kind: export_kind_for_node(node, &input.source),
                        component_kind: "component".to_string(),
                        props_type_name: None,
                        hooks: Vec::new(),
                        usages: Vec::new(),
                        line_start: metadata.line_start,
                        line_end: metadata.line_end,
                        confidence: metadata.confidence,
                        source_kind: "AngularComponent".to_string(),
                    }),
                );
            }
            symbol.visibility = merge_visibility(visibility, encode_angular_metadata(&metadata));
        }
    }
}

pub(super) fn collect_angular_routes(
    root: Node<'_>,
    input: &ParseInput,
    _symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !looks_like_angular_router_file(&input.source) {
        return Vec::new();
    }
    let mut routes = Vec::new();
    collect_angular_route_objects(root, input, &mut routes);
    routes
}

pub(super) fn collect_angular_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    for route in symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
        .filter(|symbol| {
            route_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "framework",
            )
            .as_deref()
                == Some("angular")
        })
    {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        let Some(component) = route_metadata_value(metadata, "class")
            .or_else(|| route_metadata_value(metadata, "handler"))
        else {
            continue;
        };
        if let Some(target) = symbols.iter().find(|symbol| {
            symbol.name == component && symbol.kind == NodeKind::Class && symbol.id != route.id
        }) {
            relationships.push(index_edge(
                &route.id,
                &target.id,
                EdgeKind::References,
                EdgeProvenance::Ast,
                8_000,
            ));
        }
    }

    for service in symbols.iter().filter(|symbol| {
        angular_metadata_value(symbol.visibility.as_deref().unwrap_or_default(), "kind").as_deref()
            == Some("service")
    }) {
        let metadata = service.visibility.as_deref().unwrap_or_default();
        let Some(dependencies) = angular_metadata_value(metadata, "dependencies") else {
            continue;
        };
        for dependency in dependencies.split(',').filter(|value| !value.is_empty()) {
            if let Some(target) = symbols.iter().find(|symbol| {
                symbol.name == dependency
                    && symbol.id != service.id
                    && matches!(
                        symbol.kind,
                        NodeKind::Class
                            | NodeKind::Interface
                            | NodeKind::Variable
                            | NodeKind::Package
                    )
            }) {
                relationships.push(index_edge(
                    &service.id,
                    &target.id,
                    EdgeKind::References,
                    EdgeProvenance::Ast,
                    7_500,
                ));
            }
        }
    }
}

pub fn detect_angular_config_path(path: &Path) -> Option<DetectedTechnology> {
    let file_name = path.file_name()?.to_str()?;
    if !matches!(file_name, "angular.json" | "tsconfig.app.json") {
        return None;
    }
    Some(DetectedTechnology {
        id: "angular".to_string(),
        name: "Angular".to_string(),
        kind: TechnologyKind::WebFrontend,
        support_level: TechnologySupportLevel::Basic,
        capabilities: vec![
            TechnologyCapability::DetectPackage,
            TechnologyCapability::DetectImport,
            TechnologyCapability::ExtractRoutes,
            TechnologyCapability::ExtractComponents,
        ],
        source: format!("config:{}", path.to_string_lossy().replace('\\', "/")),
    })
}

fn collect_angular_candidates<'a>(
    node: Node<'a>,
    input: &ParseInput,
    candidates: &mut Vec<(String, Node<'a>, AngularMetadata)>,
) {
    if node.kind() == "class_declaration" {
        if let Some((class_name, metadata)) = angular_class_metadata(node, input) {
            candidates.push((class_name, node, metadata));
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_angular_candidates(child, input, candidates);
    }
}

fn angular_class_metadata(node: Node<'_>, input: &ParseInput) -> Option<(String, AngularMetadata)> {
    let name = node.child_by_field_name("name")?;
    let class_name = node_text(name, &input.source).to_string();
    let decorators = angular_decorator_text(node, &input.source);
    if decorators.is_empty() {
        return None;
    }
    let line_start = one_based_row(node.start_position());
    let line_end = one_based_row(node.end_position());

    if let Some(object) = decorator_object_literal(&decorators, "Component") {
        return Some((
            class_name.clone(),
            AngularMetadata {
                kind: "component",
                source_kind: "AngularComponent",
                class_name,
                selector: object_property_string(&object, "selector"),
                template_url: object_property_string(&object, "templateUrl"),
                style_urls: angular_style_urls(&object),
                inline_template_present: has_object_property(&object, "template"),
                standalone: object_property_bool(&object, "standalone"),
                imports: object_property_array_identifiers(&object, "imports"),
                providers: object_property_array_identifiers(&object, "providers"),
                provided_in: None,
                dependencies: Vec::new(),
                declarations: Vec::new(),
                exports: Vec::new(),
                bootstrap: Vec::new(),
                pipe_name: None,
                line_start,
                line_end,
                confidence: 9_000,
            },
        ));
    }

    if let Some(object) = decorator_object_literal(&decorators, "Injectable") {
        return Some((
            class_name.clone(),
            AngularMetadata {
                kind: "service",
                source_kind: "AngularService",
                class_name,
                selector: None,
                template_url: None,
                style_urls: Vec::new(),
                inline_template_present: false,
                standalone: None,
                imports: Vec::new(),
                providers: Vec::new(),
                provided_in: object_property_string(&object, "providedIn")
                    .or_else(|| object_property_identifier(&object, "providedIn")),
                dependencies: constructor_dependencies(node_text(node, &input.source)),
                declarations: Vec::new(),
                exports: Vec::new(),
                bootstrap: Vec::new(),
                pipe_name: None,
                line_start,
                line_end,
                confidence: 8_800,
            },
        ));
    }

    if let Some(object) = decorator_object_literal(&decorators, "NgModule") {
        return Some((
            class_name.clone(),
            AngularMetadata {
                kind: "module",
                source_kind: "AngularModule",
                class_name,
                selector: None,
                template_url: None,
                style_urls: Vec::new(),
                inline_template_present: false,
                standalone: None,
                imports: object_property_array_identifiers(&object, "imports"),
                providers: object_property_array_identifiers(&object, "providers"),
                provided_in: None,
                dependencies: Vec::new(),
                declarations: object_property_array_identifiers(&object, "declarations"),
                exports: object_property_array_identifiers(&object, "exports"),
                bootstrap: object_property_array_identifiers(&object, "bootstrap"),
                pipe_name: None,
                line_start,
                line_end,
                confidence: 8_800,
            },
        ));
    }

    if let Some(object) = decorator_object_literal(&decorators, "Directive") {
        return Some((
            class_name.clone(),
            AngularMetadata {
                kind: "directive",
                source_kind: "AngularDirective",
                class_name,
                selector: object_property_string(&object, "selector"),
                template_url: None,
                style_urls: Vec::new(),
                inline_template_present: false,
                standalone: object_property_bool(&object, "standalone"),
                imports: Vec::new(),
                providers: Vec::new(),
                provided_in: None,
                dependencies: Vec::new(),
                declarations: Vec::new(),
                exports: Vec::new(),
                bootstrap: Vec::new(),
                pipe_name: None,
                line_start,
                line_end,
                confidence: 8_500,
            },
        ));
    }

    if let Some(object) = decorator_object_literal(&decorators, "Pipe") {
        return Some((
            class_name.clone(),
            AngularMetadata {
                kind: "pipe",
                source_kind: "AngularPipe",
                class_name,
                selector: None,
                template_url: None,
                style_urls: Vec::new(),
                inline_template_present: false,
                standalone: object_property_bool(&object, "standalone"),
                imports: Vec::new(),
                providers: Vec::new(),
                provided_in: None,
                dependencies: Vec::new(),
                declarations: Vec::new(),
                exports: Vec::new(),
                bootstrap: Vec::new(),
                pipe_name: object_property_string(&object, "name"),
                line_start,
                line_end,
                confidence: 8_500,
            },
        ));
    }

    None
}

fn collect_angular_route_objects(
    node: Node<'_>,
    input: &ParseInput,
    routes: &mut Vec<ExtractedSymbol>,
) {
    if node.kind() == "object" {
        if let Some(route) = angular_route_symbol(node, input) {
            routes.push(route);
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_angular_route_objects(child, input, routes);
    }
}

fn angular_route_symbol(node: Node<'_>, input: &ParseInput) -> Option<ExtractedSymbol> {
    let object_text = node_text(node, &input.source);
    let path = object_property_string(object_text, "path")?;
    let has_route_field = [
        "component",
        "loadChildren",
        "loadComponent",
        "redirectTo",
        "children",
    ]
    .iter()
    .any(|key| has_object_property(object_text, key));
    if !has_route_field {
        return None;
    }
    let route_path = normalize_route_path("", &path);
    let component = object_property_identifier(object_text, "component");
    let load_children = object_property_static_reference(object_text, "loadChildren");
    let load_component = object_property_static_reference(object_text, "loadComponent");
    let redirect_to = object_property_string(object_text, "redirectTo");
    let source_kind = if redirect_to.is_some() {
        "AngularRedirectRoute"
    } else if load_children.is_some() {
        "AngularLazyRoute"
    } else if load_component.is_some() {
        "AngularLoadComponentRoute"
    } else {
        "AngularRoute"
    };
    let line_start = one_based_row(node.start_position());
    let line_end = one_based_row(node.end_position());
    let mut symbol = ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:angular-route:{}:{}",
                input.file_id.as_str(),
                route_path,
                node.start_byte()
            ),
        )),
        file_id: input.file_id.clone(),
        name: format!("GET {route_path}"),
        kind: NodeKind::Route,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_line: line_start,
        start_column: node.start_position().column,
        end_line: line_end,
        end_column: node.end_position().column,
        visibility: Some(encode_route_metadata(&RouteMetadata {
            framework: "angular".to_string(),
            route_kind: "route".to_string(),
            method: "GET".to_string(),
            path: route_path,
            file_path: input.path.to_string_lossy().replace('\\', "/"),
            symbol_id: None,
            handler_name: component.clone().or_else(|| load_component.clone()),
            class_name: component,
            function_name: load_component.or(load_children).or(redirect_to),
            line_start,
            line_end,
            confidence: 8_000,
            source_kind: source_kind.to_string(),
        })),
    };
    if has_object_property(object_text, "children") {
        symbol.visibility = merge_visibility(
            symbol.visibility.take(),
            "angular.route.children_present=true".to_string(),
        );
    }
    Some(symbol)
}

fn angular_decorator_text(node: Node<'_>, source: &str) -> String {
    let leading = leading_decorator_text(node, source);
    let class_text = node_text(node, source);
    match (leading.is_empty(), class_text.contains('@')) {
        (true, true) => class_text.to_string(),
        (false, true) => format!("{leading}\n{class_text}"),
        (false, false) => leading,
        (true, false) => String::new(),
    }
}

fn decorator_object_literal(text: &str, decorator_name: &str) -> Option<String> {
    let needle = format!("@{decorator_name}");
    let position = text.find(&needle)?;
    let after = &text[position + needle.len()..];
    let open_paren = after.find('(')?;
    let after_paren = &after[open_paren + 1..];
    let object_start = after_paren.find('{')?;
    let object = &after_paren[object_start..];
    balanced_slice(object, '{', '}')
}

fn balanced_slice(text: &str, open: char, close: char) -> Option<String> {
    let mut depth = 0usize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    for (index, character) in text.char_indices() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == quote {
                in_string = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            in_string = Some(character);
            continue;
        }
        if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(text[..=index].to_string());
            }
        }
    }
    None
}

fn has_object_property(object_text: &str, key: &str) -> bool {
    object_text.contains(&format!("{key}:")) || object_text.contains(&format!("{key} :"))
}

fn object_property_bool(object_text: &str, key: &str) -> Option<bool> {
    let value = raw_object_property_value(object_text, key)?;
    if value.trim_start().starts_with("true") {
        Some(true)
    } else if value.trim_start().starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn raw_object_property_value<'a>(object_text: &'a str, key: &str) -> Option<&'a str> {
    let key_position = object_text.find(key)?;
    let after_key = &object_text[key_position + key.len()..];
    let colon_position = after_key.find(':')?;
    Some(after_key[colon_position + 1..].trim_start())
}

fn angular_style_urls(object_text: &str) -> Vec<String> {
    let mut values = object_property_array_strings(object_text, "styleUrls");
    if let Some(style_url) = object_property_string(object_text, "styleUrl") {
        values.push(style_url);
    }
    values
}

fn object_property_array_strings(object_text: &str, key: &str) -> Vec<String> {
    let Some(array_text) = object_property_array_text(object_text, key) else {
        return Vec::new();
    };
    split_top_level_commas(array_text.trim_matches(['[', ']']))
        .into_iter()
        .filter_map(|value| quoted_value(value.trim()))
        .collect()
}

fn object_property_array_identifiers(object_text: &str, key: &str) -> Vec<String> {
    let Some(array_text) = object_property_array_text(object_text, key) else {
        return Vec::new();
    };
    split_top_level_commas(array_text.trim_matches(['[', ']']))
        .into_iter()
        .filter_map(|value| compact_angular_reference(value.trim()))
        .collect()
}

fn object_property_array_text(object_text: &str, key: &str) -> Option<String> {
    let value = raw_object_property_value(object_text, key)?;
    let start = value.find('[')?;
    balanced_slice(&value[start..], '[', ']')
}

fn object_property_static_reference(object_text: &str, key: &str) -> Option<String> {
    let value = raw_object_property_value(object_text, key)?;
    if let Some(import_path) = import_string(value) {
        return Some(import_path);
    }
    compact_angular_reference(value)
}

fn compact_angular_reference(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with("()") || trimmed.starts_with("async") {
        return import_string(trimmed);
    }
    let token = trimmed
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '.')
        })
        .find(|token| !token.is_empty())?;
    Some(token.to_string())
}

fn import_string(value: &str) -> Option<String> {
    let import_position = value.find("import(")?;
    let after_import = &value[import_position + "import(".len()..];
    let quote = after_import
        .chars()
        .find(|character| *character == '"' || *character == '\'')?;
    let after_quote = after_import.split_once(quote)?.1;
    Some(after_quote.split_once(quote)?.0.to_string())
}

fn quoted_value(value: &str) -> Option<String> {
    let quote = value
        .chars()
        .find(|character| *character == '"' || *character == '\'' || *character == '`')?;
    let after_quote = value.split_once(quote)?.1;
    Some(after_quote.split_once(quote)?.0.to_string())
}

fn split_top_level_commas(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == quote {
                in_string = None;
            }
            continue;
        }
        match character {
            '"' | '\'' | '`' => in_string = Some(character),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if bracket_depth == 0 && paren_depth == 0 && brace_depth == 0 => {
                parts.push(value[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    if start <= value.len() {
        let tail = value[start..].trim();
        if !tail.is_empty() {
            parts.push(tail);
        }
    }
    parts
}

fn constructor_dependencies(class_text: &str) -> Vec<String> {
    let Some(constructor_position) = class_text.find("constructor") else {
        return Vec::new();
    };
    let after_constructor = &class_text[constructor_position + "constructor".len()..];
    let Some(open) = after_constructor.find('(') else {
        return Vec::new();
    };
    let Some(parameters) = balanced_slice(&after_constructor[open..], '(', ')') else {
        return Vec::new();
    };
    split_top_level_commas(parameters.trim_matches(['(', ')']))
        .into_iter()
        .filter_map(constructor_dependency_type)
        .collect()
}

fn constructor_dependency_type(parameter: &str) -> Option<String> {
    let after_colon = parameter.split_once(':')?.1.trim();
    let token = after_colon
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '.')
        })
        .find(|token| !token.is_empty())?;
    Some(token.to_string())
}

fn looks_like_angular_router_file(source: &str) -> bool {
    source.contains("@angular/router")
        || source.contains("RouterModule.forRoot")
        || source.contains("RouterModule.forChild")
        || source.contains(": Routes")
        || source.contains("Routes =")
}

fn encode_angular_metadata(metadata: &AngularMetadata) -> String {
    let mut parts = vec![
        angular_pair("framework", Some("angular")),
        angular_pair("kind", Some(metadata.kind)),
        angular_pair("source", Some(metadata.source_kind)),
        angular_pair("class", Some(metadata.class_name.as_str())),
        angular_pair("selector", metadata.selector.as_deref()),
        angular_pair("template_url", metadata.template_url.as_deref()),
        angular_pair("provided_in", metadata.provided_in.as_deref()),
        angular_pair("pipe_name", metadata.pipe_name.as_deref()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}style_urls={}",
        metadata.style_urls.join(",")
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}inline_template_present={}",
        metadata.inline_template_present
    ));
    if let Some(standalone) = metadata.standalone {
        parts.push(format!("{ANGULAR_METADATA_PREFIX}standalone={standalone}"));
    }
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}imports={}",
        metadata.imports.join(",")
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}providers={}",
        metadata.providers.join(",")
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}dependencies={}",
        metadata.dependencies.join(",")
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}declarations={}",
        metadata.declarations.join(",")
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}exports={}",
        metadata.exports.join(",")
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}bootstrap={}",
        metadata.bootstrap.join(",")
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}line_start={}",
        metadata.line_start
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}line_end={}",
        metadata.line_end
    ));
    parts.push(format!(
        "{ANGULAR_METADATA_PREFIX}confidence={}",
        metadata.confidence
    ));
    parts.join(";")
}

fn angular_pair(key: &str, value: Option<&str>) -> Option<String> {
    value.map(|value| {
        format!(
            "{ANGULAR_METADATA_PREFIX}{key}={}",
            escape_metadata_semicolon(value)
        )
    })
}

pub(crate) fn angular_metadata_value(metadata: &str, key: &str) -> Option<String> {
    prefixed_metadata_value_semicolon(metadata, "angular", key)
}
