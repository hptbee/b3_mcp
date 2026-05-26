use std::collections::{BTreeMap, BTreeSet};

use b3_core::{
    ArchitectureConfidence, ArchitectureConfidenceLevel, ArchitectureEdge, ArchitectureEdgeKind,
    ArchitectureEvidence, ArchitectureEvidenceKind, ArchitectureMatchCandidate, ArchitectureNode,
    ArchitectureNodeKind, ArchitectureSource, ArchitectureSourceKind, ArchitectureWarning,
    ContractResult,
};
use b3_storage::StoredRoute;
use serde::{Deserialize, Serialize};

use super::{
    http_clients::{extract_http_client_calls, HttpClientCall},
    match_keys::{route_pattern_matches, RouteMatchKey, UNKNOWN_METHOD},
    open_existing_read_only, FederatedProjectStatus, FederatedQueryContext, FederatedRecord,
    GroupFederation, DEFAULT_BRANCH, DEFAULT_LIMIT,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteMatchOptions {
    pub method: Option<String>,
    pub path: Option<String>,
    pub source_project_id: Option<String>,
    pub target_project_id: Option<String>,
    pub min_confidence: Option<u16>,
    pub limit: usize,
    pub branch: Option<String>,
}

impl Default for RouteMatchOptions {
    fn default() -> Self {
        Self {
            method: None,
            path: None,
            source_project_id: None,
            target_project_id: None,
            min_confidence: None,
            limit: DEFAULT_LIMIT,
            branch: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupRouteMatchReport {
    pub group_id: String,
    pub group_name: String,
    pub matching_kind: String,
    pub match_count: usize,
    pub matches: Vec<RouteMatch>,
    pub warnings: Vec<ArchitectureWarning>,
    pub local_only: bool,
    pub federation_ready: bool,
    pub route_matching_ready: bool,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteMatch {
    pub candidate: ArchitectureMatchCandidate,
    pub edge: ArchitectureEdge,
    pub method: String,
    pub path: String,
    pub match_rule: String,
    pub score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerRoute {
    project_id: String,
    project_name: String,
    route: StoredRoute,
    key: RouteMatchKey,
    role: RouteRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RouteRole {
    ServerEndpoint,
    FrontendPageRoute,
    UnknownRoute,
}

impl GroupFederation {
    pub fn route_matches(
        &self,
        group_id: &str,
        options: RouteMatchOptions,
    ) -> ContractResult<GroupRouteMatchReport> {
        let context = self.resolve_context(group_id)?;
        match_routes(self, context, options)
    }
}

fn match_routes(
    _federation: &GroupFederation,
    context: FederatedQueryContext,
    options: RouteMatchOptions,
) -> ContractResult<GroupRouteMatchReport> {
    let branch = options
        .branch
        .clone()
        .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
    let mut warnings = context.warnings.clone();
    let limit = if options.limit == 0 {
        DEFAULT_LIMIT
    } else {
        options.limit.min(1_000)
    };
    let mut server_routes = Vec::new();
    let mut client_calls = Vec::new();

    for handle in context
        .projects
        .iter()
        .filter(|project| project.status == FederatedProjectStatus::Ready)
    {
        let storage = open_existing_read_only(handle)?;
        let routes = storage.routes(&handle.project_id, &branch, None, None, None, 1_000)?;
        server_routes.extend(routes.into_iter().map(|route| ServerRoute {
            key: RouteMatchKey::new(Some(&route.method), &route.path),
            role: classify_route_role(&route),
            project_id: handle.project_id.clone(),
            project_name: handle.display_name.clone(),
            route,
        }));
        let files = storage.file_contents(&handle.project_id, &branch, 1_000)?;
        client_calls.extend(extract_http_client_calls(&handle.project_id, &files));
    }

    let mut matches = Vec::new();
    let mut seen = BTreeSet::new();
    for client in &client_calls {
        if !matches_source_project(client, &options) {
            continue;
        }
        for server in server_routes
            .iter()
            .filter(|route| route.role == RouteRole::ServerEndpoint)
        {
            if !matches_target_project(server, &options) {
                continue;
            }
            if client.project_id == server.project_id {
                continue;
            }
            let Some((confidence, rule, warning)) = score_match(client, server) else {
                continue;
            };
            if let Some(min) = options.min_confidence {
                if confidence.score < min {
                    continue;
                }
            }
            if let Some(method) = &options.method {
                if !server.route.method.eq_ignore_ascii_case(method)
                    && client
                        .method
                        .as_deref()
                        .is_some_and(|client_method| !client_method.eq_ignore_ascii_case(method))
                {
                    continue;
                }
            }
            if let Some(path) = &options.path {
                let filter_key = RouteMatchKey::new(None, path);
                if client.key().path != filter_key.path && server.key.path != filter_key.path {
                    continue;
                }
            }
            let route_match = build_match(client, server, confidence, &rule, warning);
            if seen.insert(route_match.candidate.id.clone()) {
                matches.push(route_match);
            }
        }
    }

    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                left.candidate
                    .left_project_id
                    .cmp(&right.candidate.left_project_id)
            })
            .then_with(|| {
                left.candidate
                    .right_project_id
                    .cmp(&right.candidate.right_project_id)
            })
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.method.cmp(&right.method))
    });
    matches.truncate(limit);
    warnings.extend(unmatched_warnings(&client_calls, &server_routes));
    warnings.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.project_id.cmp(&right.project_id))
            .then_with(|| left.message.cmp(&right.message))
    });
    warnings.dedup_by(|left, right| {
        left.code == right.code
            && left.project_id == right.project_id
            && left.message == right.message
    });

    Ok(GroupRouteMatchReport {
        group_id: context.group_id,
        group_name: context.group_name,
        matching_kind: "route_api".to_string(),
        match_count: matches.len(),
        matches,
        warnings,
        local_only: true,
        federation_ready: true,
        route_matching_ready: true,
        branch,
    })
}

