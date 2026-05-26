//! Local group query federation for cross-project architecture work.
//!
//! Phase 11.1 reads registry-defined project groups and queries each
//! repo-local `.b3/b3.db` independently. Phase 11.2 adds local/static
//! route/API matching without merging databases or performing runtime calls.

#[path = "architecture/http_clients.rs"]
pub mod http_clients;
#[path = "architecture/match_keys.rs"]
pub mod match_keys;
#[path = "architecture/messaging_keys.rs"]
pub mod messaging_keys;
#[path = "architecture/messaging_matching.rs"]
pub mod messaging_matching;
#[path = "architecture/route_matching.rs"]
pub mod route_matching;

use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

use b3_core::{ArchitectureWarning, ContractError, ContractResult};
use b3_storage::{
    SqliteStorage, StoredComponent, StoredDataAccess, StoredInfrastructure, StoredMessaging,
    StoredRealtime, StoredRoute, StoredWpf,
};
use serde::{Deserialize, Serialize};

pub use messaging_matching::{GroupMessageMatchReport, MessageMatch, MessageMatchOptions};
pub use route_matching::{GroupRouteMatchReport, RouteMatch, RouteMatchOptions};

const DEFAULT_BRANCH: &str = "main";
const DEFAULT_LIMIT: usize = 200;
const MAX_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalRegistry {
    #[serde(default)]
    pub projects: Vec<LocalRegistryProject>,
    #[serde(default)]
    pub groups: Vec<LocalRegistryGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRegistryProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub database: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub last_indexed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRegistryGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub project_ids: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FederatedProjectStatus {
    Ready,
    MissingRegistryProject,
    MissingDb,
    UnreadableDb,
    NotIndexed,
    UnsupportedVersion,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedProjectHandle {
    pub project_id: String,
    pub display_name: String,
    pub root_path: String,
    pub database_path: String,
    pub status: FederatedProjectStatus,
    pub warnings: Vec<ArchitectureWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedQueryContext {
    pub group_id: String,
    pub group_name: String,
    pub registry_path: String,
    pub projects: Vec<FederatedProjectHandle>,
    pub opened_project_count: usize,
    pub skipped_project_count: usize,
    pub warnings: Vec<ArchitectureWarning>,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FederatedMetadataCounts {
    pub routes: usize,
    pub components: usize,
    pub data_access: usize,
    pub realtime: usize,
    pub messaging: usize,
    pub infrastructure: usize,
    pub wpf: usize,
    pub vector_documents: usize,
    pub vectors: usize,
}

impl FederatedMetadataCounts {
    fn add(&mut self, other: &Self) {
        self.routes += other.routes;
        self.components += other.components;
        self.data_access += other.data_access;
        self.realtime += other.realtime;
        self.messaging += other.messaging;
        self.infrastructure += other.infrastructure;
        self.wpf += other.wpf;
        self.vector_documents += other.vector_documents;
        self.vectors += other.vectors;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectArchitectureSummary {
    pub project_id: String,
    pub name: String,
    pub root_path: String,
    pub database_path: String,
    pub status: FederatedProjectStatus,
    pub counts: FederatedMetadataCounts,
    pub semantic_ready: bool,
    pub warnings: Vec<ArchitectureWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupArchitectureSummary {
    pub group_id: String,
    pub group_name: String,
    pub project_count: usize,
    pub ready_project_count: usize,
    pub skipped_project_count: usize,
    pub counts: FederatedMetadataCounts,
    pub semantic_ready_project_count: usize,
    pub projects: Vec<ProjectArchitectureSummary>,
    pub warnings: Vec<ArchitectureWarning>,
    pub local_only: bool,
    pub federation_ready: bool,
    pub matching_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRecord<T> {
    pub project_id: String,
    pub project_name: String,
    pub record: T,
}

pub struct GroupFederation {
    registry_path: PathBuf,
    registry: LocalRegistry,
}

impl GroupFederation {
    pub fn from_registry_path(path: impl Into<PathBuf>) -> ContractResult<Self> {
        let registry_path = path.into();
        let registry = load_local_registry(&registry_path)?;
        Ok(Self {
            registry_path,
            registry,
        })
    }

    pub fn from_default_registry() -> ContractResult<Self> {
        Self::from_registry_path(default_registry_path())
    }

    pub fn groups(&self) -> Vec<LocalRegistryGroup> {
        let mut groups = self.registry.groups.clone();
        groups.sort_by(|left, right| left.id.cmp(&right.id));
        groups
    }

    pub fn resolve_context(&self, group_id: &str) -> ContractResult<FederatedQueryContext> {
        validate_group_id(group_id)?;
        let group = self
            .registry
            .groups
            .iter()
            .find(|group| group.id == group_id || group.name == group_id)
            .ok_or_else(|| ContractError::new(format!("group not found: {group_id}")))?;

        let projects_by_id = self
            .registry
            .projects
            .iter()
            .map(|project| (project.id.as_str(), project))
            .collect::<BTreeMap<_, _>>();

        let mut seen = HashSet::new();
        let mut projects = Vec::new();
        let mut warnings = Vec::new();
        for project_id in &group.project_ids {
            if !seen.insert(project_id.clone()) {
                continue;
            }
            let handle = match projects_by_id.get(project_id.as_str()) {
                Some(project) => self.resolve_project(project),
                None => {
                    let warning = ArchitectureWarning {
                        code: "missing_registry_project".to_string(),
                        message: format!(
                            "project is referenced by group but missing: {project_id}"
                        ),
                        project_id: Some(project_id.clone()),
                    };
                    FederatedProjectHandle {
                        project_id: project_id.clone(),
                        display_name: project_id.clone(),
                        root_path: String::new(),
                        database_path: String::new(),
                        status: FederatedProjectStatus::MissingRegistryProject,
                        warnings: vec![warning],
                    }
                }
            };
            warnings.extend(handle.warnings.clone());
            projects.push(handle);
        }

        projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        let opened_project_count = projects
            .iter()
            .filter(|project| project.status == FederatedProjectStatus::Ready)
            .count();
        let skipped_project_count = projects.len().saturating_sub(opened_project_count);

        Ok(FederatedQueryContext {
            group_id: group.id.clone(),
            group_name: group.name.clone(),
            registry_path: self.registry_path.to_string_lossy().to_string(),
            projects,
            opened_project_count,
            skipped_project_count,
            warnings,
            local_only: true,
        })
    }

    pub fn summary(&self, group_id: &str) -> ContractResult<GroupArchitectureSummary> {
        let context = self.resolve_context(group_id)?;
        let mut counts = FederatedMetadataCounts::default();
        let mut projects = Vec::new();
        let mut semantic_ready_project_count = 0;

        for handle in &context.projects {
            let mut project_summary = ProjectArchitectureSummary {
                project_id: handle.project_id.clone(),
                name: handle.display_name.clone(),
                root_path: handle.root_path.clone(),
                database_path: handle.database_path.clone(),
                status: handle.status,
                counts: FederatedMetadataCounts::default(),
                semantic_ready: false,
                warnings: handle.warnings.clone(),
            };
            if handle.status == FederatedProjectStatus::Ready {
                let storage = open_existing_read_only(handle)?;
                project_summary.counts = project_counts(&storage, &handle.project_id)?;
                project_summary.semantic_ready = project_summary.counts.vector_documents > 0;
                if project_summary.semantic_ready {
                    semantic_ready_project_count += 1;
                }
                counts.add(&project_summary.counts);
            }
            projects.push(project_summary);
        }

        Ok(GroupArchitectureSummary {
            group_id: context.group_id,
            group_name: context.group_name,
            project_count: context.projects.len(),
            ready_project_count: context.opened_project_count,
            skipped_project_count: context.skipped_project_count,
            counts,
            semantic_ready_project_count,
            projects,
            warnings: context.warnings,
            local_only: true,
            federation_ready: true,
            matching_ready: true,
        })
    }

    pub fn routes(
        &self,
        group_id: &str,
        limit: usize,
    ) -> ContractResult<Vec<FederatedRecord<StoredRoute>>> {
        self.collect(
            group_id,
            limit,
            |storage, project_id, limit| {
                storage.routes(project_id, DEFAULT_BRANCH, None, None, None, limit)
            },
            route_sort_key,
        )
    }

    pub fn messaging(
        &self,
        group_id: &str,
        limit: usize,
    ) -> ContractResult<Vec<FederatedRecord<StoredMessaging>>> {
        self.collect(
            group_id,
            limit,
            |storage, project_id, limit| {
                storage.messaging(
                    project_id,
                    DEFAULT_BRANCH,
                    None,
                    None,
                    None,
                    None,
                    None,
                    limit,
                )
            },
            messaging_sort_key,
        )
    }

    pub fn infrastructure(
        &self,
        group_id: &str,
        limit: usize,
    ) -> ContractResult<Vec<FederatedRecord<StoredInfrastructure>>> {
        self.collect(
            group_id,
            limit,
            |storage, project_id, limit| {
                storage.infrastructure(project_id, DEFAULT_BRANCH, None, None, None, limit)
            },
            infrastructure_sort_key,
        )
    }

    pub fn data_access(
        &self,
        group_id: &str,
        limit: usize,
    ) -> ContractResult<Vec<FederatedRecord<StoredDataAccess>>> {
        self.collect(
            group_id,
            limit,
            |storage, project_id, limit| {
                storage.data_access(project_id, DEFAULT_BRANCH, None, None, None, None, limit)
            },
            data_access_sort_key,
        )
    }

    pub fn realtime(
        &self,
        group_id: &str,
        limit: usize,
    ) -> ContractResult<Vec<FederatedRecord<StoredRealtime>>> {
        self.collect(
            group_id,
            limit,
            |storage, project_id, limit| {
                storage.realtime(project_id, DEFAULT_BRANCH, None, None, None, None, limit)
            },
            realtime_sort_key,
        )
    }

    pub fn wpf(
        &self,
        group_id: &str,
        limit: usize,
    ) -> ContractResult<Vec<FederatedRecord<StoredWpf>>> {
        self.collect(
            group_id,
            limit,
            |storage, project_id, limit| {
                storage.wpf(project_id, DEFAULT_BRANCH, None, None, None, limit)
            },
            wpf_sort_key,
        )
    }

    pub fn components(
        &self,
        group_id: &str,
        limit: usize,
    ) -> ContractResult<Vec<FederatedRecord<StoredComponent>>> {
        self.collect(
            group_id,
            limit,
            |storage, project_id, limit| {
                storage.components(project_id, DEFAULT_BRANCH, None, None, None, limit)
            },
            component_sort_key,
        )
    }

    fn collect<T>(
        &self,
        group_id: &str,
        limit: usize,
        read: impl Fn(&SqliteStorage, &str, usize) -> ContractResult<Vec<T>>,
        sort_key: impl Fn(&FederatedRecord<T>) -> String,
    ) -> ContractResult<Vec<FederatedRecord<T>>> {
        let limit = clamp_limit(limit);
        let context = self.resolve_context(group_id)?;
        let mut records = Vec::new();
        for handle in context
            .projects
            .iter()
            .filter(|project| project.status == FederatedProjectStatus::Ready)
        {
            let storage = open_existing_read_only(handle)?;
            for record in read(&storage, &handle.project_id, limit)? {
                records.push(FederatedRecord {
                    project_id: handle.project_id.clone(),
                    project_name: handle.display_name.clone(),
                    record,
                });
            }
        }
        records.sort_by_key(sort_key);
        records.truncate(limit);
        Ok(records)
    }

    fn resolve_project(&self, project: &LocalRegistryProject) -> FederatedProjectHandle {
        let database_path = PathBuf::from(&project.database);
        let root_path = project.path.clone();
        let mut warnings = Vec::new();
        let status = if !database_path.exists() {
            warnings.push(ArchitectureWarning::missing_database(
                project.id.clone(),
                project.database.clone(),
            ));
            FederatedProjectStatus::MissingDb
        } else {
            match SqliteStorage::open_read_only(&database_path) {
                Ok(storage) => {
                    match project_counts(&storage, &project.id) {
                        Ok(counts) if counts == FederatedMetadataCounts::default() => {
                            warnings.push(ArchitectureWarning {
                            code: "not_indexed".to_string(),
                            message: "project database exists but has no indexed architecture metadata".to_string(),
                            project_id: Some(project.id.clone()),
                        });
                            FederatedProjectStatus::NotIndexed
                        }
                        Ok(_) => FederatedProjectStatus::Ready,
                        Err(error) => {
                            warnings.push(ArchitectureWarning {
                                code: "unsupported_or_unreadable_db".to_string(),
                                message: error.to_string(),
                                project_id: Some(project.id.clone()),
                            });
                            FederatedProjectStatus::UnsupportedVersion
                        }
                    }
                }
                Err(error) => {
                    warnings.push(ArchitectureWarning {
                        code: "unreadable_db".to_string(),
                        message: error.to_string(),
                        project_id: Some(project.id.clone()),
                    });
                    FederatedProjectStatus::UnreadableDb
                }
            }
        };

        FederatedProjectHandle {
            project_id: project.id.clone(),
            display_name: project.name.clone(),
            root_path,
            database_path: project.database.clone(),
            status,
            warnings,
        }
    }
}

pub fn default_registry_path() -> PathBuf {
    env::var_os("B3_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".b3")))
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".b3")))
        .unwrap_or_else(|| PathBuf::from(".b3"))
        .join("registry.json")
}

pub fn load_local_registry(path: &Path) -> ContractResult<LocalRegistry> {
    if !path.exists() {
        return Ok(LocalRegistry::default());
    }
    let content = fs::read_to_string(path).map_err(|error| {
        ContractError::new(format!(
            "failed to read registry at {}: {error}",
            path.display()
        ))
    })?;
    serde_json::from_str(&content).map_err(|error| {
        ContractError::new(format!(
            "invalid registry JSON at {}: {error}",
            path.display()
        ))
    })
}

fn open_existing_read_only(handle: &FederatedProjectHandle) -> ContractResult<SqliteStorage> {
    SqliteStorage::open_read_only(&handle.database_path).map_err(|error| {
        ContractError::new(format!(
            "failed to open project {} read-only: {error}",
            handle.project_id
        ))
    })
}

fn project_counts(
    storage: &SqliteStorage,
    project_id: &str,
) -> ContractResult<FederatedMetadataCounts> {
    let routes = storage
        .routes(project_id, DEFAULT_BRANCH, None, None, None, DEFAULT_LIMIT)?
        .len();
    let components = storage
        .components(project_id, DEFAULT_BRANCH, None, None, None, DEFAULT_LIMIT)?
        .len();
    let data_access = storage
        .data_access(
            project_id,
            DEFAULT_BRANCH,
            None,
            None,
            None,
            None,
            DEFAULT_LIMIT,
        )?
        .len();
    let realtime = storage
        .realtime(
            project_id,
            DEFAULT_BRANCH,
            None,
            None,
            None,
            None,
            DEFAULT_LIMIT,
        )?
        .len();
    let messaging = storage
        .messaging(
            project_id,
            DEFAULT_BRANCH,
            None,
            None,
            None,
            None,
            None,
            DEFAULT_LIMIT,
        )?
        .len();
    let infrastructure = storage
        .infrastructure(project_id, DEFAULT_BRANCH, None, None, None, DEFAULT_LIMIT)?
        .len();
    let wpf = storage
        .wpf(project_id, DEFAULT_BRANCH, None, None, None, DEFAULT_LIMIT)?
        .len();
    let vector_stats = storage.vector_stats()?;
    Ok(FederatedMetadataCounts {
        routes,
        components,
        data_access,
        realtime,
        messaging,
        infrastructure,
        wpf,
        vector_documents: vector_stats.documents,
        vectors: vector_stats.vectors,
    })
}

fn clamp_limit(limit: usize) -> usize {
    if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.min(MAX_LIMIT)
    }
}

fn validate_group_id(group_id: &str) -> ContractResult<()> {
    let trimmed = group_id.trim();
    if trimmed.is_empty() {
        return Err(ContractError::new("group id is required"));
    }
    if trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') {
        return Err(ContractError::new(
            "group id must be a local registry id or name",
        ));
    }
    Ok(())
}

fn route_sort_key(record: &FederatedRecord<StoredRoute>) -> String {
    format!(
        "{}\0{}\0{}\0{}",
        record.project_id, record.record.file_path, record.record.path, record.record.id
    )
}

fn messaging_sort_key(record: &FederatedRecord<StoredMessaging>) -> String {
    format!(
        "{}\0{}\0{}",
        record.project_id, record.record.file_path, record.record.id
    )
}

fn infrastructure_sort_key(record: &FederatedRecord<StoredInfrastructure>) -> String {
    format!(
        "{}\0{}\0{}",
        record.project_id, record.record.file_path, record.record.id
    )
}

fn component_sort_key(record: &FederatedRecord<StoredComponent>) -> String {
    format!(
        "{}\0{}\0{}",
        record.project_id, record.record.file_path, record.record.id
    )
}

fn data_access_sort_key(record: &FederatedRecord<StoredDataAccess>) -> String {
    format!(
        "{}\0{}\0{}",
        record.project_id, record.record.file_path, record.record.id
    )
}

fn realtime_sort_key(record: &FederatedRecord<StoredRealtime>) -> String {
    format!(
        "{}\0{}\0{}",
        record.project_id, record.record.file_path, record.record.id
    )
}

fn wpf_sort_key(record: &FederatedRecord<StoredWpf>) -> String {
    format!(
        "{}\0{}\0{}",
        record.project_id, record.record.file_path, record.record.id
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use b3_core::{
        BranchId, BranchMetadata, FileId, FileRecord, NodeKind, ProjectId, SymbolId, SymbolRecord,
    };
    use b3_storage::SqliteStorage;
    use tempfile::TempDir;

    fn write_registry(path: &Path, projects: &[(&str, &str, &Path)], group_projects: &[&str]) {
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
        let project_ids = group_projects
            .iter()
            .map(|id| format!(r#""{id}""#))
            .collect::<Vec<_>>()
            .join(",");
        let registry = format!(
            r#"{{"version":1,"projects":[{projects_json}],"groups":[{{"id":"suite","name":"Suite","description":"","project_ids":[{project_ids}],"tags":[]}}]}}"#
        );
        fs::write(path, registry).expect("registry");
    }

    fn seed_project(db: &Path, project_id: &str, route_path: &str) {
        let storage = SqliteStorage::open(db).expect("storage");
        let project_id_value = ProjectId::new(project_id);
        let branch_id = BranchId::new(DEFAULT_BRANCH);
        storage
            .upsert_project(
                &project_id_value,
                project_id,
                &db.parent().unwrap().to_string_lossy(),
            )
            .expect("project");
        storage
            .upsert_branch(
                &branch_id,
                &project_id_value,
                &BranchMetadata::new(DEFAULT_BRANCH),
            )
            .expect("branch");
        let file = FileRecord {
            id: FileId::new(format!("{project_id}-file")),
            project_id: project_id_value.clone(),
            path: format!("src/{project_id}.ts"),
            content_hash: "hash".to_string(),
        };
        storage.upsert_file(&file, &branch_id).expect("file");
        let mut route = SymbolRecord::new(
            SymbolId::new(format!("{project_id}-route")),
            file.id,
            format!("GET {route_path}"),
            NodeKind::Route,
        );
        route.start_line = 1;
        route.end_line = 2;
        route.visibility = Some(format!(
            "route.framework=express;route.method=GET;route.path={route_path};route.file=src/{project_id}.ts;route.handler=handler;route.source=ExpressCall;route.confidence=9500"
        ));
        storage
            .upsert_symbol(&project_id_value, &branch_id, &route)
            .expect("route");
    }

    #[test]
    fn group_resolution_handles_ready_missing_and_deterministic_order() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        let missing_db = dir.path().join("missing").join(".b3").join("b3.db");
        seed_project(&api_db, "api", "/orders");
        write_registry(
            &registry,
            &[("missing", "Missing", &missing_db), ("api", "API", &api_db)],
            &["missing", "api", "ghost"],
        );

        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let context = federation.resolve_context("suite").expect("context");

        assert!(context.local_only);
        assert_eq!(context.opened_project_count, 1);
        assert_eq!(context.skipped_project_count, 2);
        assert_eq!(context.projects[0].project_id, "api");
        assert_eq!(context.projects[0].status, FederatedProjectStatus::Ready);
        assert_eq!(
            context.projects[1].status,
            FederatedProjectStatus::MissingRegistryProject
        );
        assert_eq!(
            context.projects[2].status,
            FederatedProjectStatus::MissingDb
        );
        assert!(context
            .warnings
            .iter()
            .any(|warning| warning.code == "missing_database"));
    }

    #[test]
    fn summary_and_metadata_helpers_aggregate_without_matching() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        seed_project(&api_db, "api", "/orders");
        seed_project(&web_db, "web", "/users");
        write_registry(
            &registry,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );

        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let summary = federation.summary("suite").expect("summary");
        let routes = federation.routes("suite", 10).expect("routes");

        assert_eq!(summary.ready_project_count, 2);
        assert_eq!(summary.skipped_project_count, 0);
        assert_eq!(summary.counts.routes, 2);
        assert!(summary.federation_ready);
        assert!(summary.matching_ready);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].project_id, "api");
        assert_eq!(routes[1].project_id, "web");
    }

    #[test]
    fn missing_group_returns_structured_error() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        write_registry(&registry, &[], &[]);
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let error = federation
            .resolve_context("missing")
            .expect_err("missing group");
        assert!(error.to_string().contains("group not found"));
    }
}
