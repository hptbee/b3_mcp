use b3_core::{
    ArchitectureConfidence, ArchitectureSource, ArchitectureSourceKind, ArchitectureWarning,
};
use b3_storage::{StoredComponent, StoredDataAccess, StoredFileContent};

use super::dependency_keys::{is_generic_contract_name, ContractKind, ContractMatchKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractEntry {
    pub project_id: String,
    pub project_name: String,
    pub key: ContractMatchKey,
    pub file_path: String,
    pub symbol_id: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub source_kind: ArchitectureSourceKind,
    pub extractor: String,
    pub confidence: ArchitectureConfidence,
    pub role_hint: ContractRoleHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractRoleHint {
    Definition,
    Reference,
}

impl ContractEntry {
    pub fn source(&self) -> ArchitectureSource {
        ArchitectureSource {
            project_id: self.project_id.clone(),
            file_path: self.file_path.clone(),
            symbol_id: self.symbol_id.clone(),
            line_start: Some(self.line_start),
            line_end: Some(self.line_end),
            source_kind: self.source_kind,
            extractor: Some(self.extractor.clone()),
            metadata_key: Some("contract".to_string()),
        }
    }
}

pub fn collect_contract_entries(
    project_id: &str,
    project_name: &str,
    files: &[StoredFileContent],
    components: &[StoredComponent],
    data_access: &[StoredDataAccess],
) -> (Vec<ContractEntry>, Vec<ArchitectureWarning>) {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for file in files {
        entries.extend(contract_entries_from_file(project_id, project_name, file));
    }
    for component in components {
        if let Some(props) = component.props_type_name.as_deref() {
            entries.push(ContractEntry {
                project_id: project_id.to_string(),
                project_name: project_name.to_string(),
                key: ContractMatchKey::new(classify_contract_name(props), props),
                file_path: component.file_path.clone(),
                symbol_id: Some(component.symbol_id.clone()),
                line_start: component.line_start,
                line_end: component.line_end,
                source_kind: ArchitectureSourceKind::ComponentMetadata,
                extractor: component.source_kind.clone(),
                confidence: ArchitectureConfidence::medium("React props type metadata"),
                role_hint: ContractRoleHint::Reference,
            });
        }
    }
    for record in data_access {
        for name in [
            record.entity_name.as_deref(),
            record.context_name.as_deref(),
            record.repository_name.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            entries.push(ContractEntry {
                project_id: project_id.to_string(),
                project_name: project_name.to_string(),
                key: ContractMatchKey::new(classify_contract_name(name), name),
                file_path: record.file_path.clone(),
                symbol_id: Some(record.symbol_id.clone()),
                line_start: record.line_start,
                line_end: record.line_end,
                source_kind: ArchitectureSourceKind::DataAccessMetadata,
                extractor: record.source_kind.clone(),
                confidence: ArchitectureConfidence::low("data-access model/entity metadata"),
                role_hint: ContractRoleHint::Reference,
            });
        }
    }
    entries.sort_by(|left, right| {
        left.key
            .normalized_key
            .cmp(&right.key.normalized_key)
            .then_with(|| left.project_id.cmp(&right.project_id))
            .then_with(|| left.file_path.cmp(&right.file_path))
            .then_with(|| left.line_start.cmp(&right.line_start))
    });
    entries.dedup_by(|left, right| {
        left.project_id == right.project_id
            && left.key == right.key
            && left.file_path == right.file_path
            && left.line_start == right.line_start
    });
    if entries.is_empty() && !files.is_empty() {
        warnings.push(ArchitectureWarning {
            code: "no_contract_metadata".to_string(),
            message: "no local contract/schema names found in indexed files".to_string(),
            project_id: Some(project_id.to_string()),
        });
    }
    (entries, warnings)
}

fn contract_entries_from_file(
    project_id: &str,
    project_name: &str,
    file: &StoredFileContent,
) -> Vec<ContractEntry> {
    let mut entries = Vec::new();
    let lower_path = file.path.to_ascii_lowercase();
    if lower_path.ends_with(".schema.json") {
        entries.push(file_contract_entry(
            project_id,
            project_name,
            file,
            ContractKind::JsonSchema,
            file.path
                .split(['/', '\\'])
                .next_back()
                .unwrap_or(file.path.as_str()),
            1,
        ));
    } else if lower_path.ends_with(".graphql") || lower_path.ends_with(".gql") {
        collect_keyword_names(
            project_id,
            project_name,
            file,
            &[
                ("type ", ContractKind::Graphql),
                ("interface ", ContractKind::Graphql),
                ("enum ", ContractKind::Graphql),
            ],
            &mut entries,
        );
    } else if lower_path.ends_with(".proto") {
        collect_keyword_names(
            project_id,
            project_name,
            file,
            &[
                ("message ", ContractKind::Protobuf),
                ("enum ", ContractKind::Protobuf),
            ],
            &mut entries,
        );
    } else if lower_path.ends_with(".avsc") {
        entries.push(file_contract_entry(
            project_id,
            project_name,
            file,
            ContractKind::Avro,
            file.path
                .split(['/', '\\'])
                .next_back()
                .unwrap_or(file.path.as_str()),
            1,
        ));
    } else if lower_path.ends_with("openapi.json")
        || lower_path.ends_with("openapi.yaml")
        || lower_path.ends_with("swagger.json")
        || lower_path.ends_with("swagger.yaml")
    {
        entries.push(file_contract_entry(
            project_id,
            project_name,
            file,
            ContractKind::OpenApi,
            file.path
                .split(['/', '\\'])
                .next_back()
                .unwrap_or(file.path.as_str()),
            1,
        ));
    }

    collect_keyword_names(
        project_id,
        project_name,
        file,
        &[
            ("interface ", ContractKind::Interface),
            ("type ", ContractKind::Type),
            ("enum ", ContractKind::Enum),
            ("class ", ContractKind::Model),
            ("record ", ContractKind::Model),
            ("struct ", ContractKind::Model),
        ],
        &mut entries,
    );
    entries
}

fn collect_keyword_names(
    project_id: &str,
    project_name: &str,
    file: &StoredFileContent,
    patterns: &[(&str, ContractKind)],
    entries: &mut Vec<ContractEntry>,
) {
    for (line_index, line) in file.content.lines().enumerate() {
        let trimmed = line.trim_start();
        for (pattern, kind) in patterns {
            let Some(index) = trimmed.find(pattern) else {
                continue;
            };
            let before = trimmed[..index].trim_end();
            if !before.is_empty()
                && !before.ends_with("export")
                && !before.ends_with("public")
                && !before.ends_with("private")
                && !before.ends_with("internal")
                && !before.ends_with("declare")
            {
                continue;
            }
            let rest = &trimmed[index + pattern.len()..];
            let name = rest
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
                .collect::<String>();
            if name.len() < 2 {
                continue;
            }
            entries.push(file_contract_entry(
                project_id,
                project_name,
                file,
                classify_declared_kind(*kind, &name),
                &name,
                line_index + 1,
            ));
        }
    }
}

fn file_contract_entry(
    project_id: &str,
    project_name: &str,
    file: &StoredFileContent,
    kind: ContractKind,
    name: &str,
    line: usize,
) -> ContractEntry {
    let generic = is_generic_contract_name(name);
    ContractEntry {
        project_id: project_id.to_string(),
        project_name: project_name.to_string(),
        key: ContractMatchKey::new(kind, name),
        file_path: file.path.clone(),
        symbol_id: None,
        line_start: line,
        line_end: line,
        source_kind: ArchitectureSourceKind::Unknown,
        extractor: "FileContentContractScan".to_string(),
        confidence: if generic {
            ArchitectureConfidence::low("generic contract/type name")
        } else {
            ArchitectureConfidence::medium("local contract/type declaration")
        },
        role_hint: ContractRoleHint::Definition,
    }
}

fn classify_declared_kind(kind: ContractKind, name: &str) -> ContractKind {
    if matches!(kind, ContractKind::Model | ContractKind::Type) {
        classify_contract_name(name)
    } else {
        kind
    }
}

fn classify_contract_name(name: &str) -> ContractKind {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with("dto") {
        ContractKind::Dto
    } else if lower.ends_with("request") || lower.ends_with("response") {
        ContractKind::Dto
    } else if lower.ends_with("model") {
        ContractKind::Model
    } else {
        ContractKind::Type
    }
}
