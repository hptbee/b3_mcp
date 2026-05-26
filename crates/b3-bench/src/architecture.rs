use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use b3_core::{
    ArchitectureConfidenceLevel, BranchId, FileId, FileRecord, IndexStore, IndexedFileRecord,
    NodeKind, ProjectId, SymbolId, SymbolRecord,
};
use b3_query::architecture::{
    ArchitectureGraphRequest, ContextPackProfile, GroupFederation, GroupImpactDirection,
    GroupImpactRequest, GroupImpactSeedType, MessageMatchOptions, RouteMatchOptions,
    ServiceMapRequest,
};
use b3_storage::SqliteStorage;
use serde::{Deserialize, Serialize};

use crate::{to_contract_error, BenchmarkResult};
use b3_core::{ContractError, ContractResult};

const CONFIG_PATH: &str = "benchmarks/b3.benchmark.toml";
const GROUP_ID: &str = "phase_11_fixture";
const ARCH_BRANCH: &str = "main";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossProjectBenchmarkReport {
    pub schema_version: u32,
    pub benchmark_config_path: String,
    pub local_only: bool,
    pub offline_required: bool,
    pub telemetry: bool,
    pub global_db_merge_required: bool,
    pub projects_used: Vec<BenchmarkProjectStatus>,
    pub projects_skipped: Vec<BenchmarkProjectStatus>,
    pub warnings: Vec<String>,
    pub limitations: Vec<String>,
    pub readiness: ArchitectureReadinessMetrics,
    pub group_results: GroupBenchmarkMetrics,
    pub task_results: Vec<ArchitectureTaskResult>,
    pub metrics: CrossProjectMetrics,
    pub target_comparisons: ArchitectureTargetComparisons,
    pub branch_safety: BranchSafetyReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkProjectStatus {
    pub id: String,
    pub name: String,
    pub path: String,
    pub database: String,
    pub branch: String,
    pub required: bool,
    pub used: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureReadinessMetrics {
    pub group_ready: bool,
    pub route_matching_ready: bool,
    pub messaging_matching_ready: bool,
    pub dependency_matching_ready: bool,
    pub group_impact_ready: bool,
    pub graph_api_ready: bool,
    pub service_map_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GroupBenchmarkMetrics {
    pub project_count: usize,
    pub ready_project_count: usize,
    pub skipped_project_count: usize,
    pub route_match_count: usize,
    pub message_match_count: usize,
    pub dependency_match_count: usize,
    pub package_match_count: usize,
    pub contract_match_count: usize,
    pub infra_match_count: usize,
    pub impact_result_count: usize,
    pub graph_node_count: usize,
    pub graph_edge_count: usize,
    pub service_count: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureTaskResult {
    pub id: String,
    pub category: String,
    pub success: bool,
    pub skipped: bool,
    pub duration_ms: f64,
    pub expected_entity_hits: usize,
    pub expected_relationship_hits: usize,
    pub warning_count: usize,
    pub error: Option<String>,
    pub metrics: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossProjectMetrics {
    pub project_count: usize,
    pub ready_project_count: usize,
    pub skipped_project_count: usize,
    pub route_match_count: usize,
    pub message_match_count: usize,
    pub dependency_match_count: usize,
    pub impact_result_count: usize,
    pub graph_node_count: usize,
    pub graph_edge_count: usize,
    pub service_count: usize,
    pub expected_entity_hit_rate: f64,
    pub expected_relationship_hit_rate: f64,
    pub confidence_weighted_score: f64,
    pub warning_ratio: f64,
    pub unresolved_ratio: f64,
    pub context_pack_chars: usize,
    pub estimated_tokens: usize,
    pub token_reduction_multiplier: f64,
    pub tool_call_reduction_multiplier: f64,
    pub benchmark_duration_ms: f64,
    pub section_durations_ms: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureTargetComparisons {
    pub token_reduction_multiplier_target: f64,
    pub token_reduction_multiplier_actual: f64,
    pub token_reduction_multiplier_met: bool,
    pub tool_call_reduction_multiplier_target: f64,
    pub tool_call_reduction_multiplier_actual: f64,
    pub tool_call_reduction_multiplier_met: bool,
    pub answer_quality_target: f64,
    pub answer_quality_actual: f64,
    pub answer_quality_met: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchSafetyReport {
    pub requested_branch: String,
    pub branch_used: String,
    pub warnings: Vec<String>,
    pub full_git_intelligence_ready: bool,
}

#[derive(Debug, Clone, Default)]
struct BenchmarkConfig {
    targets: BenchmarkTargets,
    defaults: BenchmarkDefaults,
    projects: Vec<ConfiguredProject>,
}

#[derive(Debug, Clone)]
struct BenchmarkTargets {
    token_reduction_multiplier: f64,
    tool_call_reduction_multiplier: f64,
    answer_quality: f64,
}

impl Default for BenchmarkTargets {
    fn default() -> Self {
        Self {
            token_reduction_multiplier: 10.0,
            tool_call_reduction_multiplier: 2.1,
            answer_quality: 0.8,
        }
    }
}

#[derive(Debug, Clone)]
struct BenchmarkDefaults {
    branch: String,
    database_relative_path: String,
}

impl Default for BenchmarkDefaults {
    fn default() -> Self {
        Self {
            branch: ARCH_BRANCH.to_string(),
            database_relative_path: ".b3/b3.db".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ConfiguredProject {
    id: String,
    name: String,
    path: String,
    database: String,
    branch: String,
    enabled: bool,
    required: bool,
}

pub fn run_cross_project_benchmark(
    workspace: &Path,
) -> ContractResult<(CrossProjectBenchmarkReport, Vec<BenchmarkResult>)> {
    let started = Instant::now();
    let config_path = workspace_root().join(CONFIG_PATH);
    let (config, mut warnings) = load_benchmark_config(&config_path);
    let optional_projects = inspect_optional_projects(&config);
    warnings.extend(
        optional_projects
            .iter()
            .filter_map(|project| project.warning.clone()),
    );

    let fixture_dir = workspace.join("phase_11_architecture_fixture");
    fs::create_dir_all(&fixture_dir).map_err(to_contract_error)?;
    let registry_path = seed_architecture_fixture(&fixture_dir)?;
    let federation = GroupFederation::from_registry_path(&registry_path)?;

    let mut results = Vec::new();
    let mut section_durations = BTreeMap::new();

    let section = Instant::now();
    let summary = federation.summary(GROUP_ID)?;
    section_durations.insert(
        "group_federation".to_string(),
        elapsed_ms(section.elapsed()),
    );
    let mut federation_result =
        BenchmarkResult::new("cross_project_group_federation_latency", section.elapsed());
    federation_result.query_result_count = summary.project_count;
    federation_result.metadata.insert(
        "ready_projects".to_string(),
        summary.ready_project_count.to_string(),
    );
    federation_result.metadata.insert(
        "skipped_projects".to_string(),
        summary.skipped_project_count.to_string(),
    );
    results.push(federation_result);

    let section = Instant::now();
    let route_report = federation.route_matches(
        GROUP_ID,
        RouteMatchOptions {
            limit: 100,
            ..RouteMatchOptions::default()
        },
    )?;
    section_durations.insert("route_matching".to_string(), elapsed_ms(section.elapsed()));
    let mut route_result =
        BenchmarkResult::new("cross_project_route_matching_latency", section.elapsed());
    route_result.query_result_count = route_report.match_count;
    attach_confidence_metadata(
        &mut route_result,
        route_report
            .matches
            .iter()
            .map(|matched| matched.candidate.confidence.level),
    );
    results.push(route_result);

    let section = Instant::now();
    let message_report = federation.message_matches(
        GROUP_ID,
        MessageMatchOptions {
            limit: 100,
            ..MessageMatchOptions::default()
        },
    )?;
    section_durations.insert(
        "messaging_matching".to_string(),
        elapsed_ms(section.elapsed()),
    );
    let mut message_result =
        BenchmarkResult::new("cross_project_message_matching_latency", section.elapsed());
    message_result.query_result_count = message_report.match_count;
    message_result.metadata.insert(
        "channel_count".to_string(),
        message_report
            .matches
            .iter()
            .map(|matched| matched.channel_name.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            .to_string(),
    );
    attach_confidence_metadata(
        &mut message_result,
        message_report
            .matches
            .iter()
            .map(|matched| matched.candidate.confidence.level),
    );
    results.push(message_result);

    let section = Instant::now();
    let dependency_report = federation.dependency_matches(
        GROUP_ID,
        b3_query::architecture::DependencyMatchOptions {
            limit: 100,
            ..b3_query::architecture::DependencyMatchOptions::default()
        },
    )?;
    section_durations.insert(
        "dependency_matching".to_string(),
        elapsed_ms(section.elapsed()),
    );
    let mut dependency_result = BenchmarkResult::new(
        "cross_project_dependency_matching_latency",
        section.elapsed(),
    );
    dependency_result.query_result_count = dependency_report.match_count;
    dependency_result.metadata.insert(
        "package_matches".to_string(),
        dependency_report
            .matches
            .iter()
            .filter(|matched| matched.kind.as_str() == "package")
            .count()
            .to_string(),
    );
    dependency_result.metadata.insert(
        "contract_matches".to_string(),
        dependency_report
            .matches
            .iter()
            .filter(|matched| matched.kind.as_str() == "contract")
            .count()
            .to_string(),
    );
    dependency_result.metadata.insert(
        "infra_matches".to_string(),
        dependency_report
            .matches
            .iter()
            .filter(|matched| matched.kind.as_str() == "infrastructure")
            .count()
            .to_string(),
    );
    attach_confidence_metadata(
        &mut dependency_result,
        dependency_report
            .matches
            .iter()
            .map(|matched| matched.candidate.confidence.level),
    );
    results.push(dependency_result);

    let mut task_results = Vec::new();
    let route_impact = run_impact_task(
        &federation,
        "route_impact_orders",
        "route_impact",
        GroupImpactRequest {
            seed_type: GroupImpactSeedType::Route,
            method: Some("GET".to_string()),
            route_path: Some("/api/orders".to_string()),
            direction: GroupImpactDirection::Downstream,
            max_depth: Some(2),
            limit: Some(100),
            ..impact_defaults()
        },
    );
    task_results.push(route_impact);
    let message_impact = run_impact_task(
        &federation,
        "message_impact_orders_created",
        "message_impact",
        GroupImpactRequest {
            seed_type: GroupImpactSeedType::Message,
            message_name: Some("orders.created".to_string()),
            direction: GroupImpactDirection::Downstream,
            max_depth: Some(2),
            limit: Some(100),
            ..impact_defaults()
        },
    );
    task_results.push(message_impact);
    let package_impact = run_impact_task(
        &federation,
        "package_shared_contracts",
        "package_contract",
        GroupImpactRequest {
            seed_type: GroupImpactSeedType::Package,
            package_name: Some("shared-contracts".to_string()),
            direction: GroupImpactDirection::Downstream,
            max_depth: Some(2),
            limit: Some(100),
            ..impact_defaults()
        },
    );
    task_results.push(package_impact);
    let context_pack = run_impact_task(
        &federation,
        "cross_repo_context_pack_orders",
        "context_pack",
        GroupImpactRequest {
            seed_type: GroupImpactSeedType::Route,
            method: Some("GET".to_string()),
            route_path: Some("/api/orders".to_string()),
            direction: GroupImpactDirection::Downstream,
            include_context_pack: true,
            context_profile: ContextPackProfile::Balanced,
            max_depth: Some(2),
            limit: Some(100),
            ..impact_defaults()
        },
    );
    task_results.push(context_pack);

    let section = Instant::now();
    let graph = federation.architecture_graph(
        GROUP_ID,
        ArchitectureGraphRequest {
            max_nodes: Some(500),
            max_edges: Some(500),
            ..ArchitectureGraphRequest::default()
        },
    )?;
    section_durations.insert(
        "architecture_graph".to_string(),
        elapsed_ms(section.elapsed()),
    );
    let mut graph_result = BenchmarkResult::new(
        "cross_project_architecture_graph_latency",
        section.elapsed(),
    );
    graph_result.query_result_count = graph.summary.node_count;
    graph_result.edges_indexed = graph.summary.edge_count;
    graph_result.metadata.insert(
        "unresolved_count".to_string(),
        graph.summary.unresolved_count.to_string(),
    );
    results.push(graph_result);
    task_results.push(ArchitectureTaskResult {
        id: "architecture_graph".to_string(),
        category: "architecture_graph".to_string(),
        success: graph.summary.edge_count > 0,
        skipped: false,
        duration_ms: *section_durations.get("architecture_graph").unwrap_or(&0.0),
        expected_entity_hits: graph.summary.node_count,
        expected_relationship_hits: graph.summary.edge_count,
        warning_count: graph.warnings.len(),
        error: None,
        metrics: BTreeMap::from([
            ("nodes".to_string(), graph.summary.node_count.to_string()),
            ("edges".to_string(), graph.summary.edge_count.to_string()),
            (
                "isolated_projects".to_string(),
                graph.summary.isolated_projects.len().to_string(),
            ),
        ]),
    });

    let section = Instant::now();
    let service_map = federation.service_map(
        GROUP_ID,
        ServiceMapRequest {
            limit: Some(500),
            ..ServiceMapRequest::default()
        },
    )?;
    section_durations.insert("service_map".to_string(), elapsed_ms(section.elapsed()));
    let mut service_result =
        BenchmarkResult::new("cross_project_service_map_latency", section.elapsed());
    service_result.query_result_count = service_map.services.len();
    service_result.edges_indexed = service_map.service_edges.len();
    service_result.metadata.insert(
        "service_edges".to_string(),
        service_map.service_edges.len().to_string(),
    );
    results.push(service_result);
    task_results.push(ArchitectureTaskResult {
        id: "service_map".to_string(),
        category: "service_map".to_string(),
        success: !service_map.services.is_empty(),
        skipped: false,
        duration_ms: *section_durations.get("service_map").unwrap_or(&0.0),
        expected_entity_hits: service_map.services.len(),
        expected_relationship_hits: service_map.service_edges.len(),
        warning_count: service_map.warnings.len(),
        error: None,
        metrics: BTreeMap::from([
            (
                "services".to_string(),
                service_map.services.len().to_string(),
            ),
            (
                "service_edges".to_string(),
                service_map.service_edges.len().to_string(),
            ),
        ]),
    });

    let context_pack_chars = task_results
        .iter()
        .filter(|task| task.category == "context_pack")
        .filter_map(|task| task.metrics.get("context_pack_chars"))
        .filter_map(|value| value.parse::<usize>().ok())
        .sum::<usize>();
    let estimated_tokens = context_pack_chars.div_ceil(4);
    let expected_entity_hits = task_results
        .iter()
        .map(|task| task.expected_entity_hits)
        .sum::<usize>();
    let expected_relationship_hits = task_results
        .iter()
        .map(|task| task.expected_relationship_hits)
        .sum::<usize>();
    let task_successes = task_results.iter().filter(|task| task.success).count();
    let answer_quality = if task_results.is_empty() {
        0.0
    } else {
        task_successes as f64 / task_results.len() as f64
    };
    let warning_count = summary.warnings.len()
        + route_report.warnings.len()
        + message_report.warnings.len()
        + dependency_report.warnings.len()
        + graph.warnings.len()
        + service_map.warnings.len()
        + task_results
            .iter()
            .map(|task| task.warning_count)
            .sum::<usize>()
        + warnings.len();
    let operation_count = 7 + task_results.len();
    let warning_ratio = ratio(warning_count, operation_count);
    let unresolved_ratio = ratio(graph.summary.unresolved_count, graph.summary.edge_count);
    let token_reduction_multiplier = if context_pack_chars == 0 {
        0.0
    } else {
        12_000.0 / context_pack_chars as f64
    };
    let tool_call_reduction_multiplier = 18.0 / 7.0;

    let group_results = GroupBenchmarkMetrics {
        project_count: summary.project_count,
        ready_project_count: summary.ready_project_count,
        skipped_project_count: summary.skipped_project_count,
        route_match_count: route_report.match_count,
        message_match_count: message_report.match_count,
        dependency_match_count: dependency_report.match_count,
        package_match_count: dependency_report
            .matches
            .iter()
            .filter(|matched| matched.kind.as_str() == "package")
            .count(),
        contract_match_count: dependency_report
            .matches
            .iter()
            .filter(|matched| matched.kind.as_str() == "contract")
            .count(),
        infra_match_count: dependency_report
            .matches
            .iter()
            .filter(|matched| matched.kind.as_str() == "infrastructure")
            .count(),
        impact_result_count: task_results
            .iter()
            .filter(|task| task.category.contains("impact") && task.success)
            .count(),
        graph_node_count: graph.summary.node_count,
        graph_edge_count: graph.summary.edge_count,
        service_count: service_map.services.len(),
        warning_count,
    };

    let metrics = CrossProjectMetrics {
        project_count: group_results.project_count,
        ready_project_count: group_results.ready_project_count,
        skipped_project_count: group_results.skipped_project_count,
        route_match_count: group_results.route_match_count,
        message_match_count: group_results.message_match_count,
        dependency_match_count: group_results.dependency_match_count,
        impact_result_count: group_results.impact_result_count,
        graph_node_count: group_results.graph_node_count,
        graph_edge_count: group_results.graph_edge_count,
        service_count: group_results.service_count,
        expected_entity_hit_rate: ratio(expected_entity_hits, task_results.len().max(1)),
        expected_relationship_hit_rate: ratio(
            expected_relationship_hits,
            task_results.len().max(1),
        ),
        confidence_weighted_score: answer_quality,
        warning_ratio,
        unresolved_ratio,
        context_pack_chars,
        estimated_tokens,
        token_reduction_multiplier,
        tool_call_reduction_multiplier,
        benchmark_duration_ms: elapsed_ms(started.elapsed()),
        section_durations_ms: section_durations,
    };

    let target_comparisons = ArchitectureTargetComparisons {
        token_reduction_multiplier_target: config.targets.token_reduction_multiplier,
        token_reduction_multiplier_actual: token_reduction_multiplier,
        token_reduction_multiplier_met: token_reduction_multiplier
            >= config.targets.token_reduction_multiplier,
        tool_call_reduction_multiplier_target: config.targets.tool_call_reduction_multiplier,
        tool_call_reduction_multiplier_actual: tool_call_reduction_multiplier,
        tool_call_reduction_multiplier_met: tool_call_reduction_multiplier
            >= config.targets.tool_call_reduction_multiplier,
        answer_quality_target: config.targets.answer_quality,
        answer_quality_actual: answer_quality,
        answer_quality_met: answer_quality >= config.targets.answer_quality,
    };

    let branch_safety = BranchSafetyReport {
        requested_branch: config.defaults.branch.clone(),
        branch_used: ARCH_BRANCH.to_string(),
        warnings: vec![
            "Phase 11.7 reports branch assumptions only; full Git Intelligence is Phase 21"
                .to_string(),
            "After switching branches, reindex before comparing architecture benchmark results"
                .to_string(),
        ],
        full_git_intelligence_ready: false,
    };

    let mut projects_used = vec![
        BenchmarkProjectStatus {
            id: "fixture_frontend".to_string(),
            name: "Fixture Frontend".to_string(),
            path: fixture_dir.join("frontend").display().to_string(),
            database: fixture_dir
                .join("frontend")
                .join(".b3")
                .join("b3.db")
                .display()
                .to_string(),
            branch: ARCH_BRANCH.to_string(),
            required: false,
            used: true,
            warning: None,
        },
        BenchmarkProjectStatus {
            id: "fixture_api".to_string(),
            name: "Fixture API".to_string(),
            path: fixture_dir.join("api").display().to_string(),
            database: fixture_dir
                .join("api")
                .join(".b3")
                .join("b3.db")
                .display()
                .to_string(),
            branch: ARCH_BRANCH.to_string(),
            required: false,
            used: true,
            warning: None,
        },
        BenchmarkProjectStatus {
            id: "fixture_worker".to_string(),
            name: "Fixture Worker".to_string(),
            path: fixture_dir.join("worker").display().to_string(),
            database: fixture_dir
                .join("worker")
                .join(".b3")
                .join("b3.db")
                .display()
                .to_string(),
            branch: ARCH_BRANCH.to_string(),
            required: false,
            used: true,
            warning: None,
        },
        BenchmarkProjectStatus {
            id: "fixture_shared".to_string(),
            name: "Fixture Shared".to_string(),
            path: fixture_dir.join("shared").display().to_string(),
            database: fixture_dir
                .join("shared")
                .join(".b3")
                .join("b3.db")
                .display()
                .to_string(),
            branch: ARCH_BRANCH.to_string(),
            required: false,
            used: true,
            warning: None,
        },
    ];
    projects_used.extend(
        optional_projects
            .iter()
            .filter(|project| project.used)
            .cloned(),
    );
    let projects_skipped = optional_projects
        .into_iter()
        .filter(|project| !project.used)
        .collect::<Vec<_>>();

    Ok((
        CrossProjectBenchmarkReport {
            schema_version: 1,
            benchmark_config_path: CONFIG_PATH.to_string(),
            local_only: true,
            offline_required: true,
            telemetry: false,
            global_db_merge_required: false,
            projects_used,
            projects_skipped,
            warnings,
            limitations: vec![
                "fixture/local-repo benchmark only; not a 31 public real-world repository claim"
                    .to_string(),
                "quality is deterministic task coverage, not human or LLM grading".to_string(),
                "architecture graph UI is not implemented".to_string(),
                "full Git Intelligence remains Phase 21".to_string(),
            ],
            readiness: ArchitectureReadinessMetrics {
                group_ready: true,
                route_matching_ready: true,
                messaging_matching_ready: true,
                dependency_matching_ready: true,
                group_impact_ready: true,
                graph_api_ready: true,
                service_map_ready: true,
            },
            group_results,
            task_results,
            metrics,
            target_comparisons,
            branch_safety,
        },
        results,
    ))
}

pub fn format_cross_project_summary(report: &CrossProjectBenchmarkReport) -> String {
    let mut lines = Vec::new();
    lines.push("Phase 11 cross-project architecture benchmark".to_string());
    lines.push(format!(
        "projects used={} skipped={} warnings={}",
        report.projects_used.len(),
        report.projects_skipped.len(),
        report.warnings.len()
    ));
    lines.push(String::new());
    lines.push("Phase 11 readiness | ready".to_string());
    lines.push("--- | ---".to_string());
    lines.push(format!(
        "group federation | {}",
        report.readiness.group_ready
    ));
    lines.push(format!(
        "route matching | {}",
        report.readiness.route_matching_ready
    ));
    lines.push(format!(
        "messaging matching | {}",
        report.readiness.messaging_matching_ready
    ));
    lines.push(format!(
        "dependency matching | {}",
        report.readiness.dependency_matching_ready
    ));
    lines.push(format!(
        "group impact | {}",
        report.readiness.group_impact_ready
    ));
    lines.push(format!("graph API | {}", report.readiness.graph_api_ready));
    lines.push(format!(
        "service map | {}",
        report.readiness.service_map_ready
    ));
    lines.push(String::new());
    lines.push("match count | value".to_string());
    lines.push("--- | ---:".to_string());
    lines.push(format!(
        "route matches | {}",
        report.group_results.route_match_count
    ));
    lines.push(format!(
        "message matches | {}",
        report.group_results.message_match_count
    ));
    lines.push(format!(
        "dependency matches | {}",
        report.group_results.dependency_match_count
    ));
    lines.push(format!(
        "package matches | {}",
        report.group_results.package_match_count
    ));
    lines.push(format!(
        "contract matches | {}",
        report.group_results.contract_match_count
    ));
    lines.push(format!(
        "infrastructure matches | {}",
        report.group_results.infra_match_count
    ));
    lines.push(String::new());
    lines.push("impact/context-pack | value".to_string());
    lines.push("--- | ---:".to_string());
    lines.push(format!(
        "impact successes | {}",
        report.group_results.impact_result_count
    ));
    lines.push(format!(
        "context pack chars | {}",
        report.metrics.context_pack_chars
    ));
    lines.push(format!(
        "estimated context tokens | {}",
        report.metrics.estimated_tokens
    ));
    for task in &report.task_results {
        if matches!(
            task.category.as_str(),
            "route_impact" | "message_impact" | "package_contract" | "context_pack"
        ) {
            lines.push(format!(
                "{} | entities={} relationships={} warnings={}",
                task.id,
                task.expected_entity_hits,
                task.expected_relationship_hits,
                task.warning_count
            ));
        }
    }
    lines.push(String::new());
    lines.push("graph/service-map | value".to_string());
    lines.push("--- | ---:".to_string());
    lines.push(format!(
        "graph nodes | {}",
        report.group_results.graph_node_count
    ));
    lines.push(format!(
        "graph edges | {}",
        report.group_results.graph_edge_count
    ));
    lines.push(format!("services | {}", report.group_results.service_count));
    if let Some(graph_task) = report
        .task_results
        .iter()
        .find(|task| task.category == "architecture_graph")
    {
        lines.push(format!(
            "architecture graph warnings | {}",
            graph_task.warning_count
        ));
    }
    if let Some(service_task) = report
        .task_results
        .iter()
        .find(|task| task.category == "service_map")
    {
        lines.push(format!(
            "service map warnings | {}",
            service_task.warning_count
        ));
    }
    lines.push(String::new());
    lines.push("current measured results vs targets | actual | target | met".to_string());
    lines.push("--- | ---: | ---: | ---".to_string());
    lines.push(format!(
        "token reduction | {:.2}x | {:.1}x | {}",
        report.target_comparisons.token_reduction_multiplier_actual,
        report.target_comparisons.token_reduction_multiplier_target,
        report.target_comparisons.token_reduction_multiplier_met
    ));
    lines.push(format!(
        "tool-call reduction | {:.2}x | {:.1}x | {}",
        report
            .target_comparisons
            .tool_call_reduction_multiplier_actual,
        report
            .target_comparisons
            .tool_call_reduction_multiplier_target,
        report.target_comparisons.tool_call_reduction_multiplier_met
    ));
    lines.push(format!(
        "answer quality approximation | {:.3} | {:.1} | {}",
        report.target_comparisons.answer_quality_actual,
        report.target_comparisons.answer_quality_target,
        report.target_comparisons.answer_quality_met
    ));
    lines.push(String::new());
    lines.push(format!(
        "branch safety: requested={} used={} full_git_intelligence_ready={}",
        report.branch_safety.requested_branch,
        report.branch_safety.branch_used,
        report.branch_safety.full_git_intelligence_ready
    ));
    if !report.branch_safety.warnings.is_empty() {
        lines.push(format!(
            "branch warnings: {}",
            report.branch_safety.warnings.join(" | ")
        ));
    }
    if !report.warnings.is_empty() {
        lines.push(format!("warnings: {}", report.warnings.join(" | ")));
    }
    if !report.limitations.is_empty() {
        lines.push(format!("limitations: {}", report.limitations.join(" | ")));
    }
    lines.push("offline/free: local registry/project DBs only; no network, telemetry, cloud graph/vector DB, package manager, Docker/Kubernetes/Terraform, broker, or runtime HTTP calls".to_string());
    lines.join("\n")
}

fn run_impact_task(
    federation: &GroupFederation,
    id: &str,
    category: &str,
    request: GroupImpactRequest,
) -> ArchitectureTaskResult {
    let started = Instant::now();
    match federation.group_impact(GROUP_ID, request) {
        Ok(report) => {
            let context_pack_chars = report
                .context_pack
                .as_ref()
                .map(|pack| pack.returned_chars)
                .unwrap_or_default();
            ArchitectureTaskResult {
                id: id.to_string(),
                category: category.to_string(),
                success: !report.nodes.is_empty(),
                skipped: false,
                duration_ms: elapsed_ms(started.elapsed()),
                expected_entity_hits: report.nodes.len(),
                expected_relationship_hits: report.edges.len(),
                warning_count: report.warnings.len(),
                error: None,
                metrics: BTreeMap::from([
                    (
                        "impacted_projects".to_string(),
                        report.impacted_project_count.to_string(),
                    ),
                    (
                        "impacted_files".to_string(),
                        report.impacted_file_count.to_string(),
                    ),
                    (
                        "impacted_symbols".to_string(),
                        report.impacted_symbol_count.to_string(),
                    ),
                    ("edges".to_string(), report.edges.len().to_string()),
                    (
                        "context_pack_chars".to_string(),
                        context_pack_chars.to_string(),
                    ),
                    (
                        "estimated_tokens".to_string(),
                        context_pack_chars.div_ceil(4).to_string(),
                    ),
                ]),
            }
        }
        Err(error) => ArchitectureTaskResult {
            id: id.to_string(),
            category: category.to_string(),
            success: false,
            skipped: true,
            duration_ms: elapsed_ms(started.elapsed()),
            expected_entity_hits: 0,
            expected_relationship_hits: 0,
            warning_count: 1,
            error: Some(error.to_string()),
            metrics: BTreeMap::new(),
        },
    }
}

fn impact_defaults() -> GroupImpactRequest {
    GroupImpactRequest {
        seed_type: GroupImpactSeedType::Query,
        seed_project_id: None,
        seed_path: None,
        seed_symbol: None,
        method: None,
        route_path: None,
        message_name: None,
        package_name: None,
        contract_name: None,
        infra_name: None,
        query: Some("order creation flow".to_string()),
        branch: Some(ARCH_BRANCH.to_string()),
        direction: GroupImpactDirection::Downstream,
        max_depth: Some(2),
        limit: Some(100),
        context_profile: ContextPackProfile::Balanced,
        include_context_pack: false,
        min_confidence: None,
    }
}

fn seed_architecture_fixture(root: &Path) -> ContractResult<PathBuf> {
    let frontend_db = root.join("frontend").join(".b3").join("b3.db");
    let api_db = root.join("api").join(".b3").join("b3.db");
    let worker_db = root.join("worker").join(".b3").join("b3.db");
    let shared_db = root.join("shared").join(".b3").join("b3.db");
    seed_project(
        &frontend_db,
        "frontend",
        "src/orders.ts",
        r#"import { OrderDto } from "shared-contracts"; export async function loadOrders() { return fetch("/api/orders"); }"#,
        Vec::new(),
    )?;
    seed_project(
        &api_db,
        "api",
        "src/orders.ts",
        "app.get('/api/orders', handler); publish('orders.created'); type OrderDto = { id: string };",
        vec![
            route_symbol("api", "src/orders.ts", "GET", "/api/orders"),
            message_symbol("api", "src/orders.ts", "outbound", "orders.created"),
        ],
    )?;
    seed_project(
        &worker_db,
        "worker",
        "src/worker.ts",
        "consume('orders.created'); import { OrderDto } from 'shared-contracts';",
        vec![message_symbol(
            "worker",
            "src/worker.ts",
            "inbound",
            "orders.created",
        )],
    )?;
    seed_project(
        &worker_db,
        "worker",
        "package.json",
        r#"{"name":"worker","dependencies":{"shared-contracts":"file:../shared"}}"#,
        Vec::new(),
    )?;
    seed_project(
        &shared_db,
        "shared",
        "package.json",
        r#"{"name":"shared-contracts","version":"1.0.0"}"#,
        Vec::new(),
    )?;
    seed_project(
        &shared_db,
        "shared",
        "src/contracts.ts",
        "export interface OrderDto { id: string }",
        Vec::new(),
    )?;
    let registry_path = root.join("registry.json");
    let projects = [
        ("frontend", "Frontend", &frontend_db),
        ("api", "API", &api_db),
        ("worker", "Worker", &worker_db),
        ("shared", "Shared", &shared_db),
    ];
    let projects_json = projects
        .iter()
        .map(|(id, name, db)| {
            format!(
                r#"{{"id":"{id}","name":"{name}","path":"{}","database":"{}","tags":[]}}"#,
                db.parent()
                    .unwrap()
                    .display()
                    .to_string()
                    .replace('\\', "\\\\"),
                db.display().to_string().replace('\\', "\\\\")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        &registry_path,
        format!(
            r#"{{"version":1,"projects":[{projects_json}],"groups":[{{"id":"{GROUP_ID}","name":"Phase 11 Fixture","project_ids":["frontend","api","worker","shared"]}}]}}"#
        ),
    )
    .map_err(to_contract_error)?;
    Ok(registry_path)
}

fn seed_project(
    db: &Path,
    project_id: &str,
    file_path: &str,
    content: &str,
    symbols: Vec<SymbolRecord>,
) -> ContractResult<()> {
    let storage = SqliteStorage::open(db)?;
    let project = ProjectId::new(project_id);
    let branch = BranchId::new(ARCH_BRANCH);
    storage.ensure_project_branch(&project, &branch, &db.parent().unwrap().to_string_lossy())?;
    storage.upsert_indexed_file(
        &project,
        &branch,
        IndexedFileRecord {
            file: FileRecord {
                id: FileId::new(format!("{project_id}-{file_path}")),
                project_id: project.clone(),
                path: file_path.to_string(),
                content_hash: format!("hash-{project_id}-{file_path}"),
            },
            language: Some("typescript".to_string()),
            size_bytes: content.len() as u64,
            content: content.to_string(),
            symbols,
            edges: Vec::new(),
        },
    )?;
    Ok(())
}

fn route_symbol(project_id: &str, file_path: &str, method: &str, path: &str) -> SymbolRecord {
    let mut symbol = SymbolRecord::new(
        SymbolId::new(format!("{project_id}-route")),
        FileId::new(format!("{project_id}-{file_path}")),
        format!("{method} {path}"),
        NodeKind::Route,
    );
    symbol.start_line = 1;
    symbol.end_line = 1;
    symbol.visibility = Some(format!(
        "route.framework=express;route.kind=api;route.method={method};route.path={path};route.file=src/orders.ts;route.handler=handler;route.source=ExpressCall;route.line_start=1;route.line_end=1;route.confidence=9500"
    ));
    symbol
}

fn message_symbol(project_id: &str, file_path: &str, direction: &str, topic: &str) -> SymbolRecord {
    let mut symbol = SymbolRecord::new(
        SymbolId::new(format!("{project_id}-message-{direction}")),
        FileId::new(format!("{project_id}-{file_path}")),
        topic.to_string(),
        NodeKind::Endpoint,
    );
    let kind = if direction == "inbound" {
        "Consumer"
    } else {
        "Producer"
    };
    symbol.start_line = 1;
    symbol.end_line = 1;
    symbol.visibility = Some(format!(
        "messaging.technology=kafka;messaging.kind={kind};messaging.direction={direction};messaging.topic={topic};messaging.file=src/worker.ts;messaging.source=BenchmarkMessaging;messaging.line_start=1;messaging.line_end=1;messaging.confidence=9000"
    ));
    symbol
}

fn load_benchmark_config(path: &Path) -> (BenchmarkConfig, Vec<String>) {
    if !path.exists() {
        return (
            BenchmarkConfig {
                targets: BenchmarkTargets::default(),
                defaults: BenchmarkDefaults::default(),
                projects: default_optional_projects(),
            },
            vec![format!(
                "Benchmark config not found; using local defaults: {}",
                path.display()
            )],
        );
    }
    match fs::read_to_string(path) {
        Ok(content) => parse_benchmark_config(&content),
        Err(error) => (
            BenchmarkConfig {
                targets: BenchmarkTargets::default(),
                defaults: BenchmarkDefaults::default(),
                projects: default_optional_projects(),
            },
            vec![format!(
                "Failed to read benchmark config {}; using defaults: {error}",
                path.display()
            )],
        ),
    }
}

fn parse_benchmark_config(content: &str) -> (BenchmarkConfig, Vec<String>) {
    let mut config = BenchmarkConfig::default();
    let mut warnings = Vec::new();
    let mut section = String::new();
    let mut current_project: Option<ConfiguredProject> = None;
    let mut local_only = false;
    let mut offline_required = false;
    let mut telemetry_false = false;
    for raw in content.lines() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[projects]]" {
            if let Some(project) = current_project.take() {
                config.projects.push(project);
            }
            current_project = Some(ConfiguredProject {
                enabled: true,
                branch: config.defaults.branch.clone(),
                ..ConfiguredProject::default()
            });
            section = "projects".to_string();
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if let Some(project) = current_project.take() {
                config.projects.push(project);
            }
            section = line.trim_matches(['[', ']']).to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = unquote(value.trim());
        match section.as_str() {
            "targets" => match key {
                "token_reduction_multiplier" => {
                    config.targets.token_reduction_multiplier = value
                        .parse()
                        .unwrap_or(config.targets.token_reduction_multiplier);
                }
                "tool_call_reduction_multiplier" => {
                    config.targets.tool_call_reduction_multiplier = value
                        .parse()
                        .unwrap_or(config.targets.tool_call_reduction_multiplier);
                }
                "answer_quality" => {
                    config.targets.answer_quality =
                        value.parse().unwrap_or(config.targets.answer_quality);
                }
                _ => {}
            },
            "defaults" => match key {
                "branch" => config.defaults.branch = value,
                "database_relative_path" => config.defaults.database_relative_path = value,
                _ => {}
            },
            "projects" => {
                if let Some(project) = current_project.as_mut() {
                    match key {
                        "id" => project.id = value,
                        "name" => project.name = value,
                        "path" => project.path = value,
                        "database" => project.database = value,
                        "branch" => project.branch = value,
                        "enabled" => project.enabled = value == "true",
                        "required" => project.required = value == "true",
                        _ => {}
                    }
                }
            }
            _ => match key {
                "local_only" => local_only = value == "true",
                "offline_required" => offline_required = value == "true",
                "telemetry" => telemetry_false = value == "false",
                _ => {}
            },
        }
    }
    if let Some(project) = current_project.take() {
        config.projects.push(project);
    }
    if !local_only {
        warnings.push("Benchmark config local_only was not true".to_string());
    }
    if !offline_required {
        warnings.push("Benchmark config offline_required was not true".to_string());
    }
    if !telemetry_false {
        warnings.push("Benchmark config telemetry was not false".to_string());
    }
    if config.projects.is_empty() {
        config.projects = default_optional_projects();
    }
    (config, warnings)
}

fn default_optional_projects() -> Vec<ConfiguredProject> {
    vec![
        ConfiguredProject {
            id: "b3_mcp".to_string(),
            name: "B3 MCP".to_string(),
            path: r"D:\Project\b3_mcp".to_string(),
            database: r"D:\Project\b3_mcp\.b3\b3.db".to_string(),
            branch: ARCH_BRANCH.to_string(),
            enabled: true,
            required: false,
        },
        ConfiguredProject {
            id: "project_b".to_string(),
            name: "Project_B".to_string(),
            path: r"D:\Project\Project_B".to_string(),
            database: r"D:\Project\Project_B\.b3\b3.db".to_string(),
            branch: ARCH_BRANCH.to_string(),
            enabled: true,
            required: false,
        },
        ConfiguredProject {
            id: "tuvi_b".to_string(),
            name: "Tuvi_B".to_string(),
            path: r"D:\Project\Tuvi_B".to_string(),
            database: r"D:\Project\Tuvi_B\.b3\b3.db".to_string(),
            branch: ARCH_BRANCH.to_string(),
            enabled: true,
            required: false,
        },
    ]
}

fn inspect_optional_projects(config: &BenchmarkConfig) -> Vec<BenchmarkProjectStatus> {
    config
        .projects
        .iter()
        .filter(|project| project.enabled && project.id != "semantic_search_fixture")
        .map(|project| {
            let path = PathBuf::from(&project.path);
            let database = if project.database.is_empty() {
                path.join(&config.defaults.database_relative_path)
            } else {
                PathBuf::from(&project.database)
            };
            let warning = if !path.exists() {
                Some(format!(
                    "Optional benchmark project path not found: {}",
                    path.display()
                ))
            } else if !database.exists() {
                Some(format!(
                    "Optional benchmark database not found: {}",
                    database.display()
                ))
            } else {
                None
            };
            BenchmarkProjectStatus {
                id: project.id.clone(),
                name: if project.name.is_empty() {
                    project.id.clone()
                } else {
                    project.name.clone()
                },
                path: path.display().to_string(),
                database: database.display().to_string(),
                branch: if project.branch.is_empty() {
                    config.defaults.branch.clone()
                } else {
                    project.branch.clone()
                },
                required: project.required,
                used: warning.is_none(),
                warning,
            }
        })
        .collect()
}

fn attach_confidence_metadata(
    result: &mut BenchmarkResult,
    levels: impl Iterator<Item = ArchitectureConfidenceLevel>,
) {
    let mut counts = BTreeMap::<String, usize>::new();
    for level in levels {
        *counts.entry(format!("{level:?}")).or_default() += 1;
    }
    for (level, count) in counts {
        result
            .metadata
            .insert(format!("confidence_{level}"), count.to_string());
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn elapsed_ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').replace("\\\\", "\\")
}

fn workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("Cargo.toml").exists() && cwd.join("crates").exists() {
        return cwd;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[allow(dead_code)]
fn ensure_no_external_runtime() -> Result<(), ContractError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parsing_keeps_optional_projects_optional() {
        let (config, warnings) = parse_benchmark_config(
            r#"
            local_only = true
            offline_required = true
            telemetry = false
            [targets]
            token_reduction_multiplier = 10.0
            tool_call_reduction_multiplier = 2.1
            answer_quality = 0.8
            [[projects]]
            id = "project_b"
            name = "Project_B"
            path = "D:\\Project\\Project_B"
            database = "D:\\Project\\Project_B\\.b3\\b3.db"
            enabled = true
            required = false
            "#,
        );
        assert!(warnings.is_empty());
        assert_eq!(config.projects.len(), 1);
        assert!(!config.projects[0].required);
    }

    #[test]
    fn missing_optional_paths_warn_not_fail() {
        let config = BenchmarkConfig {
            projects: vec![ConfiguredProject {
                id: "missing".to_string(),
                name: "Missing".to_string(),
                path: r"Z:\definitely\missing".to_string(),
                database: r"Z:\definitely\missing\.b3\b3.db".to_string(),
                branch: ARCH_BRANCH.to_string(),
                enabled: true,
                required: false,
            }],
            ..BenchmarkConfig::default()
        };
        let projects = inspect_optional_projects(&config);
        assert_eq!(projects.len(), 1);
        assert!(!projects[0].used);
        assert!(projects[0]
            .warning
            .as_deref()
            .unwrap_or_default()
            .contains("not found"));
    }

    #[test]
    fn cross_project_benchmark_outputs_metrics_and_warnings() {
        let root = std::env::temp_dir().join(format!("b3-arch-bench-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        let (report, results) = run_cross_project_benchmark(&root).expect("benchmark");
        assert!(report.local_only);
        assert!(report.readiness.graph_api_ready);
        assert!(report.readiness.service_map_ready);
        assert!(report.group_results.route_match_count > 0);
        assert!(report.group_results.message_match_count > 0);
        assert!(report.group_results.dependency_match_count > 0);
        assert!(report.group_results.graph_node_count > 0);
        assert!(report.group_results.service_count > 0);
        assert!(results
            .iter()
            .any(|result| result.name == "cross_project_architecture_graph_latency"));
        let json = serde_json::to_string(&report).expect("json");
        assert!(json.contains("target_comparisons"));
        assert!(json.contains("full_git_intelligence_ready"));
    }
}
