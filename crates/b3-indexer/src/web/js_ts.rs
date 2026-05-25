use super::*;

pub(super) fn collect_web_symbols(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &mut Vec<ExtractedSymbol>,
) {
    if let Some((name, kind, visibility)) = web_symbol_name_kind_and_visibility(node, &input.source)
    {
        symbols.push(symbol_from_node(input, node, name, kind, visibility));
    }

    if let Some(import_name) = web_import_specifier(node, &input.source) {
        symbols.push(symbol_from_node(
            input,
            node,
            import_name,
            NodeKind::Package,
            None,
        ));
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_web_symbols(child, input, symbols);
    }
}

fn web_symbol_name_kind_and_visibility(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let exported =
        has_parent_kind(node, "export_statement") || has_parent_kind(node, "export_clause");
    let visibility = exported.then(|| "export".to_string());
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            node.child_by_field_name("name").map(|name| {
                (
                    node_text(name, source).to_string(),
                    NodeKind::Function,
                    visibility,
                )
            })
        }
        "class_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Class,
                visibility,
            )
        }),
        "method_definition" | "method_signature" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Method,
                visibility,
            )
        }),
        "interface_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Interface,
                visibility,
            )
        }),
        "type_alias_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Variable,
                visibility,
            )
        }),
        "enum_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Enum,
                visibility,
            )
        }),
        "variable_declarator" => web_variable_symbol(node, source, visibility),
        "export_statement" => web_default_export_symbol(node, source),
        "assignment_expression" => web_module_exports_symbol(node, source),
        _ => None,
    }
}

fn web_variable_symbol(
    node: Node<'_>,
    source: &str,
    visibility: Option<String>,
) -> Option<(String, NodeKind, Option<String>)> {
    let name = node.child_by_field_name("name")?;
    let value = node.child_by_field_name("value");
    let value_kind = value.map(|value| value.kind());
    let exported = visibility.is_some();
    let should_index = exported
        || matches!(
            value_kind,
            Some(
                "arrow_function"
                    | "function"
                    | "function_expression"
                    | "class"
                    | "class_expression"
            )
        );
    if !should_index {
        return None;
    }

    let kind = if matches!(value_kind, Some("class" | "class_expression")) {
        NodeKind::Class
    } else if matches!(
        value_kind,
        Some("arrow_function" | "function" | "function_expression")
    ) {
        NodeKind::Function
    } else {
        NodeKind::Variable
    };
    Some((node_text(name, source).to_string(), kind, visibility))
}

fn web_default_export_symbol(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let text = node_text(node, source).trim_start();
    if !text.starts_with("export default") {
        return None;
    }
    if node
        .named_child(0)
        .map(|child| matches!(child.kind(), "function_declaration" | "class_declaration"))
        .unwrap_or(false)
    {
        return None;
    }
    Some((
        "default".to_string(),
        NodeKind::Variable,
        Some("export default".to_string()),
    ))
}

fn web_module_exports_symbol(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let left = node.child_by_field_name("left")?;
    let text = node_text(left, source).replace(' ', "");
    if text == "module.exports"
        || text.starts_with("module.exports.")
        || text.starts_with("exports.")
    {
        Some((
            node_text(left, source).trim().to_string(),
            NodeKind::Variable,
            Some("commonjs export".to_string()),
        ))
    } else {
        None
    }
}

fn web_import_specifier(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "import_statement" => node
            .child_by_field_name("source")
            .and_then(|source_node| string_literal_value(source_node, source)),
        "call_expression" => {
            let function = node.child_by_field_name("function")?;
            if node_text(function, source) != "require" {
                return None;
            }
            let arguments = node.child_by_field_name("arguments")?;
            first_string_child(arguments, source)
        }
        _ => None,
    }
}

pub fn resolve_web_import_path(importer_path: &Path, specifier: &str) -> Option<PathBuf> {
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return None;
    }
    let base = importer_path.parent()?.join(specifier);
    web_import_candidates(&base)
        .into_iter()
        .find(|candidate| candidate.exists())
}

fn web_import_candidates(base: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if base.extension().is_some() {
        candidates.push(base.to_path_buf());
    } else {
        for extension in ["js", "jsx", "ts", "tsx"] {
            candidates.push(base.with_extension(extension));
        }
        for extension in ["js", "jsx", "ts", "tsx"] {
            candidates.push(base.join(format!("index.{extension}")));
        }
    }
    candidates
}

pub fn detect_package_json_technologies(source: &str) -> ContractResult<Vec<DetectedTechnology>> {
    let value = serde_json::from_str::<serde_json::Value>(source)
        .map_err(|error| ContractError::new(format!("invalid package.json: {error}")))?;
    let mut technologies = Vec::new();
    let dependencies = ["dependencies", "devDependencies", "peerDependencies"];
    for section in dependencies {
        let Some(object) = value.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for package_name in object.keys() {
            if let Some(technology) = package_technology(package_name, section) {
                if !technologies
                    .iter()
                    .any(|existing: &DetectedTechnology| existing.id == technology.id)
                {
                    technologies.push(technology);
                }
            }
        }
    }
    for technology in data_access::detect_package_json_data_access_technologies(source)? {
        if !technologies
            .iter()
            .any(|existing: &DetectedTechnology| existing.id == technology.id)
        {
            technologies.push(technology);
        }
    }
    for technology in realtime::detect_package_json_realtime_technologies(source)? {
        if !technologies
            .iter()
            .any(|existing: &DetectedTechnology| existing.id == technology.id)
        {
            technologies.push(technology);
        }
    }
    for technology in messaging::detect_package_json_messaging_technologies(source)? {
        if !technologies
            .iter()
            .any(|existing: &DetectedTechnology| existing.id == technology.id)
        {
            technologies.push(technology);
        }
    }
    Ok(technologies)
}

fn package_technology(package_name: &str, section: &str) -> Option<DetectedTechnology> {
    let (id, name, kind, support_level, capabilities) = match package_name {
        "express" => (
            "express",
            "Express",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "@nestjs/core" | "@nestjs/common" => (
            "nestjs",
            "NestJS",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "fastify" => (
            "fastify",
            "Fastify",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "typescript" | "ts-node" => (
            "typescript",
            "TypeScript",
            TechnologyKind::Language,
            TechnologySupportLevel::Basic,
            vec![TechnologyCapability::DetectPackage],
        ),
        "react" | "react-dom" | "@types/react" => (
            "react",
            "React",
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractComponents,
            ],
        ),
        "next" => (
            "nextjs",
            "Next.js",
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
                TechnologyCapability::ExtractComponents,
            ],
        ),
        "@angular/core"
        | "@angular/common"
        | "@angular/router"
        | "@angular/forms"
        | "@angular/platform-browser"
        | "@angular/cli" => (
            "angular",
            "Angular",
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
                TechnologyCapability::ExtractComponents,
            ],
        ),
        "vite" => (
            package_name,
            package_name,
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::DetectOnly,
            vec![TechnologyCapability::DetectPackage],
        ),
        name if name.starts_with("@fastify/") => (
            "fastify",
            "Fastify",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![TechnologyCapability::DetectPackage],
        ),
        _ => return None,
    };
    Some(DetectedTechnology {
        id: id.to_string(),
        name: name.to_string(),
        kind,
        support_level,
        capabilities,
        source: format!("package.json:{section}:{package_name}"),
    })
}
