use std::collections::{BTreeMap, BTreeSet};

use b3_core::{
    ArchitectureConfidence, ArchitectureConfidenceLevel, ArchitectureEdge, ArchitectureEdgeKind,
    ArchitectureEvidence, ArchitectureEvidenceKind, ArchitectureMatchCandidate, ArchitectureNode,
    ArchitectureNodeKind, ArchitectureSource, ArchitectureSourceKind, ArchitectureWarning,
    ContractResult,
};
use b3_storage::StoredFileContent;
use serde::{Deserialize, Serialize};

use super::{
    contract_matching::{collect_contract_entries, ContractEntry},
    dependency_keys::{
        is_generic_contract_name, normalize_infra_name, ContractKind, InfraKind, PackageEcosystem,
        PackageMatchKey,
    },
    infra_matching::{
        collect_infra_entries, infra_relationships, InfraEntry, InfraRelationshipKind,
    },
    open_existing_read_only, FederatedProjectStatus, FederatedQueryContext, GroupFederation,
    DEFAULT_BRANCH, DEFAULT_LIMIT,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyMatchOptions {
    pub kind: DependencyMatchKindFilter,
    pub ecosystem: Option<String>,
    pub contract_kind: Option<String>,
    pub infra_kind: Option<String>,
    pub name: Option<String>,
    pub source_project_id: Option<String>,
    pub target_project_id: Option<String>,
    pub min_confidence: Option<u16>,
    pub limit: usize,
    pub branch: Option<String>,
}

impl Default for DependencyMatchOptions {
    fn default() -> Self {
        Self {
            kind: DependencyMatchKindFilter::All,
            ecosystem: None,
            contract_kind: None,
            infra_kind: None,
            name: None,
            source_project_id: None,
            target_project_id: None,
            min_confidence: None,
            limit: DEFAULT_LIMIT,
            branch: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyMatchKindFilter {
    Package,
    Contract,
    Infrastructure,
    All,
}

impl DependencyMatchKindFilter {
    pub fn from_query(value: Option<&str>) -> Self {
        match value.unwrap_or("all").trim().to_ascii_lowercase().as_str() {
            "package" | "packages" => Self::Package,
            "contract" | "contracts" | "schema" | "schemas" => Self::Contract,
            "infrastructure" | "infra" => Self::Infrastructure,
            _ => Self::All,
        }
    }

    fn includes(self, kind: DependencyMatchKind) -> bool {
        matches!(self, Self::All)
            || matches!(
                (self, kind),
                (Self::Package, DependencyMatchKind::Package)
                    | (Self::Contract, DependencyMatchKind::Contract)
                    | (Self::Infrastructure, DependencyMatchKind::Infrastructure)
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DependencyMatchKind {
    Package,
    Contract,
    Infrastructure,
}

impl DependencyMatchKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Contract => "contract",
            Self::Infrastructure => "infrastructure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupDependencyMatchReport {
    pub group_id: String,
    pub group_name: String,
    pub matching_kind: String,
    pub match_count: usize,
    pub matches: Vec<DependencyMatch>,
    pub warnings: Vec<ArchitectureWarning>,
    pub local_only: bool,
    pub federation_ready: bool,
    pub dependency_matching_ready: bool,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyMatch {
    pub candidate: ArchitectureMatchCandidate,
    pub edge: ArchitectureEdge,
    pub kind: DependencyMatchKind,
    pub ecosystem: Option<String>,
    pub contract_kind: Option<String>,
    pub infra_kind: Option<String>,
    pub name: String,
    pub match_rule: String,
    pub score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageEntry {
    project_id: String,
    project_name: String,
    key: PackageMatchKey,
    version: Option<String>,
    file_path: String,
    line: usize,
    role: PackageRole,
    evidence: String,
    confidence: ArchitectureConfidence,
    path_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PackageRole {
    Provider,
    Consumer,
}

impl PackageEntry {
    fn source(&self) -> ArchitectureSource {
        ArchitectureSource {
            project_id: self.project_id.clone(),
            file_path: self.file_path.clone(),
            symbol_id: None,
            line_start: Some(self.line),
            line_end: Some(self.line),
            source_kind: ArchitectureSourceKind::Unknown,
            extractor: Some("FileContentPackageScan".to_string()),
            metadata_key: Some("package".to_string()),
        }
    }
}

impl GroupFederation {
    pub fn dependency_matches(
        &self,
        group_id: &str,
        options: DependencyMatchOptions,
    ) -> ContractResult<GroupDependencyMatchReport> {
        let context = self.resolve_context(group_id)?;
        match_dependencies(context, options)
    }
}

fn match_dependencies(
    context: FederatedQueryContext,
    options: DependencyMatchOptions,
) -> ContractResult<GroupDependencyMatchReport> {
    let branch = options
        .branch
        .clone()
        .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
    let limit = if options.limit == 0 {
        DEFAULT_LIMIT
    } else {
        options.limit.min(1_000)
    };
    let mut warnings = context.warnings.clone();
    let mut packages = Vec::new();
    let mut contracts = Vec::new();
    let mut infra = Vec::new();

    for handle in context
        .projects
        .iter()
        .filter(|project| project.status == FederatedProjectStatus::Ready)
    {
        let storage = open_existing_read_only(handle)?;
        let files = storage.file_contents(&handle.project_id, &branch, 2_000)?;
        let components =
            storage.components(&handle.project_id, &branch, None, None, None, 1_000)?;
        let data_access =
            storage.data_access(&handle.project_id, &branch, None, None, None, None, 1_000)?;
        let infrastructure =
            storage.infrastructure(&handle.project_id, &branch, None, None, None, 1_000)?;
        packages.extend(collect_package_entries(
            &handle.project_id,
            &handle.display_name,
            &files,
        ));
        let (contract_entries, contract_warnings) = collect_contract_entries(
            &handle.project_id,
            &handle.display_name,
            &files,
            &components,
            &data_access,
        );
        warnings.extend(contract_warnings);
        contracts.extend(contract_entries);
        infra.extend(collect_infra_entries(
            &handle.project_id,
            &handle.display_name,
            &infrastructure,
        ));
    }

    let mut matches = Vec::new();
    if options.kind.includes(DependencyMatchKind::Package) {
        matches.extend(match_packages(&packages, &options));
    }
    if options.kind.includes(DependencyMatchKind::Contract) {
        matches.extend(match_contracts(&contracts, &options));
    }
    if options.kind.includes(DependencyMatchKind::Infrastructure) {
        matches.extend(match_infrastructure(&infra, &options));
        warnings.extend(unmatched_infra_warnings(&infra));
    }

    let mut seen = BTreeSet::new();
    matches.retain(|matched| {
        seen.insert((
            matched.candidate.id.clone(),
            matched.kind,
            matched.match_rule.clone(),
        ))
    });
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
            .then_with(|| left.name.cmp(&right.name))
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
    });
    matches.truncate(limit);
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

    Ok(GroupDependencyMatchReport {
        group_id: context.group_id,
        group_name: context.group_name,
        matching_kind: "dependency".to_string(),
        match_count: matches.len(),
        matches,
        warnings,
        local_only: true,
        federation_ready: true,
        dependency_matching_ready: true,
        branch,
    })
}

fn collect_package_entries(
    project_id: &str,
    project_name: &str,
    files: &[StoredFileContent],
) -> Vec<PackageEntry> {
    let mut entries = Vec::new();
    for file in files {
        let lower = file.path.to_ascii_lowercase();
        if lower.ends_with("package.json") {
            collect_package_json(project_id, project_name, file, &mut entries);
        } else if lower.ends_with(".csproj") {
            collect_csproj(project_id, project_name, file, &mut entries);
        } else if lower.ends_with("go.mod") {
            collect_go_mod(project_id, project_name, file, &mut entries);
        } else if lower.ends_with("cargo.toml") {
            collect_cargo_toml(project_id, project_name, file, &mut entries);
        }
    }
    entries.sort_by(|left, right| {
        left.key
            .normalized_key
            .cmp(&right.key.normalized_key)
            .then_with(|| left.project_id.cmp(&right.project_id))
            .then_with(|| left.role.cmp(&right.role))
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
    entries.dedup_by(|left, right| {
        left.project_id == right.project_id
            && left.key == right.key
            && left.role == right.role
            && left.file_path == right.file_path
    });
    entries
}

fn collect_package_json(
    project_id: &str,
    project_name: &str,
    file: &StoredFileContent,
    entries: &mut Vec<PackageEntry>,
) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&file.content) else {
        return;
    };
    if let Some(name) = value.get("name").and_then(serde_json::Value::as_str) {
        entries.push(package_entry(
            project_id,
            project_name,
            PackageEcosystem::Npm,
            name,
            value
                .get("version")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            file,
            PackageRole::Provider,
            "package.json name",
            None,
        ));
    }
    for section in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        let Some(object) = value.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for (name, version) in object {
            entries.push(package_entry(
                project_id,
                project_name,
                PackageEcosystem::Npm,
                name,
                version.as_str().map(ToString::to_string),
                file,
                PackageRole::Consumer,
                &format!("package.json {section}"),
                local_path_hint(version.as_str()),
            ));
        }
    }
}

fn collect_csproj(
    project_id: &str,
    project_name: &str,
    file: &StoredFileContent,
    entries: &mut Vec<PackageEntry>,
) {
    let fallback = file
        .path
        .split(['/', '\\'])
        .next_back()
        .unwrap_or(file.path.as_str())
        .trim_end_matches(".csproj");
    let provider = xml_tag_value(&file.content, "PackageId")
        .or_else(|| xml_tag_value(&file.content, "AssemblyName"))
        .or_else(|| xml_tag_value(&file.content, "RootNamespace"))
        .unwrap_or_else(|| fallback.to_string());
    entries.push(package_entry(
        project_id,
        project_name,
        PackageEcosystem::Dotnet,
        &provider,
        xml_tag_value(&file.content, "Version"),
        file,
        PackageRole::Provider,
        ".csproj identity",
        Some(file.path.clone()),
    ));
    for include in xml_include_values(&file.content, "PackageReference") {
        entries.push(package_entry(
            project_id,
            project_name,
            PackageEcosystem::Dotnet,
            &include,
            None,
            file,
            PackageRole::Consumer,
            ".csproj PackageReference",
            None,
        ));
    }
    for include in xml_include_values(&file.content, "ProjectReference") {
        let name = include
            .split(['/', '\\'])
            .next_back()
            .unwrap_or(include.as_str())
            .trim_end_matches(".csproj")
            .to_string();
        entries.push(package_entry(
            project_id,
            project_name,
            PackageEcosystem::Dotnet,
            &name,
            None,
            file,
            PackageRole::Consumer,
            ".csproj ProjectReference",
            Some(include),
        ));
    }
}

fn collect_go_mod(
    project_id: &str,
    project_name: &str,
    file: &StoredFileContent,
    entries: &mut Vec<PackageEntry>,
) {
    for line in file.content.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("module ") {
            entries.push(package_entry(
                project_id,
                project_name,
                PackageEcosystem::Go,
                name.split_whitespace().next().unwrap_or(name),
                None,
                file,
                PackageRole::Provider,
                "go.mod module",
                None,
            ));
        } else if let Some(name) = trimmed.strip_prefix("require ") {
            let name = name.split_whitespace().next().unwrap_or(name);
            if !name.starts_with('(') {
                entries.push(package_entry(
                    project_id,
                    project_name,
                    PackageEcosystem::Go,
                    name,
                    None,
                    file,
                    PackageRole::Consumer,
                    "go.mod require",
                    None,
                ));
            }
        } else if trimmed.starts_with(|ch: char| ch.is_ascii_alphanumeric())
            && file.content.contains("require (")
        {
            let name = trimmed.split_whitespace().next().unwrap_or_default();
            if name.contains('/') {
                entries.push(package_entry(
                    project_id,
                    project_name,
                    PackageEcosystem::Go,
                    name,
                    None,
                    file,
                    PackageRole::Consumer,
                    "go.mod require block",
                    None,
                ));
            }
        }
    }
}

fn collect_cargo_toml(
    project_id: &str,
    project_name: &str,
    file: &StoredFileContent,
    entries: &mut Vec<PackageEntry>,
) {
    let mut section = String::new();
    for line in file.content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed.trim_matches(['[', ']']).to_string();
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim().trim_matches('"');
            let value = value.trim();
            if section == "package" && key == "name" {
                entries.push(package_entry(
                    project_id,
                    project_name,
                    PackageEcosystem::Rust,
                    value.trim_matches('"'),
                    None,
                    file,
                    PackageRole::Provider,
                    "Cargo.toml package name",
                    None,
                ));
            } else if section.ends_with("dependencies") && !key.is_empty() {
                let dependency_name = cargo_dependency_name(key, value);
                entries.push(package_entry(
                    project_id,
                    project_name,
                    PackageEcosystem::Rust,
                    &dependency_name,
                    None,
                    file,
                    PackageRole::Consumer,
                    "Cargo.toml dependency",
                    local_path_hint(Some(value)),
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn package_entry(
    project_id: &str,
    project_name: &str,
    ecosystem: PackageEcosystem,
    name: &str,
    version: Option<String>,
    file: &StoredFileContent,
    role: PackageRole,
    evidence: &str,
    path_hint: Option<String>,
) -> PackageEntry {
    let confidence = match role {
        PackageRole::Provider => ArchitectureConfidence::high(evidence),
        PackageRole::Consumer if path_hint.is_some() => {
            ArchitectureConfidence::high("local path dependency metadata")
        }
        PackageRole::Consumer => ArchitectureConfidence::medium(evidence),
    };
    PackageEntry {
        project_id: project_id.to_string(),
        project_name: project_name.to_string(),
        key: PackageMatchKey::new(ecosystem, name),
        version,
        file_path: file.path.clone(),
        line: 1,
        role,
        evidence: evidence.to_string(),
        confidence,
        path_hint,
    }
}

fn match_packages(
    entries: &[PackageEntry],
    options: &DependencyMatchOptions,
) -> Vec<DependencyMatch> {
    let providers = entries
        .iter()
        .filter(|entry| entry.role == PackageRole::Provider)
        .collect::<Vec<_>>();
    let consumers = entries
        .iter()
        .filter(|entry| entry.role == PackageRole::Consumer)
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    for consumer in consumers {
        if !matches_source_project(&consumer.project_id, options)
            || !matches_package_filters(&consumer.key, options)
        {
            continue;
        }
        let mut matched_provider = false;
        for provider in &providers {
            if consumer.project_id == provider.project_id
                || !matches_target_project(&provider.project_id, options)
                || !matches_package_filters(&provider.key, options)
            {
                continue;
            }
            let Some((confidence, rule, warning)) = score_package_match(consumer, provider) else {
                continue;
            };
            if below_min(&confidence, options) {
                continue;
            }
            matched_provider = true;
            matches.push(build_package_match(
                consumer, provider, confidence, &rule, warning,
            ));
        }
        if !matched_provider && options.target_project_id.is_none() {
            let external = external_package_dependency(consumer);
            if !below_min(&external.candidate.confidence, options) {
                matches.push(external);
            }
        }
    }
    matches
}

fn score_package_match(
    consumer: &PackageEntry,
    provider: &PackageEntry,
) -> Option<(ArchitectureConfidence, String, Option<ArchitectureWarning>)> {
    if consumer.key.ecosystem == provider.key.ecosystem && consumer.key.name == provider.key.name {
        return Some((
            ArchitectureConfidence::high("dependency name matches local package provider")
                .with_evidence(consumer.key.normalized_key.clone())
                .with_evidence(provider.key.normalized_key.clone()),
            "provider_dependency_exact_name".to_string(),
            None,
        ));
    }
    if consumer.key.ecosystem == PackageEcosystem::Dotnet
        && consumer
            .path_hint
            .as_deref()
            .is_some_and(|path| path_matches_provider(path, provider))
    {
        return Some((
            ArchitectureConfidence::high(".NET ProjectReference path matches local project file"),
            "dotnet_project_reference_path".to_string(),
            None,
        ));
    }
    if consumer.key.ecosystem == PackageEcosystem::Go
        && provider.key.ecosystem == PackageEcosystem::Go
        && consumer.key.name.starts_with(&provider.key.name)
    {
        return Some((
            ArchitectureConfidence::new(
                ArchitectureConfidenceLevel::High,
                8_000,
                "Go import/module prefix matches local module",
                vec![
                    consumer.key.normalized_key.clone(),
                    provider.key.normalized_key.clone(),
                ],
            ),
            "go_module_prefix".to_string(),
            None,
        ));
    }
    if consumer.key.ecosystem == PackageEcosystem::Rust
        && provider.key.ecosystem == PackageEcosystem::Rust
        && consumer.key.name.replace('_', "-") == provider.key.name.replace('_', "-")
    {
        return Some((
            ArchitectureConfidence::new(
                ArchitectureConfidenceLevel::High,
                8_000,
                "Rust crate dependency matches local crate name",
                vec![
                    consumer.key.normalized_key.clone(),
                    provider.key.normalized_key.clone(),
                ],
            ),
            "rust_crate_name".to_string(),
            None,
        ));
    }
    if consumer.key.ecosystem == PackageEcosystem::Unknown && consumer.key.name == provider.key.name
    {
        return Some((
            ArchitectureConfidence::medium("unknown ecosystem exact package name match"),
            "unknown_ecosystem_exact_name".to_string(),
            Some(ArchitectureWarning {
                code: "unknown_package_ecosystem".to_string(),
                message: format!(
                    "exact package name match uses unknown ecosystem: {}",
                    consumer.key.name
                ),
                project_id: Some(consumer.project_id.clone()),
            }),
        ));
    }
    None
}

fn match_contracts(
    entries: &[ContractEntry],
    options: &DependencyMatchOptions,
) -> Vec<DependencyMatch> {
    let mut matches = Vec::new();
    for (index, left) in entries.iter().enumerate() {
        if !matches_source_project(&left.project_id, options)
            || !matches_contract_filters(left, options)
        {
            continue;
        }
        for right in entries.iter().skip(index + 1) {
            if left.project_id == right.project_id
                || !matches_target_project(&right.project_id, options)
                || !matches_contract_filters(right, options)
            {
                continue;
            }
            if left.key.normalized_name != right.key.normalized_name {
                continue;
            }
            let generic = is_generic_contract_name(&left.key.name);
            let confidence = if generic {
                ArchitectureConfidence::low("generic contract/type name matched across projects")
            } else if left.key.kind == right.key.kind {
                ArchitectureConfidence::high("same contract kind and exact normalized name")
            } else {
                ArchitectureConfidence::medium("exact contract/type name matched across projects")
            };
            if below_min(&confidence, options) {
                continue;
            }
            let rule = if generic {
                "generic_exact_name"
            } else if matches!(
                left.key.kind,
                ContractKind::OpenApi
                    | ContractKind::JsonSchema
                    | ContractKind::Graphql
                    | ContractKind::Protobuf
                    | ContractKind::Avro
            ) {
                "schema_name_match"
            } else {
                "exact_contract_name"
            };
            matches.push(build_contract_match(left, right, confidence, rule));
        }
    }
    matches
}

fn match_infrastructure(
    entries: &[InfraEntry],
    options: &DependencyMatchOptions,
) -> Vec<DependencyMatch> {
    infra_relationships(entries)
        .into_iter()
        .filter(|relationship| {
            matches_source_project(&relationship.source.project_id, options)
                && matches_target_project(&relationship.target.project_id, options)
                && matches_infra_filters(&relationship.source, options)
                && !below_min(&relationship.confidence, options)
        })
        .map(|relationship| {
            let edge_kind = match relationship.relationship {
                InfraRelationshipKind::DependsOn => ArchitectureEdgeKind::DependsOnInfrastructure,
                InfraRelationshipKind::Selects => ArchitectureEdgeKind::SelectsService,
                InfraRelationshipKind::Deploys => ArchitectureEdgeKind::DeploysService,
                InfraRelationshipKind::References => ArchitectureEdgeKind::DependsOnInfrastructure,
                InfraRelationshipKind::Defines => {
                    ArchitectureEdgeKind::DefinesInfrastructureResource
                }
            };
            build_infra_match(
                &relationship.source,
                &relationship.target,
                relationship.confidence,
                edge_kind,
                &relationship.match_rule,
            )
        })
        .collect()
}

fn build_package_match(
    consumer: &PackageEntry,
    provider: &PackageEntry,
    confidence: ArchitectureConfidence,
    rule: &str,
    warning: Option<ArchitectureWarning>,
) -> DependencyMatch {
    let left_node = package_node(consumer, "consumer");
    let right_node = package_node(provider, "provider");
    let normalized_key = format!(
        "{}=>{}",
        consumer.key.normalized_key, provider.key.normalized_key
    );
    let evidence = vec![
        evidence(
            "consumer package key",
            &consumer.key.normalized_key,
            consumer.source(),
        ),
        evidence(
            "provider package key",
            &provider.key.normalized_key,
            provider.source(),
        ),
    ];
    let edge = edge(
        &left_node,
        &right_node,
        ArchitectureEdgeKind::DependsOnPackage,
        confidence.clone(),
        evidence.clone(),
        BTreeMap::from([
            ("match_rule".to_string(), rule.to_string()),
            (
                "ecosystem".to_string(),
                consumer.key.ecosystem.as_str().to_string(),
            ),
        ]),
    );
    DependencyMatch {
        candidate: ArchitectureMatchCandidate {
            id: ArchitectureMatchCandidate::deterministic_id(
                &consumer.project_id,
                Some(&provider.project_id),
                ArchitectureEdgeKind::DependsOnPackage,
                &normalized_key,
            ),
            left_project_id: consumer.project_id.clone(),
            right_project_id: Some(provider.project_id.clone()),
            left_node,
            right_node: Some(right_node),
            relationship_kind: ArchitectureEdgeKind::DependsOnPackage,
            match_key: consumer.key.normalized_key.clone(),
            normalized_key,
            confidence: confidence.clone(),
            evidence,
            warnings: warning.into_iter().collect(),
        },
        edge,
        kind: DependencyMatchKind::Package,
        ecosystem: Some(consumer.key.ecosystem.as_str().to_string()),
        contract_kind: None,
        infra_kind: None,
        name: consumer.key.name.clone(),
        match_rule: rule.to_string(),
        score: confidence.score,
    }
}

fn external_package_dependency(consumer: &PackageEntry) -> DependencyMatch {
    let left_node = package_node(consumer, "consumer");
    let right_node = package_node(consumer, "external");
    let normalized_key = format!("{}=>external", consumer.key.normalized_key);
    let confidence =
        ArchitectureConfidence::low("shared/external dependency with no local provider");
    let evidence = vec![evidence(
        "consumer package key",
        &consumer.key.normalized_key,
        consumer.source(),
    )];
    let edge = edge(
        &left_node,
        &right_node,
        ArchitectureEdgeKind::ImportsPackage,
        confidence.clone(),
        evidence.clone(),
        BTreeMap::from([("match_rule".to_string(), "external_dependency".to_string())]),
    );
    DependencyMatch {
        candidate: ArchitectureMatchCandidate {
            id: ArchitectureMatchCandidate::deterministic_id(
                &consumer.project_id,
                None,
                ArchitectureEdgeKind::ImportsPackage,
                &normalized_key,
            ),
            left_project_id: consumer.project_id.clone(),
            right_project_id: None,
            left_node,
            right_node: Some(right_node),
            relationship_kind: ArchitectureEdgeKind::ImportsPackage,
            match_key: consumer.key.normalized_key.clone(),
            normalized_key,
            confidence: confidence.clone(),
            evidence,
            warnings: vec![ArchitectureWarning {
                code: "no_local_package_provider".to_string(),
                message: format!("no local provider matched dependency {}", consumer.key.name),
                project_id: Some(consumer.project_id.clone()),
            }],
        },
        edge,
        kind: DependencyMatchKind::Package,
        ecosystem: Some(consumer.key.ecosystem.as_str().to_string()),
        contract_kind: None,
        infra_kind: None,
        name: consumer.key.name.clone(),
        match_rule: "external_dependency".to_string(),
        score: confidence.score,
    }
}

fn build_contract_match(
    left: &ContractEntry,
    right: &ContractEntry,
    confidence: ArchitectureConfidence,
    rule: &str,
) -> DependencyMatch {
    let left_node = contract_node(left);
    let right_node = contract_node(right);
    let normalized_key = format!("{}=>{}", left.key.normalized_key, right.key.normalized_key);
    let evidence = vec![
        evidence("left contract key", &left.key.normalized_key, left.source()),
        evidence(
            "right contract key",
            &right.key.normalized_key,
            right.source(),
        ),
    ];
    let edge_kind = if rule == "generic_exact_name" {
        ArchitectureEdgeKind::UsesContract
    } else {
        ArchitectureEdgeKind::SharesContract
    };
    let edge = edge(
        &left_node,
        &right_node,
        edge_kind,
        confidence.clone(),
        evidence.clone(),
        BTreeMap::from([
            ("match_rule".to_string(), rule.to_string()),
            (
                "contract_kind".to_string(),
                left.key.kind.as_str().to_string(),
            ),
        ]),
    );
    DependencyMatch {
        candidate: ArchitectureMatchCandidate {
            id: ArchitectureMatchCandidate::deterministic_id(
                &left.project_id,
                Some(&right.project_id),
                edge_kind,
                &normalized_key,
            ),
            left_project_id: left.project_id.clone(),
            right_project_id: Some(right.project_id.clone()),
            left_node,
            right_node: Some(right_node),
            relationship_kind: edge_kind,
            match_key: left.key.normalized_key.clone(),
            normalized_key,
            confidence: confidence.clone(),
            evidence,
            warnings: Vec::new(),
        },
        edge,
        kind: DependencyMatchKind::Contract,
        ecosystem: None,
        contract_kind: Some(left.key.kind.as_str().to_string()),
        infra_kind: None,
        name: left.key.name.clone(),
        match_rule: rule.to_string(),
        score: confidence.score,
    }
}

fn build_infra_match(
    source: &InfraEntry,
    target: &InfraEntry,
    confidence: ArchitectureConfidence,
    edge_kind: ArchitectureEdgeKind,
    rule: &str,
) -> DependencyMatch {
    let left_node = infra_node(source);
    let right_node = infra_node(target);
    let normalized_key = format!(
        "{}=>{}",
        source.key.normalized_key, target.key.normalized_key
    );
    let evidence = vec![
        evidence(
            "source infrastructure key",
            &source.key.normalized_key,
            source.source(),
        ),
        evidence(
            "target infrastructure key",
            &target.key.normalized_key,
            target.source(),
        ),
    ];
    let edge = edge(
        &left_node,
        &right_node,
        edge_kind,
        confidence.clone(),
        evidence.clone(),
        BTreeMap::from([
            ("match_rule".to_string(), rule.to_string()),
            (
                "infra_kind".to_string(),
                source.key.kind.as_str().to_string(),
            ),
        ]),
    );
    DependencyMatch {
        candidate: ArchitectureMatchCandidate {
            id: ArchitectureMatchCandidate::deterministic_id(
                &source.project_id,
                Some(&target.project_id),
                edge_kind,
                &normalized_key,
            ),
            left_project_id: source.project_id.clone(),
            right_project_id: Some(target.project_id.clone()),
            left_node,
            right_node: Some(right_node),
            relationship_kind: edge_kind,
            match_key: source.key.normalized_key.clone(),
            normalized_key,
            confidence: confidence.clone(),
            evidence,
            warnings: Vec::new(),
        },
        edge,
        kind: DependencyMatchKind::Infrastructure,
        ecosystem: None,
        contract_kind: None,
        infra_kind: Some(source.key.kind.as_str().to_string()),
        name: source.key.name.clone(),
        match_rule: rule.to_string(),
        score: confidence.score,
    }
}

fn package_node(entry: &PackageEntry, role: &str) -> ArchitectureNode {
    let name = format!("{} {}", entry.key.ecosystem.as_str(), entry.key.name);
    ArchitectureNode {
        id: ArchitectureNode::deterministic_id(
            &entry.project_id,
            None,
            ArchitectureNodeKind::Package,
            &format!("{role}:{name}"),
            Some(&entry.file_path),
            None,
        ),
        project_id: entry.project_id.clone(),
        service_id: None,
        kind: ArchitectureNodeKind::Package,
        name: entry.key.name.clone(),
        label: name,
        path: Some(entry.file_path.clone()),
        symbol_id: None,
        metadata: BTreeMap::from([
            ("role".to_string(), role.to_string()),
            (
                "ecosystem".to_string(),
                entry.key.ecosystem.as_str().to_string(),
            ),
            ("project_name".to_string(), entry.project_name.clone()),
        ]),
        confidence: entry.confidence.clone(),
        sources: vec![entry.source()],
    }
}

fn contract_node(entry: &ContractEntry) -> ArchitectureNode {
    ArchitectureNode {
        id: ArchitectureNode::deterministic_id(
            &entry.project_id,
            None,
            ArchitectureNodeKind::Contract,
            &entry.key.normalized_key,
            Some(&entry.file_path),
            entry.symbol_id.as_deref(),
        ),
        project_id: entry.project_id.clone(),
        service_id: None,
        kind: ArchitectureNodeKind::Contract,
        name: entry.key.name.clone(),
        label: format!("{} {}", entry.key.kind.as_str(), entry.key.name),
        path: Some(entry.file_path.clone()),
        symbol_id: entry.symbol_id.clone(),
        metadata: BTreeMap::from([
            (
                "contract_kind".to_string(),
                entry.key.kind.as_str().to_string(),
            ),
            ("project_name".to_string(), entry.project_name.clone()),
        ]),
        confidence: entry.confidence.clone(),
        sources: vec![entry.source()],
    }
}

fn infra_node(entry: &InfraEntry) -> ArchitectureNode {
    ArchitectureNode {
        id: ArchitectureNode::deterministic_id(
            &entry.project_id,
            None,
            ArchitectureNodeKind::InfrastructureResource,
            &entry.key.normalized_key,
            Some(&entry.record.file_path),
            Some(&entry.record.symbol_id),
        ),
        project_id: entry.project_id.clone(),
        service_id: None,
        kind: ArchitectureNodeKind::InfrastructureResource,
        name: entry.key.name.clone(),
        label: format!("{} {}", entry.key.kind.as_str(), entry.key.name),
        path: Some(entry.record.file_path.clone()),
        symbol_id: Some(entry.record.symbol_id.clone()),
        metadata: BTreeMap::from([
            (
                "infra_kind".to_string(),
                entry.key.kind.as_str().to_string(),
            ),
            ("technology".to_string(), entry.record.technology.clone()),
            ("project_name".to_string(), entry.project_name.clone()),
        ]),
        confidence: entry.confidence(),
        sources: vec![entry.source()],
    }
}

fn edge(
    left: &ArchitectureNode,
    right: &ArchitectureNode,
    kind: ArchitectureEdgeKind,
    confidence: ArchitectureConfidence,
    evidence: Vec<ArchitectureEvidence>,
    metadata: BTreeMap<String, String>,
) -> ArchitectureEdge {
    ArchitectureEdge {
        id: ArchitectureEdge::deterministic_id(&left.id, &right.id, kind),
        from_node_id: left.id.clone(),
        to_node_id: right.id.clone(),
        kind,
        confidence,
        evidence: evidence.clone(),
        sources: left
            .sources
            .iter()
            .chain(right.sources.iter())
            .cloned()
            .collect(),
        metadata,
    }
}

fn evidence(description: &str, value: &str, source: ArchitectureSource) -> ArchitectureEvidence {
    ArchitectureEvidence {
        kind: ArchitectureEvidenceKind::NormalizedKey,
        description: description.to_string(),
        value: Some(value.to_string()),
        source: Some(source),
    }
}

fn matches_source_project(project_id: &str, options: &DependencyMatchOptions) -> bool {
    options
        .source_project_id
        .as_ref()
        .is_none_or(|filter| filter == project_id)
}

fn matches_target_project(project_id: &str, options: &DependencyMatchOptions) -> bool {
    options
        .target_project_id
        .as_ref()
        .is_none_or(|filter| filter == project_id)
}

fn matches_package_filters(key: &PackageMatchKey, options: &DependencyMatchOptions) -> bool {
    if let Some(ecosystem) = &options.ecosystem {
        if key.ecosystem != PackageEcosystem::from_filter(ecosystem) {
            return false;
        }
    }
    matches_name_filter(&key.name, options)
}

fn matches_contract_filters(entry: &ContractEntry, options: &DependencyMatchOptions) -> bool {
    if let Some(kind) = &options.contract_kind {
        if entry.key.kind != ContractKind::from_filter(kind) {
            return false;
        }
    }
    matches_name_filter(&entry.key.normalized_name, options)
}

fn matches_infra_filters(entry: &InfraEntry, options: &DependencyMatchOptions) -> bool {
    if let Some(kind) = &options.infra_kind {
        if entry.key.kind != InfraKind::from_filter(kind) {
            return false;
        }
    }
    matches_name_filter(&entry.key.name, options)
}

fn matches_name_filter(name: &str, options: &DependencyMatchOptions) -> bool {
    options.name.as_ref().is_none_or(|filter| {
        let normalized = normalize_infra_name(filter);
        name == normalized || name.contains(&normalized)
    })
}

fn below_min(confidence: &ArchitectureConfidence, options: &DependencyMatchOptions) -> bool {
    options
        .min_confidence
        .is_some_and(|minimum| confidence.score < minimum)
}

fn local_path_hint(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    value
        .strip_prefix("file:")
        .or_else(|| value.strip_prefix("path="))
        .map(|path| path.trim_matches(['"', '\'', '{', '}', ' ']).to_string())
        .filter(|path| !path.is_empty())
}

fn path_matches_provider(path: &str, provider: &PackageEntry) -> bool {
    let path_file = path
        .split(['/', '\\'])
        .next_back()
        .unwrap_or(path)
        .trim_end_matches(".csproj")
        .to_ascii_lowercase();
    let provider_file = provider
        .file_path
        .split(['/', '\\'])
        .next_back()
        .unwrap_or(provider.file_path.as_str())
        .trim_end_matches(".csproj")
        .to_ascii_lowercase();
    path_file == provider_file || path_file == provider.key.name
}

fn xml_tag_value(source: &str, tag: &str) -> Option<String> {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    let start_index = source.find(&start)? + start.len();
    let end_index = source[start_index..].find(&end)? + start_index;
    Some(source[start_index..end_index].trim().to_string()).filter(|value| !value.is_empty())
}

fn xml_include_values(source: &str, tag: &str) -> Vec<String> {
    source
        .split('<')
        .filter(|part| part.trim_start().starts_with(tag))
        .filter_map(|part| {
            part.split("Include=")
                .nth(1)
                .and_then(|rest| quoted_attr_value(rest).or_else(|| unquoted_attr_value(rest)))
        })
        .collect()
}

fn quoted_attr_value(rest: &str) -> Option<String> {
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let after = &rest[1..];
    let end = after.find(quote)?;
    Some(after[..end].to_string())
}

fn unquoted_attr_value(rest: &str) -> Option<String> {
    Some(
        rest.split_whitespace()
            .next()?
            .trim_matches(['"', '\'', '/', '>'])
            .to_string(),
    )
}

fn cargo_dependency_name(key: &str, value: &str) -> String {
    if value.contains("package") {
        for segment in value.split(',') {
            let segment = segment.trim().trim_matches(['{', '}']);
            if let Some((field, package)) = segment.split_once('=') {
                if field.trim() == "package" {
                    return package.trim().trim_matches('"').to_string();
                }
            }
        }
    }
    key.to_string()
}

fn unmatched_infra_warnings(entries: &[InfraEntry]) -> Vec<ArchitectureWarning> {
    entries
        .iter()
        .filter(|entry| {
            entry.record.technology == "docker_compose"
                && entry.record.selectors.iter().any(|dependency| {
                    let dependency = normalize_infra_name(dependency);
                    !entries
                        .iter()
                        .any(|candidate| candidate.key.name == dependency)
                })
        })
        .map(|entry| ArchitectureWarning {
            code: "unmatched_infra_reference".to_string(),
            message: format!(
                "infrastructure metadata references a service/resource not found locally: {}",
                entry.record.selectors.join(",")
            ),
            project_id: Some(entry.project_id.clone()),
        })
        .collect()
}

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
                serde_json::to_string(&LocalRegistryProject {
                    id: id.to_string(),
                    name: name.to_string(),
                    path: db.parent().unwrap().display().to_string(),
                    database: db.display().to_string(),
                    tags: Vec::new(),
                    last_indexed_at: None,
                })
                .expect("project json")
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

    fn seed_file(
        db: &Path,
        project_id: &str,
        file_path: &str,
        content: &str,
        symbols: Vec<SymbolRecord>,
    ) {
        let storage = SqliteStorage::open(db).expect("storage");
        let project = ProjectId::new(project_id);
        let branch = BranchId::new(DEFAULT_BRANCH);
        storage
            .ensure_project_branch(&project, &branch, &db.parent().unwrap().to_string_lossy())
            .expect("project");
        storage
            .upsert_indexed_file(
                &project,
                &branch,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new(format!("{project_id}-{file_path}")),
                        project_id: project.clone(),
                        path: file_path.to_string(),
                        content_hash: format!("hash-{project_id}-{file_path}"),
                    },
                    language: Some(
                        file_path
                            .split('.')
                            .next_back()
                            .unwrap_or("txt")
                            .to_string(),
                    ),
                    size_bytes: content.len() as u64,
                    content: content.to_string(),
                    symbols,
                    edges: Vec::new(),
                },
            )
            .expect("indexed");
    }

    fn infra_symbol(project_id: &str, index: usize, metadata: &str) -> SymbolRecord {
        let mut symbol = SymbolRecord::new(
            SymbolId::new(format!("{project_id}-infra-{index}")),
            FileId::new(format!("{project_id}-deploy.yaml")),
            format!("infra {index}"),
            NodeKind::Endpoint,
        );
        symbol.start_line = index + 1;
        symbol.end_line = index + 1;
        symbol.visibility = Some(metadata.to_string());
        symbol
    }

    #[test]
    fn matches_local_package_provider_and_marks_external_dependency_low() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let shared_db = dir.path().join("shared").join(".b3").join("b3.db");
        let app_db = dir.path().join("app").join(".b3").join("b3.db");
        seed_file(
            &shared_db,
            "shared",
            "package.json",
            r#"{"name":"shared-contracts","version":"1.0.0"}"#,
            Vec::new(),
        );
        seed_file(
            &app_db,
            "app",
            "package.json",
            r#"{"name":"app","dependencies":{"shared-contracts":"file:../shared","react":"18"}}"#,
            Vec::new(),
        );
        write_registry(
            &registry,
            &[("shared", "Shared", &shared_db), ("app", "App", &app_db)],
            &["shared", "app"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let report = federation
            .dependency_matches(
                "suite",
                DependencyMatchOptions {
                    kind: DependencyMatchKindFilter::Package,
                    ..DependencyMatchOptions::default()
                },
            )
            .expect("matches");

        assert!(report.dependency_matching_ready);
        assert!(report.matches.iter().any(|matched| {
            matched.name == "shared-contracts"
                && matched.candidate.relationship_kind == ArchitectureEdgeKind::DependsOnPackage
                && matched.score >= 9_000
        }));
        assert!(report.matches.iter().any(|matched| {
            matched.name == "react"
                && matched.candidate.relationship_kind == ArchitectureEdgeKind::ImportsPackage
                && matched.score <= 3_000
        }));
    }

    #[test]
    fn matches_contracts_and_keeps_generic_names_low_confidence() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        seed_file(
            &api_db,
            "api",
            "src/contracts.ts",
            "export interface CreateOrderRequest {}\nexport interface User {}",
            Vec::new(),
        );
        seed_file(
            &web_db,
            "web",
            "src/contracts.ts",
            "export type CreateOrderRequest = {}\nexport interface User {}",
            Vec::new(),
        );
        write_registry(
            &registry,
            &[("api", "API", &api_db), ("web", "Web", &web_db)],
            &["api", "web"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let report = federation
            .dependency_matches(
                "suite",
                DependencyMatchOptions {
                    kind: DependencyMatchKindFilter::Contract,
                    ..DependencyMatchOptions::default()
                },
            )
            .expect("matches");

        assert!(report
            .matches
            .iter()
            .any(|matched| matched.name == "CreateOrderRequest" && matched.score >= 6_000));
        assert!(report
            .matches
            .iter()
            .any(|matched| matched.name == "User" && matched.score <= 3_000));
    }

    #[test]
    fn matches_infrastructure_compose_and_kubernetes_relationships() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let app_db = dir.path().join("app").join(".b3").join("b3.db");
        let infra_db = dir.path().join("infra").join(".b3").join("b3.db");
        seed_file(
            &app_db,
            "app",
            "deploy.yaml",
            "infra",
            vec![
                infra_symbol(
                    "app",
                    0,
                    "infrastructure.technology=docker_compose;infrastructure.kind=Service;infrastructure.name=api;infrastructure.service_name=api;infrastructure.selectors=db;infrastructure.file=docker-compose.yml;infrastructure.source=ComposeService;infrastructure.line_start=1;infrastructure.line_end=5;infrastructure.confidence=9000",
                ),
                infra_symbol(
                    "app",
                    1,
                    "infrastructure.technology=docker_compose;infrastructure.kind=Service;infrastructure.name=db;infrastructure.service_name=db;infrastructure.file=docker-compose.yml;infrastructure.source=ComposeService;infrastructure.line_start=6;infrastructure.line_end=9;infrastructure.confidence=9000",
                ),
            ],
        );
        seed_file(
            &infra_db,
            "infra",
            "deploy.yaml",
            "infra",
            vec![
                infra_symbol(
                    "infra",
                    0,
                    "infrastructure.technology=kubernetes;infrastructure.kind=Service;infrastructure.name=orders-api;infrastructure.resource_type=Service;infrastructure.namespace=default;infrastructure.selectors=app=orders-api;infrastructure.file=k8s.yaml;infrastructure.source=KubernetesService;infrastructure.line_start=1;infrastructure.line_end=5;infrastructure.confidence=9000",
                ),
                infra_symbol(
                    "infra",
                    1,
                    "infrastructure.technology=kubernetes;infrastructure.kind=Deployment;infrastructure.name=orders-api;infrastructure.resource_type=Deployment;infrastructure.namespace=default;infrastructure.labels=app=orders-api;infrastructure.image=orders-api:latest;infrastructure.file=k8s.yaml;infrastructure.source=KubernetesDeployment;infrastructure.line_start=6;infrastructure.line_end=15;infrastructure.confidence=9000",
                ),
            ],
        );
        write_registry(
            &registry,
            &[("app", "App", &app_db), ("infra", "Infra", &infra_db)],
            &["app", "infra"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let report = federation
            .dependency_matches(
                "suite",
                DependencyMatchOptions {
                    kind: DependencyMatchKindFilter::Infrastructure,
                    ..DependencyMatchOptions::default()
                },
            )
            .expect("matches");

        assert!(report
            .matches
            .iter()
            .any(|matched| matched.match_rule == "compose_depends_on"));
        assert!(report
            .matches
            .iter()
            .any(|matched| matched.match_rule == "k8s_selector_labels"));
        assert!(report.matches.iter().any(|matched| {
            matched.candidate.relationship_kind == ArchitectureEdgeKind::SelectsService
        }));
    }
}
