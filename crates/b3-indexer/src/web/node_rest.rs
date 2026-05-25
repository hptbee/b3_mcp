use super::*;

pub(super) fn collect_node_rest_routes(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut routes = Vec::new();
    collect_call_routes(root, input, &mut routes);
    collect_nest_routes(root, input, symbols, &mut routes);
    routes
}

fn collect_call_routes(node: Node<'_>, input: &ParseInput, routes: &mut Vec<ExtractedSymbol>) {
    if node.kind() == "call_expression" {
        if let Some(metadata) = express_or_fastify_route_metadata(node, input) {
            routes.push(route_symbol(input, node, &metadata));
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_call_routes(child, input, routes);
    }
}

fn express_or_fastify_route_metadata(node: Node<'_>, input: &ParseInput) -> Option<RouteMetadata> {
    let function = node.child_by_field_name("function")?;
    let function_text = compact_member_text(node_text(function, &input.source));
    if function_text.ends_with(".route") && function_text.contains("fastify") {
        return fastify_route_object_metadata(node, input);
    }
    let route_like_receiver = function_text.starts_with("app.")
        || function_text.starts_with("router.")
        || function_text.starts_with("fastify.")
        || function_text.contains(".route(");
    if !route_like_receiver {
        return None;
    }

    let method = route_method_from_function_text(&function_text)?;
    let (framework, source_kind, path) =
        if let Some(route_path) = route_path_from_chained_route_call(&function_text) {
            ("express", "ExpressRouterCall", route_path)
        } else {
            let arguments = node.child_by_field_name("arguments")?;
            let path = first_string_child(arguments, &input.source)?;
            let framework = if function_text.contains("fastify") {
                "fastify"
            } else {
                "express"
            };
            let source_kind = if framework == "fastify" {
                "FastifyShorthandCall"
            } else if function_text.contains("router.") {
                "ExpressRouterCall"
            } else {
                "ExpressCall"
            };
            (framework, source_kind, path)
        };
    let handler_name = node
        .child_by_field_name("arguments")
        .and_then(|arguments| nth_argument_name(arguments, &input.source, 1));
    let confidence = if source_kind == "ExpressRouterCall" {
        9_000
    } else {
        9_500
    };
    Some(RouteMetadata {
        framework: framework.to_string(),
        route_kind: "api".to_string(),
        method,
        path: normalize_route_path("", &path),
        file_path: input.path.to_string_lossy().replace('\\', "/"),
        symbol_id: None,
        handler_name: handler_name.clone(),
        class_name: None,
        function_name: handler_name,
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence,
        source_kind: source_kind.to_string(),
    })
}

fn fastify_route_object_metadata(node: Node<'_>, input: &ParseInput) -> Option<RouteMetadata> {
    let arguments = node.child_by_field_name("arguments")?;
    let object = first_child_kind(arguments, "object")?;
    let object_text = node_text(object, &input.source);
    let method = object_property_string(object_text, "method")
        .map(|value| value.to_ascii_uppercase())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let path = object_property_string(object_text, "url")
        .or_else(|| object_property_string(object_text, "path"))?;
    let handler_name = object_property_identifier(object_text, "handler");
    Some(RouteMetadata {
        framework: "fastify".to_string(),
        route_kind: "api".to_string(),
        method,
        path: normalize_route_path("", &path),
        file_path: input.path.to_string_lossy().replace('\\', "/"),
        symbol_id: None,
        handler_name: handler_name.clone(),
        class_name: None,
        function_name: handler_name,
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_000,
        source_kind: "FastifyRouteCall".to_string(),
    })
}

fn collect_nest_routes(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    routes: &mut Vec<ExtractedSymbol>,
) {
    if node.kind() == "class_declaration" {
        collect_nest_class_routes(node, input, symbols, routes);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nest_routes(child, input, symbols, routes);
    }
}

fn collect_nest_class_routes(
    class_node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    routes: &mut Vec<ExtractedSymbol>,
) {
    let class_text = node_text(class_node, &input.source);
    let leading_text = leading_decorator_text(class_node, &input.source);
    let Some(controller_path) = decorator_argument(&leading_text, "Controller")
        .or_else(|| decorator_argument(class_text, "Controller"))
    else {
        return;
    };
    let class_name = class_node
        .child_by_field_name("name")
        .map(|name| node_text(name, &input.source).to_string());
    let mut cursor = class_node.walk();
    for child in class_node.named_children(&mut cursor) {
        collect_nest_method_routes(
            child,
            input,
            symbols,
            &controller_path,
            class_name.as_deref(),
            routes,
        );
    }
}

fn collect_nest_method_routes(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    controller_path: &str,
    class_name: Option<&str>,
    routes: &mut Vec<ExtractedSymbol>,
) {
    if matches!(node.kind(), "method_definition" | "method_signature") {
        let text = format!(
            "{}\n{}",
            leading_decorator_text(node, &input.source),
            node_text(node, &input.source)
        );
        if let Some((method, method_path)) = nest_method_decorator(&text) {
            let function_name = node
                .child_by_field_name("name")
                .map(|name| node_text(name, &input.source).to_string());
            let symbol_id = function_name.as_ref().and_then(|name| {
                symbols
                    .iter()
                    .find(|symbol| {
                        symbol.kind == NodeKind::Method
                            && &symbol.name == name
                            && symbol.start_byte <= node.start_byte()
                            && symbol.end_byte >= node.end_byte()
                    })
                    .map(|symbol| symbol.id.clone())
            });
            let metadata = RouteMetadata {
                framework: "nestjs".to_string(),
                route_kind: "api".to_string(),
                method,
                path: normalize_route_path(controller_path, &method_path),
                file_path: input.path.to_string_lossy().replace('\\', "/"),
                symbol_id,
                handler_name: function_name.clone(),
                class_name: class_name.map(str::to_string),
                function_name,
                line_start: one_based_row(node.start_position()),
                line_end: one_based_row(node.end_position()),
                confidence: 9_500,
                source_kind: "NestMethodDecorator".to_string(),
            };
            routes.push(route_symbol(input, node, &metadata));
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nest_method_routes(child, input, symbols, controller_path, class_name, routes);
    }
}

fn route_method_from_function_text(function_text: &str) -> Option<String> {
    for (suffix, method) in [
        (".get", "GET"),
        (".post", "POST"),
        (".put", "PUT"),
        (".patch", "PATCH"),
        (".delete", "DELETE"),
        (".options", "OPTIONS"),
        (".head", "HEAD"),
        (".all", "ALL"),
        (".use", "ALL"),
    ] {
        if function_text.ends_with(suffix) {
            return Some(method.to_string());
        }
    }
    None
}

fn route_path_from_chained_route_call(function_text: &str) -> Option<String> {
    let route_start = function_text.find(".route(")?;
    let after_route = &function_text[route_start + ".route(".len()..];
    let quote = after_route
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_route.split_once(quote)?.1;
    let path = after_quote.split_once(quote)?.0;
    Some(path.to_string())
}

fn nth_argument_name(arguments: Node<'_>, source: &str, index: usize) -> Option<String> {
    let mut cursor = arguments.walk();
    let value = arguments
        .named_children(&mut cursor)
        .filter(|child| child.kind() != "comment")
        .nth(index)
        .and_then(|node| match node.kind() {
            "identifier" => Some(node_text(node, source).to_string()),
            "member_expression" => Some(node_text(node, source).to_string()),
            "arrow_function" | "function" | "function_expression" => None,
            _ => Some(node_text(node, source).trim().to_string()).filter(|value| !value.is_empty()),
        });
    value
}

fn nest_method_decorator(text: &str) -> Option<(String, String)> {
    for (decorator, method) in [
        ("Get", "GET"),
        ("Post", "POST"),
        ("Put", "PUT"),
        ("Patch", "PATCH"),
        ("Delete", "DELETE"),
        ("Options", "OPTIONS"),
        ("Head", "HEAD"),
        ("All", "ALL"),
    ] {
        let needle = format!("@{decorator}");
        if text.contains(&needle) {
            return Some((
                method.to_string(),
                decorator_argument(text, decorator).unwrap_or_default(),
            ));
        }
    }
    None
}