fn score_match(
    client: &HttpClientCall,
    server: &ServerRoute,
) -> Option<(ArchitectureConfidence, String, Option<ArchitectureWarning>)> {
    let client_key = client.key();
    if client_key.method == server.key.method && client_key.path == server.key.path {
        return Some((
            ArchitectureConfidence::high("exact HTTP method and normalized path match")
                .with_evidence(client.evidence.clone())
                .with_evidence(format!("server route {}", server.key.normalized_key)),
            "exact_method_path".to_string(),
            None,
        ));
    }
    if client_key.method == UNKNOWN_METHOD && client_key.path == server.key.path {
        return Some((
            ArchitectureConfidence::medium("client method unknown with exact normalized path")
                .with_evidence(client.evidence.clone()),
            "unknown_method_exact_path".to_string(),
            None,
        ));
    }
    if client_key.method == server.key.method
        && route_pattern_matches(&server.key.path, &client_key.path)
    {
        return Some((
            ArchitectureConfidence::new(
                ArchitectureConfidenceLevel::High,
                8_000,
                "HTTP method and route pattern match",
                vec![client.evidence.clone(), server.key.normalized_key.clone()],
            ),
            "method_route_pattern".to_string(),
            None,
        ));
    }
    if client_key.path == server.key.path && client_key.method != server.key.method {
        return Some((
            ArchitectureConfidence::low("same path but different HTTP methods"),
            "same_path_different_method".to_string(),
            Some(ArchitectureWarning {
                code: "method_mismatch".to_string(),
                message: format!(
                    "client method {} differs from server method {} for {}",
                    client_key.method, server.key.method, server.key.path
                ),
                project_id: Some(client.project_id.clone()),
            }),
        ));
    }
    None
}

