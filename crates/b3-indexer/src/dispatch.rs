use tree_sitter::{Node, Parser, Point};

use b3_core::{
    ContractError, ContractResult, EdgeConfidence, EdgeId, EdgeKind, EdgeProvenance,
    GraphEdgeMetadata, LanguageBackendMetadata, NodeKind, SymbolId,
};

use crate::{
    backend_languages, config_files, csharp, data_files, dotnet_desktop, go, infrastructure,
    language_from_path, stable_id, systems_languages, to_contract_error, web, web_files,
    ExtractedRelationship, ExtractedSymbol, ParseInput, ParsedFile, TreeSitterParser,
};

#[derive(Debug, Clone, Default)]
pub struct NoopTreeSitterParser;

impl TreeSitterParser for NoopTreeSitterParser {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        let _tree_sitter_anchor = std::mem::size_of::<tree_sitter::Parser>();
        Ok(ParsedFile {
            file_id: input.file_id,
            language: language_from_path(&input.path),
            symbols: Vec::new(),
            relationships: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct RustLanguagePack;

impl RustLanguagePack {
    pub fn backend_metadata() -> LanguageBackendMetadata {
        b3_core::rust_tree_sitter_backend_metadata()
    }
}

impl TreeSitterParser for RustLanguagePack {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        if language_from_path(&input.path).as_deref() != Some("rs") {
            return NoopTreeSitterParser.parse(input);
        }

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(to_contract_error)?;
        let tree = parser
            .parse(&input.source, None)
            .ok_or_else(|| ContractError::new("tree-sitter rust parse failed"))?;

        let root = tree.root_node();
        let mut symbols = Vec::new();
        collect_rust_symbols(root, &input, &mut symbols);
        let relationships = collect_rust_relationships(root, &input, &symbols);

        Ok(ParsedFile {
            file_id: input.file_id,
            language: Some("rust".to_string()),
            symbols,
            relationships,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct WebLanguagePack;

impl TreeSitterParser for WebLanguagePack {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        web::parse(input)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DefaultLanguagePack;

impl TreeSitterParser for DefaultLanguagePack {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        if infrastructure::is_infrastructure_file(&input.path, &input.source) {
            return infrastructure::parse(input);
        }
        if dotnet_desktop::is_xaml_file(&input.path) {
            return dotnet_desktop::parse(input);
        }

        match language_from_path(&input.path).as_deref() {
            Some("rs") => RustLanguagePack.parse(input),
            Some("javascript" | "jsx" | "typescript" | "tsx") => WebLanguagePack.parse(input),
            Some("csharp" | "csproj") => {
                let include_desktop =
                    dotnet_desktop::is_dotnet_desktop_file(&input.path, &input.source);
                let desktop_input = include_desktop.then(|| input.clone());
                let mut parsed = csharp::parse(input)?;
                if let Some(desktop_input) = desktop_input {
                    let desktop = dotnet_desktop::parse(desktop_input)?;
                    parsed.symbols.extend(desktop.symbols);
                    parsed.relationships.extend(desktop.relationships);
                }
                Ok(parsed)
            }
            Some("go" | "gomod") => go::parse(input),
            Some(
                "python" | "python_project" | "java" | "java_project" | "kotlin" | "kotlin_project"
                | "php" | "php_project" | "ruby" | "ruby_project",
            ) => backend_languages::parse(input),
            Some(
                "c" | "c_header" | "cpp" | "cpp_header" | "cmake" | "makefile" | "compile_commands"
                | "swift" | "swift_project" | "objective_c" | "objective_cpp" | "dart"
                | "dart_project",
            ) => systems_languages::parse(input),
            Some("yaml" | "json" | "toml" | "xml" | "env") => config_files::parse(input),
            Some("html" | "css" | "scss") => web_files::parse(input),
            Some("ksql" | "sql") => data_files::parse(input),
            _ => NoopTreeSitterParser.parse(input),
        }
    }
}

fn collect_rust_symbols(node: Node<'_>, input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    if let Some((name, kind)) = rust_symbol_name_and_kind(node, &input.source) {
        let start = node.start_position();
        let end = node.end_position();
        symbols.push(ExtractedSymbol {
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
            visibility: rust_visibility(node, &input.source),
        });
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_rust_symbols(child, input, symbols);
    }
}

fn collect_rust_relationships(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedRelationship> {
    let mut relationships = Vec::new();
    collect_contains_relationships(symbols, &mut relationships);
    collect_import_relationships(symbols, &mut relationships);
    collect_call_relationships(root, input, symbols, &mut relationships);
    // Phase 4.1 policy: do not emit REFERENCES edges yet. Rust reference
    // extraction needs name resolution to avoid noisy or misleading edges, so
    // it is deferred until a later graph-analysis phase.
    relationships
}

pub(crate) fn collect_contains_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    for child in symbols {
        let parent = symbols
            .iter()
            .filter(|candidate| candidate.id != child.id)
            .filter(|candidate| {
                candidate.start_byte <= child.start_byte && candidate.end_byte >= child.end_byte
            })
            .min_by_key(|candidate| candidate.end_byte - candidate.start_byte);

        if let Some(parent) = parent {
            relationships.push(index_edge(
                &parent.id,
                &child.id,
                EdgeKind::Contains,
                EdgeProvenance::Ast,
                10_000,
            ));
        }
    }
}

pub(crate) fn collect_import_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    let containers: Vec<&ExtractedSymbol> = symbols
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.kind,
                NodeKind::Module
                    | NodeKind::Function
                    | NodeKind::Method
                    | NodeKind::Struct
                    | NodeKind::Enum
                    | NodeKind::Interface
            )
        })
        .collect();

    for import in symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Package)
    {
        let owner = containers
            .iter()
            .copied()
            .filter(|candidate| {
                candidate.start_byte <= import.start_byte && candidate.end_byte >= import.end_byte
            })
            .min_by_key(|candidate| candidate.end_byte - candidate.start_byte);

        if let Some(owner) = owner {
            relationships.push(index_edge(
                &owner.id,
                &import.id,
                EdgeKind::Imports,
                EdgeProvenance::ImportAnalysis,
                9_000,
            ));
        }
    }
}

