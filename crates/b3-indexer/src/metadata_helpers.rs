use b3_core::{ContractError, EdgeKind, EdgeProvenance, NodeKind};

pub(crate) fn node_kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Project => "project",
        NodeKind::File => "file",
        NodeKind::Module => "module",
        NodeKind::Namespace => "namespace",
        NodeKind::Class => "class",
        NodeKind::Struct => "struct",
        NodeKind::Interface => "interface",
        NodeKind::Enum => "enum",
        NodeKind::Function => "function",
        NodeKind::Method => "method",
        NodeKind::Variable => "variable",
        NodeKind::Route => "route",
        NodeKind::Endpoint => "endpoint",
        NodeKind::ConfigKey => "config_key",
        NodeKind::Test => "test",
        NodeKind::Package => "package",
        NodeKind::Decision => "decision",
        NodeKind::CodeArea => "code_area",
    }
}

pub(crate) fn parse_node_kind(value: &str) -> NodeKind {
    match value {
        "project" => NodeKind::Project,
        "file" => NodeKind::File,
        "module" => NodeKind::Module,
        "namespace" => NodeKind::Namespace,
        "class" => NodeKind::Class,
        "struct" => NodeKind::Struct,
        "interface" => NodeKind::Interface,
        "enum" => NodeKind::Enum,
        "function" => NodeKind::Function,
        "method" => NodeKind::Method,
        "route" => NodeKind::Route,
        "endpoint" => NodeKind::Endpoint,
        "config_key" => NodeKind::ConfigKey,
        "test" => NodeKind::Test,
        "package" => NodeKind::Package,
        "decision" => NodeKind::Decision,
        "code_area" => NodeKind::CodeArea,
        _ => NodeKind::Variable,
    }
}

pub(crate) fn edge_kind_name(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Contains => "contains",
        EdgeKind::Imports => "imports",
        EdgeKind::Calls => "calls",
        EdgeKind::References => "references",
        EdgeKind::Implements => "implements",
        EdgeKind::Inherits => "inherits",
        EdgeKind::DependsOn => "depends_on",
        EdgeKind::Tests => "tests",
        EdgeKind::RoutesTo => "routes_to",
        EdgeKind::ReadsConfig => "reads_config",
        EdgeKind::WritesConfig => "writes_config",
        EdgeKind::SimilarTo => "similar_to",
        EdgeKind::Touches => "touches",
        EdgeKind::Decides => "decides",
    }
}

pub(crate) fn parse_edge_kind(value: &str) -> EdgeKind {
    match value {
        "contains" => EdgeKind::Contains,
        "imports" => EdgeKind::Imports,
        "calls" => EdgeKind::Calls,
        "references" => EdgeKind::References,
        "implements" => EdgeKind::Implements,
        "inherits" => EdgeKind::Inherits,
        "depends_on" => EdgeKind::DependsOn,
        "tests" => EdgeKind::Tests,
        "routes_to" => EdgeKind::RoutesTo,
        "reads_config" => EdgeKind::ReadsConfig,
        "writes_config" => EdgeKind::WritesConfig,
        "similar_to" => EdgeKind::SimilarTo,
        "touches" => EdgeKind::Touches,
        "decides" => EdgeKind::Decides,
        _ => EdgeKind::References,
    }
}

pub(crate) fn edge_provenance_name(provenance: EdgeProvenance) -> &'static str {
    match provenance {
        EdgeProvenance::Ast => "ast",
        EdgeProvenance::ImportAnalysis => "import_analysis",
        EdgeProvenance::TextHeuristic => "text_heuristic",
        EdgeProvenance::SemanticSimilarity => "semantic_similarity",
        EdgeProvenance::UserRecorded => "user_recorded",
    }
}

pub(crate) fn parse_edge_provenance(value: &str) -> EdgeProvenance {
    match value {
        "ast" => EdgeProvenance::Ast,
        "import_analysis" => EdgeProvenance::ImportAnalysis,
        "semantic_similarity" => EdgeProvenance::SemanticSimilarity,
        "user_recorded" => EdgeProvenance::UserRecorded,
        _ => EdgeProvenance::TextHeuristic,
    }
}

pub(crate) fn to_contract_error(error: impl std::fmt::Display) -> ContractError {
    ContractError::new(error.to_string())
}

pub(crate) fn escape_metadata(value: &str) -> String {
    value.replace(';', "%3B").replace('\n', "\\n")
}

pub(crate) fn unescape_metadata(value: &str) -> String {
    value.replace("%3B", ";").replace("\\n", "\n")
}

pub(crate) fn escape_metadata_semicolon(value: &str) -> String {
    value.replace(';', "%3B")
}

pub(crate) fn unescape_metadata_semicolon(value: &str) -> String {
    value.replace("%3B", ";")
}

pub(crate) fn prefixed_metadata_value(metadata: &str, prefix: &str, key: &str) -> Option<String> {
    let full_key = format!("{prefix}.{key}=");
    metadata
        .split(';')
        .find_map(|part| part.strip_prefix(&full_key).map(unescape_metadata))
}

pub(crate) fn prefixed_metadata_value_semicolon(
    metadata: &str,
    prefix: &str,
    key: &str,
) -> Option<String> {
    let full_key = format!("{prefix}.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(unescape_metadata_semicolon)
    })
}