fn build_match(
    client: &HttpClientCall,
    server: &ServerRoute,
    confidence: ArchitectureConfidence,
    rule: &str,
    warning: Option<ArchitectureWarning>,
) -> RouteMatch {
    let client_key = client.key();
    let left_node = client_node(client);
    let right_node = server_node(server);
    let normalized_key = format!(
        "{}=>{}",
        client_key.normalized_key, server.key.normalized_key
    );
    let candidate_id = ArchitectureMatchCandidate::deterministic_id(
        &client.project_id,
        Some(&server.project_id),
        ArchitectureEdgeKind::CallsHttpRoute,
        &normalized_key,
    );
    let evidence = vec![
        ArchitectureEvidence {
            kind: ArchitectureEvidenceKind::ExactLiteral,
            description: "client HTTP call literal".to_string(),
            value: Some(client_key.normalized_key.clone()),
            source: Some(client.source()),
        },
        ArchitectureEvidence {
            kind: ArchitectureEvidenceKind::NormalizedKey,
            description: "server route normalized key".to_string(),
            value: Some(server.key.normalized_key.clone()),
            source: Some(route_source(&server.project_id, &server.route)),
        },
    ];
    let mut metadata = BTreeMap::new();
    metadata.insert("match_rule".to_string(), rule.to_string());
    metadata.insert("method".to_string(), server.key.method.clone());
    metadata.insert("path".to_string(), server.key.path.clone());
    if let Some(base_url) = &client.base_url {
        metadata.insert("client_base_url".to_string(), base_url.clone());
    }
    let edge = ArchitectureEdge {
        id: ArchitectureEdge::deterministic_id(
            &left_node.id,
            &right_node.id,
            ArchitectureEdgeKind::CallsHttpRoute,
        ),
        from_node_id: left_node.id.clone(),
        to_node_id: right_node.id.clone(),
        kind: ArchitectureEdgeKind::CallsHttpRoute,
        confidence: confidence.clone(),
        evidence: evidence.clone(),
        sources: vec![
            client.source(),
            route_source(&server.project_id, &server.route),
        ],
        metadata: metadata.clone(),
    };
    RouteMatch {
        candidate: ArchitectureMatchCandidate {
            id: candidate_id,
            left_project_id: client.project_id.clone(),
            right_project_id: Some(server.project_id.clone()),
            left_node,
            right_node: Some(right_node),
            relationship_kind: ArchitectureEdgeKind::CallsHttpRoute,
            match_key: client_key.normalized_key,
            normalized_key,
            confidence: confidence.clone(),
            evidence,
            warnings: warning.into_iter().collect(),
        },
        edge,
        method: server.key.method.clone(),
        path: server.key.path.clone(),
        match_rule: rule.to_string(),
        score: confidence.score,
    }
}

fn client_node(client: &HttpClientCall) -> ArchitectureNode {
    let name = format!(
        "{} {}",
        client.method.as_deref().unwrap_or(UNKNOWN_METHOD),
        client.path
    );
    let id = ArchitectureNode::deterministic_id(
        &client.project_id,
        None,
        ArchitectureNodeKind::External,
        &name,
        Some(&client.file_path),
        None,
    );
    ArchitectureNode {
        id,
        project_id: client.project_id.clone(),
        service_id: None,
        kind: ArchitectureNodeKind::External,
        name: name.clone(),
        label: name,
        path: Some(client.file_path.clone()),
        symbol_id: None,
        metadata: BTreeMap::from([("http_client_path".to_string(), client.path.clone())]),
        confidence: client.confidence.clone(),
        sources: vec![client.source()],
    }
}

