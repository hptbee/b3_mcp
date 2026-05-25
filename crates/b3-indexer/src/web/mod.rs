use super::*;

mod angular;
mod components;
mod js_ts;
mod nextjs;
mod node_rest;
mod react;
mod routes;
mod tree_sitter_helpers;

#[cfg(test)]
pub(crate) use angular::angular_metadata_value;
pub use angular::detect_angular_config_path;
#[cfg(test)]
pub(crate) use components::component_metadata_value;
use components::{collect_component_relationships, encode_component_metadata, merge_visibility};
pub use js_ts::{detect_package_json_technologies, resolve_web_import_path};
pub use nextjs::detect_nextjs_config_path;
use react::export_kind_for_node;
#[cfg(test)]
pub(crate) use routes::route_metadata_value;
use routes::{
    collect_route_handler_relationships, encode_route_metadata, normalize_route_path, route_symbol,
};
use tree_sitter_helpers::{
    compact_member_text, decorator_argument, first_child_kind, first_string_child,
    leading_decorator_text, object_property_identifier, object_property_string,
    string_literal_value, symbol_from_node,
};

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let Some(language) = language_from_path(&input.path) else {
        return NoopTreeSitterParser.parse(input);
    };
    if !matches!(
        language.as_str(),
        "javascript" | "jsx" | "typescript" | "tsx"
    ) {
        return NoopTreeSitterParser.parse(input);
    }

    let mut parser = Parser::new();
    let tree_sitter_language = match language.as_str() {
        "javascript" | "jsx" => tree_sitter_javascript::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => unreachable!("language checked above"),
    };
    parser
        .set_language(&tree_sitter_language)
        .map_err(to_contract_error)?;
    let tree = parser
        .parse(&input.source, None)
        .ok_or_else(|| ContractError::new("tree-sitter web language parse failed"))?;

    let root = tree.root_node();
    let mut symbols = vec![module_symbol(&input)];
    js_ts::collect_web_symbols(root, &input, &mut symbols);
    react::annotate_react_components(root, &input, &mut symbols);
    angular::annotate_angular_symbols(root, &input, &mut symbols);
    let routes = node_rest::collect_node_rest_routes(root, &input, &symbols);
    symbols.extend(routes);
    let nextjs_routes = nextjs::collect_nextjs_routes(root, &input, &symbols);
    symbols.extend(nextjs_routes);
    let angular_routes = angular::collect_angular_routes(root, &input, &symbols);
    symbols.extend(angular_routes);
    let relationships = collect_web_relationships(&symbols);

    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some(language),
        symbols,
        relationships,
    })
}

fn module_symbol(input: &ParseInput) -> ExtractedSymbol {
    let end_line = input.source.lines().count().max(1);
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:module:{}", input.file_id.as_str(), input.path.display()),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("module")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line,
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: None,
    }
}

fn collect_web_relationships(symbols: &[ExtractedSymbol]) -> Vec<ExtractedRelationship> {
    let mut relationships = Vec::new();
    collect_contains_relationships(symbols, &mut relationships);
    collect_import_relationships(symbols, &mut relationships);
    collect_route_handler_relationships(symbols, &mut relationships);
    collect_component_relationships(symbols, &mut relationships);
    angular::collect_angular_relationships(symbols, &mut relationships);
    relationships
}