fn collect_call_relationships(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    if node.kind() == "call_expression" {
        if let Some(function) = node.child_by_field_name("function") {
            let call_name = rust_call_name(function, &input.source);
            if let Some(call_name) = call_name {
                let caller = containing_callable(node, symbols);
                let callee = symbols.iter().find(|symbol| {
                    matches!(symbol.kind, NodeKind::Function | NodeKind::Method)
                        && symbol.name == call_name
                });

                if let (Some(caller), Some(callee)) = (caller, callee) {
                    if caller.id != callee.id {
                        relationships.push(index_edge(
                            &caller.id,
                            &callee.id,
                            EdgeKind::Calls,
                            EdgeProvenance::Ast,
                            8_500,
                        ));
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_call_relationships(child, input, symbols, relationships);
    }
}

fn rust_symbol_name_and_kind(node: Node<'_>, source: &str) -> Option<(String, NodeKind)> {
    let kind = match node.kind() {
        "mod_item" => NodeKind::Module,
        "struct_item" => NodeKind::Struct,
        "enum_item" => NodeKind::Enum,
        "trait_item" => NodeKind::Interface,
        "impl_item" => NodeKind::Class,
        "function_item" => {
            if has_parent_kind(node, "impl_item") || has_parent_kind(node, "trait_item") {
                NodeKind::Method
            } else if has_test_attribute(node, source) {
                NodeKind::Test
            } else {
                NodeKind::Function
            }
        }
        "use_declaration" => NodeKind::Package,
        _ => return None,
    };

    let name = if node.kind() == "impl_item" {
        rust_impl_name(node, source)
    } else if node.kind() == "use_declaration" {
        Some(
            node_text(node, source)
                .trim_end_matches(';')
                .trim()
                .to_string(),
        )
    } else {
        node.child_by_field_name("name")
            .map(|name| node_text(name, source).to_string())
    }?;

    Some((name, kind))
}

fn rust_impl_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("type")
        .map(|value| format!("impl {}", node_text(value, source)))
}

fn rust_visibility(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let visibility = node
        .children(&mut cursor)
        .find(|child| child.kind() == "visibility_modifier")
        .map(|child| node_text(child, source).to_string());
    visibility
}

fn rust_call_name(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(node, source).to_string()),
        "field_expression" => node
            .child_by_field_name("field")
            .map(|field| node_text(field, source).to_string()),
        _ => None,
    }
}

fn containing_callable<'a>(
    node: Node<'_>,
    symbols: &'a [ExtractedSymbol],
) -> Option<&'a ExtractedSymbol> {
    symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, NodeKind::Function | NodeKind::Method))
        .filter(|symbol| {
            symbol.start_byte <= node.start_byte() && symbol.end_byte >= node.end_byte()
        })
        .min_by_key(|symbol| symbol.end_byte - symbol.start_byte)
}

pub(crate) fn index_edge(
    from_symbol: &SymbolId,
    to_symbol: &SymbolId,
    kind: EdgeKind,
    provenance: EdgeProvenance,
    confidence_bps: u16,
) -> ExtractedRelationship {
    ExtractedRelationship {
        id: EdgeId::new(stable_id(
            "edge",
            &format!(
                "{}:{}:{kind:?}:{}",
                from_symbol.as_str(),
                to_symbol.as_str(),
                confidence_bps
            ),
        )),
        from_symbol: from_symbol.clone(),
        to_symbol: to_symbol.clone(),
        kind,
        metadata: GraphEdgeMetadata {
            confidence: EdgeConfidence::from_basis_points(confidence_bps),
            provenance,
            created_at_unix_ms: 0,
            updated_at_unix_ms: 0,
        },
    }
}

pub(crate) fn has_parent_kind(node: Node<'_>, kind: &str) -> bool {
    let mut parent = node.parent();
    while let Some(value) = parent {
        if value.kind() == kind {
            return true;
        }
        parent = value.parent();
    }
    false
}

fn has_test_attribute(node: Node<'_>, source: &str) -> bool {
    let mut previous = node.prev_named_sibling();
    while let Some(sibling) = previous {
        if sibling.kind() != "attribute_item" {
            return false;
        }
        if node_text(sibling, source).contains("#[test]") {
            return true;
        }
        previous = sibling.prev_named_sibling();
    }
    false
}

pub(crate) fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or_default()
}

pub(crate) fn one_based_row(point: Point) -> usize {
    point.row + 1
}