fn server_node(server: &ServerRoute) -> ArchitectureNode {
    let name = format!("{} {}", server.key.method, server.key.path);
    let id = ArchitectureNode::deterministic_id(
        &server.project_id,
        None,
        ArchitectureNodeKind::Route,
        &name,
        Some(&server.route.file_path),
        Some(&server.route.symbol_id),
    );
    ArchitectureNode {
        id,
        project_id: server.project_id.clone(),
        service_id: None,
        kind: ArchitectureNodeKind::Route,
        name: name.clone(),
        label: name,
        path: Some(server.route.file_path.clone()),
        symbol_id: Some(server.route.symbol_id.clone()),
        metadata: BTreeMap::from([
            ("framework".to_string(), server.route.framework.clone()),
            ("route_kind".to_string(), server.route.route_kind.clone()),
            ("project_name".to_string(), server.project_name.clone()),
        ]),
        confidence: ArchitectureConfidence::new(
            ArchitectureConfidenceLevel::High,
            server.route.confidence,
            "server route metadata",
            vec![server.route.source_kind.clone()],
        ),
        sources: vec![route_source(&server.project_id, &server.route)],
    }
}

fn route_source(project_id: &str, route: &StoredRoute) -> ArchitectureSource {
    ArchitectureSource {
        project_id: project_id.to_string(),
        file_path: route.file_path.clone(),
        symbol_id: Some(route.symbol_id.clone()),
        line_start: Some(route.line_start),
        line_end: Some(route.line_end),
        source_kind: ArchitectureSourceKind::RouteMetadata,
        extractor: Some(route.source_kind.clone()),
        metadata_key: Some("route.path".to_string()),
    }
}

fn classify_route_role(route: &StoredRoute) -> RouteRole {
    let kind = route.route_kind.to_ascii_lowercase();
    let framework = route.framework.to_ascii_lowercase();
    if kind == "api"
        || framework == "express"
        || framework == "nestjs"
        || framework == "fastify"
        || framework == "aspnetcore"
        || framework.starts_with("go_")
    {
        RouteRole::ServerEndpoint
    } else if kind == "page" || framework == "angular" || kind == "route" {
        RouteRole::FrontendPageRoute
    } else {
        RouteRole::UnknownRoute
    }
}

fn matches_source_project(client: &HttpClientCall, options: &RouteMatchOptions) -> bool {
    options
        .source_project_id
        .as_ref()
        .is_none_or(|project| &client.project_id == project)
}

fn matches_target_project(server: &ServerRoute, options: &RouteMatchOptions) -> bool {
    options
        .target_project_id
        .as_ref()
        .is_none_or(|project| &server.project_id == project)
}

fn unmatched_warnings(
    clients: &[HttpClientCall],
    servers: &[ServerRoute],
) -> Vec<ArchitectureWarning> {
    let server_paths = servers
        .iter()
        .filter(|server| server.role == RouteRole::ServerEndpoint)
        .map(|server| server.key.path.clone())
        .collect::<BTreeSet<_>>();
    clients
        .iter()
        .filter(|client| !server_paths.contains(&client.key().path))
        .map(|client| ArchitectureWarning {
            code: "unmatched_http_client_call".to_string(),
            message: format!("no server route matched client path {}", client.path),
            project_id: Some(client.project_id.clone()),
        })
        .collect()
}

#[allow(dead_code)]
fn _keep_federated_route_type(_: FederatedRecord<StoredRoute>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture::{LocalRegistryProject, DEFAULT_BRANCH};
    use b3_core::{
        BranchId, FileId, FileRecord, IndexStore, IndexedFileRecord, NodeKind, ProjectId, SymbolId,
        SymbolRecord,
    };
    use b3_storage::SqliteStorage;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn write_registry(path: &Path, projects: &[(&str, &str, &Path)], group_projects: &[&str]) {
        let projects_json = projects
            .iter()
            .map(|(id, name, db)| {
                let project = LocalRegistryProject {
                    id: id.to_string(),
                    name: name.to_string(),
                    path: db.parent().unwrap().display().to_string(),
                    database: db.display().to_string(),
                    tags: Vec::new(),
                    last_indexed_at: None,
                };
                serde_json::to_string(&project).expect("project json")
            })
            .collect::<Vec<_>>()
            .join(",");
        let project_ids = group_projects
            .iter()
            .map(|id| format!(r#""{id}""#))
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            path,
            format!(
                r#"{{"version":1,"projects":[{projects_json}],"groups":[{{"id":"suite","name":"Suite","project_ids":[{project_ids}]}}]}}"#
            ),
        )
        .expect("registry");
    }

    fn seed_project(
        db: &Path,
        project_id: &str,
        file_path: &str,
        content: &str,
        routes: &[(&str, &str, &str)],
    ) {
        let storage = SqliteStorage::open(db).expect("storage");
        let project = ProjectId::new(project_id);
        let branch = BranchId::new(DEFAULT_BRANCH);
        storage
            .ensure_project_branch(&project, &branch, &db.parent().unwrap().to_string_lossy())
            .expect("project");
        let symbols = routes
            .iter()
            .enumerate()
            .map(|(index, (method, path, kind))| {
                let mut symbol = SymbolRecord::new(
                    SymbolId::new(format!("{project_id}-route-{index}")),
                    FileId::new(format!("{project_id}-file")),
                    format!("{method} {path}"),
                    NodeKind::Route,
                );
                symbol.start_line = index + 1;
                symbol.end_line = index + 1;
                symbol.visibility = Some(format!(
                    "route.framework=express;route.kind={kind};route.method={method};route.path={path};route.file={file_path};route.handler=handler;route.source=ExpressCall;route.line_start={};route.line_end={};route.confidence=9500",
                    index + 1,
                    index + 1
                ));
                symbol
            })
            .collect();
        storage
            .upsert_indexed_file(
                &project,
                &branch,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new(format!("{project_id}-file")),
                        project_id: project.clone(),
                        path: file_path.to_string(),
                        content_hash: format!("hash-{project_id}"),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: content.len() as u64,
                    content: content.to_string(),
                    symbols,
                    edges: Vec::new(),
                },
            )
            .expect("indexed");
    }

    #[test]
    fn matches_exact_method_path_and_filters_frontend_pages() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        seed_project(
            &web_db,
            "web",
            "src/client.ts",
            r#"fetch("/api/orders"); axios.post("/api/users"); fetch("/dashboard");"#,
            &[("GET", "/dashboard", "page")],
        );
        seed_project(
            &api_db,
            "api",
            "src/server.ts",
            "app routes",
            &[("GET", "/api/orders", "api"), ("POST", "/api/users", "api")],
        );
        write_registry(
            &registry,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let report = federation
            .route_matches("suite", RouteMatchOptions::default())
            .expect("route matches");

        assert_eq!(report.match_count, 2);
        assert!(report.route_matching_ready);
        assert!(report
            .matches
            .iter()
            .any(|matched| matched.method == "GET" && matched.path == "/api/orders"));
        assert!(!report
            .matches
            .iter()
            .any(|matched| matched.path == "/dashboard"));
    }

    #[test]
    fn matches_route_patterns_and_method_mismatch_warnings() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        seed_project(
            &web_db,
            "web",
            "src/client.ts",
            r#"axios.get("/api/orders/123"); axios.post("/api/orders");"#,
            &[("GET", "/dashboard", "page")],
        );
        seed_project(
            &api_db,
            "api",
            "src/server.ts",
            "app routes",
            &[
                ("GET", "/api/orders/{id}", "api"),
                ("GET", "/api/orders", "api"),
            ],
        );
        write_registry(
            &registry,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let report = federation
            .route_matches("suite", RouteMatchOptions::default())
            .expect("route matches");

        assert!(report
            .matches
            .iter()
            .any(|matched| matched.match_rule == "method_route_pattern"));
        assert!(report
            .matches
            .iter()
            .any(|matched| matched.match_rule == "same_path_different_method"));
        assert!(report.matches.iter().any(|matched| {
            matched
                .candidate
                .warnings
                .iter()
                .any(|warning| warning.code == "method_mismatch")
        }));
    }
}
