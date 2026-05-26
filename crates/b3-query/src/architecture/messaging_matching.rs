use std::collections::{BTreeMap, BTreeSet};

use b3_core::{
    ArchitectureConfidence, ArchitectureConfidenceLevel, ArchitectureEdge, ArchitectureEdgeKind,
    ArchitectureEvidence, ArchitectureEvidenceKind, ArchitectureMatchCandidate, ArchitectureNode,
    ArchitectureNodeKind, ArchitectureSource, ArchitectureSourceKind, ArchitectureWarning,
    ContractResult,
};
use b3_storage::StoredMessaging;
use serde::{Deserialize, Serialize};

use super::{
    messaging_keys::{normalize_broker_kind, MessagingMatchKey, UNKNOWN_BROKER},
    open_existing_read_only, FederatedProjectStatus, FederatedQueryContext, GroupFederation,
    DEFAULT_BRANCH, DEFAULT_LIMIT,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageMatchOptions {
    pub broker: Option<String>,
    pub channel_kind: Option<String>,
    pub name: Option<String>,
    pub source_project_id: Option<String>,
    pub target_project_id: Option<String>,
    pub min_confidence: Option<u16>,
    pub limit: usize,
    pub branch: Option<String>,
}

impl Default for MessageMatchOptions {
    fn default() -> Self {
        Self {
            broker: None,
            channel_kind: None,
            name: None,
            source_project_id: None,
            target_project_id: None,
            min_confidence: None,
            limit: DEFAULT_LIMIT,
            branch: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupMessageMatchReport {
    pub group_id: String,
    pub group_name: String,
    pub matching_kind: String,
    pub match_count: usize,
    pub matches: Vec<MessageMatch>,
    pub warnings: Vec<ArchitectureWarning>,
    pub local_only: bool,
    pub federation_ready: bool,
    pub messaging_matching_ready: bool,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageMatch {
    pub candidate: ArchitectureMatchCandidate,
    pub edge: ArchitectureEdge,
    pub broker: String,
    pub channel_kind: String,
    pub channel_name: String,
    pub match_rule: String,
    pub score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MessagingEndpoint {
    project_id: String,
    project_name: String,
    record: StoredMessaging,
    role: MessagingRole,
    keys: Vec<MessagingMatchKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessagingRole {
    Producer,
    Consumer,
}

impl GroupFederation {
    pub fn message_matches(
        &self,
        group_id: &str,
        options: MessageMatchOptions,
    ) -> ContractResult<GroupMessageMatchReport> {
        let context = self.resolve_context(group_id)?;
        match_messages(context, options)
    }
}

fn match_messages(
    context: FederatedQueryContext,
    options: MessageMatchOptions,
) -> ContractResult<GroupMessageMatchReport> {
    let branch = options
        .branch
        .clone()
        .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
    let limit = if options.limit == 0 {
        DEFAULT_LIMIT
    } else {
        options.limit.min(1_000)
    };
    let mut endpoints = Vec::new();
    let mut warnings = context.warnings.clone();

    for handle in context
        .projects
        .iter()
        .filter(|project| project.status == FederatedProjectStatus::Ready)
    {
        let storage = open_existing_read_only(handle)?;
        let records = storage.messaging(
            &handle.project_id,
            &branch,
            None,
            None,
            None,
            None,
            None,
            1_000,
        )?;
        endpoints.extend(records.into_iter().filter_map(|record| {
            endpoint_from_record(&handle.project_id, &handle.display_name, record)
        }));
    }

    let producers = endpoints
        .iter()
        .filter(|endpoint| endpoint.role == MessagingRole::Producer)
        .collect::<Vec<_>>();
    let consumers = endpoints
        .iter()
        .filter(|endpoint| endpoint.role == MessagingRole::Consumer)
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    let mut seen = BTreeSet::new();

    for producer in producers {
        if !matches_source_project(producer, &options) {
            continue;
        }
        for consumer in &consumers {
            if producer.project_id == consumer.project_id
                || !matches_target_project(consumer, &options)
            {
                continue;
            }
            for producer_key in &producer.keys {
                if !matches_key_filter(producer_key, &options) {
                    continue;
                }
                for consumer_key in &consumer.keys {
                    if !matches_key_filter(consumer_key, &options) {
                        continue;
                    }
                    let Some((confidence, rule, warning)) =
                        score_message_match(producer_key, consumer_key)
                    else {
                        continue;
                    };
                    if let Some(min) = options.min_confidence {
                        if confidence.score < min {
                            continue;
                        }
                    }
                    let message_match = build_message_match(
                        producer,
                        consumer,
                        producer_key,
                        consumer_key,
                        confidence,
                        &rule,
                        warning,
                    );
                    if seen.insert(message_match.candidate.id.clone()) {
                        matches.push(message_match);
                    }
                }
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
            .then_with(|| left.broker.cmp(&right.broker))
            .then_with(|| left.channel_kind.cmp(&right.channel_kind))
            .then_with(|| left.channel_name.cmp(&right.channel_name))
    });
    matches.truncate(limit);
    warnings.extend(unmatched_warnings(&endpoints));
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

    Ok(GroupMessageMatchReport {
        group_id: context.group_id,
        group_name: context.group_name,
        matching_kind: "messaging".to_string(),
        match_count: matches.len(),
        matches,
        warnings,
        local_only: true,
        federation_ready: true,
        messaging_matching_ready: true,
        branch,
    })
}

fn endpoint_from_record(
    project_id: &str,
    project_name: &str,
    record: StoredMessaging,
) -> Option<MessagingEndpoint> {
    let role = classify_role(&record)?;
    let keys = keys_for_record(&record);
    if keys.is_empty() {
        return None;
    }
    Some(MessagingEndpoint {
        project_id: project_id.to_string(),
        project_name: project_name.to_string(),
        record,
        role,
        keys,
    })
}

fn classify_role(record: &StoredMessaging) -> Option<MessagingRole> {
    let direction = record.direction.to_ascii_lowercase();
    let kind = record.kind.to_ascii_lowercase();
    if matches!(
        direction.as_str(),
        "outbound" | "publish" | "producer" | "send"
    ) || matches!(
        kind.as_str(),
        "producer" | "publisher" | "client" | "emit" | "send"
    ) {
        Some(MessagingRole::Producer)
    } else if matches!(
        direction.as_str(),
        "inbound" | "consume" | "consumer" | "subscribe" | "handler"
    ) || matches!(
        kind.as_str(),
        "consumer" | "subscriber" | "handler" | "eventpattern" | "messagepattern"
    ) {
        Some(MessagingRole::Consumer)
    } else {
        None
    }
}

fn keys_for_record(record: &StoredMessaging) -> Vec<MessagingMatchKey> {
    let broker = Some(record.technology.as_str());
    let mut keys = Vec::new();
    if let Some(topic) = record.topic.as_deref() {
        keys.push(MessagingMatchKey::new(broker, "topic", topic));
    }
    if let Some(queue) = record.queue.as_deref() {
        keys.push(MessagingMatchKey::new(broker, "queue", queue));
    }
    if let Some(pattern) = record.pattern.as_deref() {
        keys.push(MessagingMatchKey::new(broker, "pattern", pattern));
    }
    if let Some(routing_key) = record.routing_key.as_deref() {
        keys.push(MessagingMatchKey::new(broker, "routing_key", routing_key));
    }
    if let (Some(exchange), Some(routing_key)) =
        (record.exchange.as_deref(), record.routing_key.as_deref())
    {
        keys.push(MessagingMatchKey::new(
            broker,
            "exchange_routing_key",
            &format!("{exchange}:{routing_key}"),
        ));
    }
    keys.sort();
    keys.dedup();
    keys
}

fn score_message_match(
    producer: &MessagingMatchKey,
    consumer: &MessagingMatchKey,
) -> Option<(ArchitectureConfidence, String, Option<ArchitectureWarning>)> {
    if producer.broker == consumer.broker
        && producer.channel_kind == consumer.channel_kind
        && producer.name == consumer.name
    {
        return Some((
            ArchitectureConfidence::high(
                "same broker, channel kind, and normalized messaging name",
            )
            .with_evidence(producer.normalized_key.clone())
            .with_evidence(consumer.normalized_key.clone()),
            "exact_broker_kind_name".to_string(),
            None,
        ));
    }
    if producer.broker == consumer.broker
        && compatible_channel_kinds(&producer.channel_kind, &consumer.channel_kind)
        && producer.name == consumer.name
    {
        return Some((
            ArchitectureConfidence::high("same broker and compatible topic/queue/pattern name")
                .with_evidence(producer.normalized_key.clone())
                .with_evidence(consumer.normalized_key.clone()),
            "same_broker_compatible_name".to_string(),
            None,
        ));
    }
    if (producer.broker == UNKNOWN_BROKER || consumer.broker == UNKNOWN_BROKER)
        && producer.name == consumer.name
    {
        return Some((
            ArchitectureConfidence::medium("one broker is unknown with exact messaging name")
                .with_evidence(producer.normalized_key.clone())
                .with_evidence(consumer.normalized_key.clone()),
            "unknown_broker_exact_name".to_string(),
            None,
        ));
    }
    if (producer.broker == "nestjs" || consumer.broker == "nestjs")
        && producer.name == consumer.name
        && (producer.channel_kind == "pattern" || consumer.channel_kind == "pattern")
    {
        return Some((
            ArchitectureConfidence::medium("NestJS pattern matches messaging channel name")
                .with_evidence(producer.normalized_key.clone())
                .with_evidence(consumer.normalized_key.clone()),
            "nestjs_pattern_name".to_string(),
            None,
        ));
    }
    if producer.name == consumer.name && producer.broker != consumer.broker {
        return Some((
            ArchitectureConfidence::low("same messaging name but conflicting broker kinds"),
            "same_name_conflicting_broker".to_string(),
            Some(ArchitectureWarning {
                code: "broker_mismatch".to_string(),
                message: format!(
                    "producer broker {} differs from consumer broker {} for {}",
                    producer.broker, consumer.broker, producer.name
                ),
                project_id: None,
            }),
        ));
    }
    None
}

fn compatible_channel_kinds(left: &str, right: &str) -> bool {
    left == right
        || matches!(
            (left, right),
            ("topic", "pattern")
                | ("pattern", "topic")
                | ("queue", "pattern")
                | ("pattern", "queue")
                | ("routing_key", "pattern")
                | ("pattern", "routing_key")
        )
}

fn build_message_match(
    producer: &MessagingEndpoint,
    consumer: &MessagingEndpoint,
    producer_key: &MessagingMatchKey,
    consumer_key: &MessagingMatchKey,
    confidence: ArchitectureConfidence,
    rule: &str,
    warning: Option<ArchitectureWarning>,
) -> MessageMatch {
    let left_node = messaging_node(producer, producer_key, true);
    let right_node = messaging_node(consumer, consumer_key, false);
    let normalized_key = format!(
        "{}=>{}",
        producer_key.normalized_key, consumer_key.normalized_key
    );
    let evidence = vec![
        ArchitectureEvidence {
            kind: ArchitectureEvidenceKind::NormalizedKey,
            description: "producer messaging key".to_string(),
            value: Some(producer_key.normalized_key.clone()),
            source: Some(messaging_source(producer)),
        },
        ArchitectureEvidence {
            kind: ArchitectureEvidenceKind::NormalizedKey,
            description: "consumer messaging key".to_string(),
            value: Some(consumer_key.normalized_key.clone()),
            source: Some(messaging_source(consumer)),
        },
    ];
    let mut metadata = BTreeMap::new();
    metadata.insert("match_rule".to_string(), rule.to_string());
    metadata.insert("broker".to_string(), producer_key.broker.clone());
    metadata.insert(
        "channel_kind".to_string(),
        producer_key.channel_kind.clone(),
    );
    metadata.insert("channel_name".to_string(), producer_key.name.clone());
    let edge = ArchitectureEdge {
        id: ArchitectureEdge::deterministic_id(
            &left_node.id,
            &right_node.id,
            ArchitectureEdgeKind::PublishesMessage,
        ),
        from_node_id: left_node.id.clone(),
        to_node_id: right_node.id.clone(),
        kind: ArchitectureEdgeKind::PublishesMessage,
        confidence: confidence.clone(),
        evidence: evidence.clone(),
        sources: vec![messaging_source(producer), messaging_source(consumer)],
        metadata,
    };
    MessageMatch {
        candidate: ArchitectureMatchCandidate {
            id: ArchitectureMatchCandidate::deterministic_id(
                &producer.project_id,
                Some(&consumer.project_id),
                ArchitectureEdgeKind::PublishesMessage,
                &normalized_key,
            ),
            left_project_id: producer.project_id.clone(),
            right_project_id: Some(consumer.project_id.clone()),
            left_node,
            right_node: Some(right_node),
            relationship_kind: ArchitectureEdgeKind::PublishesMessage,
            match_key: producer_key.normalized_key.clone(),
            normalized_key,
            confidence: confidence.clone(),
            evidence,
            warnings: warning.into_iter().collect(),
        },
        edge,
        broker: producer_key.broker.clone(),
        channel_kind: producer_key.channel_kind.clone(),
        channel_name: producer_key.name.clone(),
        match_rule: rule.to_string(),
        score: confidence.score,
    }
}

fn messaging_node(
    endpoint: &MessagingEndpoint,
    key: &MessagingMatchKey,
    producer: bool,
) -> ArchitectureNode {
    let role = if producer { "producer" } else { "consumer" };
    let name = format!("{role} {} {}", key.channel_kind, key.name);
    let id = ArchitectureNode::deterministic_id(
        &endpoint.project_id,
        None,
        ArchitectureNodeKind::MessagingTopic,
        &name,
        Some(&endpoint.record.file_path),
        Some(&endpoint.record.symbol_id),
    );
    ArchitectureNode {
        id,
        project_id: endpoint.project_id.clone(),
        service_id: None,
        kind: ArchitectureNodeKind::MessagingTopic,
        name: name.clone(),
        label: name,
        path: Some(endpoint.record.file_path.clone()),
        symbol_id: Some(endpoint.record.symbol_id.clone()),
        metadata: BTreeMap::from([
            ("broker".to_string(), key.broker.clone()),
            ("channel_kind".to_string(), key.channel_kind.clone()),
            ("channel_name".to_string(), key.name.clone()),
            ("project_name".to_string(), endpoint.project_name.clone()),
        ]),
        confidence: ArchitectureConfidence::new(
            ArchitectureConfidenceLevel::High,
            endpoint.record.confidence,
            "messaging metadata",
            vec![endpoint.record.source_kind.clone()],
        ),
        sources: vec![messaging_source(endpoint)],
    }
}

fn messaging_source(endpoint: &MessagingEndpoint) -> ArchitectureSource {
    ArchitectureSource {
        project_id: endpoint.project_id.clone(),
        file_path: endpoint.record.file_path.clone(),
        symbol_id: Some(endpoint.record.symbol_id.clone()),
        line_start: Some(endpoint.record.line_start),
        line_end: Some(endpoint.record.line_end),
        source_kind: ArchitectureSourceKind::MessagingMetadata,
        extractor: Some(endpoint.record.source_kind.clone()),
        metadata_key: Some("messaging".to_string()),
    }
}

fn matches_source_project(endpoint: &MessagingEndpoint, options: &MessageMatchOptions) -> bool {
    options
        .source_project_id
        .as_ref()
        .is_none_or(|project| &endpoint.project_id == project)
}

fn matches_target_project(endpoint: &MessagingEndpoint, options: &MessageMatchOptions) -> bool {
    options
        .target_project_id
        .as_ref()
        .is_none_or(|project| &endpoint.project_id == project)
}

fn matches_key_filter(key: &MessagingMatchKey, options: &MessageMatchOptions) -> bool {
    if let Some(broker) = &options.broker {
        if key.broker != normalize_broker_kind(Some(broker)) {
            return false;
        }
    }
    if let Some(channel_kind) = &options.channel_kind {
        if key.channel_kind != super::messaging_keys::normalize_channel_kind(channel_kind) {
            return false;
        }
    }
    if let Some(name) = &options.name {
        if key.name != super::messaging_keys::normalize_channel_name(name) {
            return false;
        }
    }
    true
}

fn unmatched_warnings(endpoints: &[MessagingEndpoint]) -> Vec<ArchitectureWarning> {
    let producer_names = endpoints
        .iter()
        .filter(|endpoint| endpoint.role == MessagingRole::Producer)
        .flat_map(|endpoint| endpoint.keys.iter().map(|key| key.name.clone()))
        .collect::<BTreeSet<_>>();
    let consumer_names = endpoints
        .iter()
        .filter(|endpoint| endpoint.role == MessagingRole::Consumer)
        .flat_map(|endpoint| endpoint.keys.iter().map(|key| key.name.clone()))
        .collect::<BTreeSet<_>>();
    endpoints
        .iter()
        .filter_map(|endpoint| {
            let names = endpoint
                .keys
                .iter()
                .map(|key| key.name.clone())
                .collect::<BTreeSet<_>>();
            let matched = match endpoint.role {
                MessagingRole::Producer => names.iter().any(|name| consumer_names.contains(name)),
                MessagingRole::Consumer => names.iter().any(|name| producer_names.contains(name)),
            };
            (!matched).then(|| ArchitectureWarning {
                code: "unmatched_messaging_endpoint".to_string(),
                message: format!(
                    "no peer messaging endpoint matched {}",
                    names.into_iter().collect::<Vec<_>>().join(",")
                ),
                project_id: Some(endpoint.project_id.clone()),
            })
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

    #[allow(clippy::too_many_arguments)]
    fn seed_project(
        db: &Path,
        project_id: &str,
        records: &[(
            &str,
            &str,
            &str,
            Option<&str>,
            Option<&str>,
            Option<&str>,
            Option<&str>,
        )],
    ) {
        let storage = SqliteStorage::open(db).expect("storage");
        let project = ProjectId::new(project_id);
        let branch = BranchId::new(DEFAULT_BRANCH);
        storage
            .ensure_project_branch(&project, &branch, &db.parent().unwrap().to_string_lossy())
            .expect("project");
        let symbols = records
            .iter()
            .enumerate()
            .map(
                |(index, (technology, direction, kind, topic, queue, routing_key, pattern))| {
                    let mut metadata = format!(
                        "messaging.technology={technology};messaging.kind={kind};messaging.direction={direction};messaging.file=src/{project_id}.ts;messaging.source=TestMessaging;messaging.line_start={};messaging.line_end={};messaging.confidence=9000",
                        index + 1,
                        index + 1
                    );
                    if let Some(topic) = topic {
                        metadata.push_str(&format!(";messaging.topic={topic}"));
                    }
                    if let Some(queue) = queue {
                        metadata.push_str(&format!(";messaging.queue={queue}"));
                    }
                    if let Some(routing_key) = routing_key {
                        metadata.push_str(&format!(";messaging.routing_key={routing_key}"));
                    }
                    if let Some(pattern) = pattern {
                        metadata.push_str(&format!(";messaging.pattern={pattern}"));
                    }
                    let mut symbol = SymbolRecord::new(
                        SymbolId::new(format!("{project_id}-msg-{index}")),
                        FileId::new(format!("{project_id}-file")),
                        format!("{technology} {kind} {index}"),
                        NodeKind::Endpoint,
                    );
                    symbol.start_line = index + 1;
                    symbol.end_line = index + 1;
                    symbol.visibility = Some(metadata);
                    symbol
                },
            )
            .collect::<Vec<_>>();
        storage
            .upsert_indexed_file(
                &project,
                &branch,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new(format!("{project_id}-file")),
                        project_id: project.clone(),
                        path: format!("src/{project_id}.ts"),
                        content_hash: format!("hash-{project_id}"),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: 1,
                    content: "messaging metadata".to_string(),
                    symbols,
                    edges: Vec::new(),
                },
            )
            .expect("indexed");
    }

    #[test]
    fn matches_exact_topics_queues_and_patterns() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let producer_db = dir.path().join("producer").join(".b3").join("b3.db");
        let consumer_db = dir.path().join("consumer").join(".b3").join("b3.db");
        seed_project(
            &producer_db,
            "producer",
            &[
                (
                    "kafka",
                    "outbound",
                    "Producer",
                    Some("orders.created"),
                    None,
                    None,
                    None,
                ),
                (
                    "rabbitmq",
                    "outbound",
                    "Producer",
                    None,
                    Some("payments.created"),
                    None,
                    None,
                ),
                (
                    "nestjs",
                    "outbound",
                    "Producer",
                    None,
                    None,
                    None,
                    Some("invoice.created"),
                ),
            ],
        );
        seed_project(
            &consumer_db,
            "consumer",
            &[
                (
                    "kafka",
                    "inbound",
                    "Consumer",
                    Some("orders.created"),
                    None,
                    None,
                    None,
                ),
                (
                    "rabbitmq",
                    "inbound",
                    "Consumer",
                    None,
                    Some("payments.created"),
                    None,
                    None,
                ),
                (
                    "nestjs",
                    "inbound",
                    "EventPattern",
                    None,
                    None,
                    None,
                    Some("invoice.created"),
                ),
            ],
        );
        write_registry(
            &registry,
            &[
                ("producer", "Producer", &producer_db),
                ("consumer", "Consumer", &consumer_db),
            ],
            &["producer", "consumer"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let report = federation
            .message_matches("suite", MessageMatchOptions::default())
            .expect("message matches");

        assert_eq!(report.match_count, 3);
        assert!(report.messaging_matching_ready);
        assert!(report
            .matches
            .iter()
            .any(|matched| matched.broker == "kafka" && matched.channel_name == "orders.created"));
        assert!(report
            .matches
            .iter()
            .all(|matched| matched.candidate.relationship_kind
                == ArchitectureEdgeKind::PublishesMessage));
    }

    #[test]
    fn reports_conflicting_broker_and_filters() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let producer_db = dir.path().join("producer").join(".b3").join("b3.db");
        let consumer_db = dir.path().join("consumer").join(".b3").join("b3.db");
        seed_project(
            &producer_db,
            "producer",
            &[(
                "kafka",
                "outbound",
                "Producer",
                Some("orders.created"),
                None,
                None,
                None,
            )],
        );
        seed_project(
            &consumer_db,
            "consumer",
            &[(
                "rabbitmq",
                "inbound",
                "Consumer",
                None,
                Some("orders.created"),
                None,
                None,
            )],
        );
        write_registry(
            &registry,
            &[
                ("producer", "Producer", &producer_db),
                ("consumer", "Consumer", &consumer_db),
            ],
            &["producer", "consumer"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let report = federation
            .message_matches(
                "suite",
                MessageMatchOptions {
                    name: Some("orders.created".to_string()),
                    ..MessageMatchOptions::default()
                },
            )
            .expect("message matches");

        assert_eq!(report.match_count, 1);
        assert_eq!(report.matches[0].match_rule, "same_name_conflicting_broker");
        assert!(report.matches[0]
            .candidate
            .warnings
            .iter()
            .any(|warning| warning.code == "broker_mismatch"));
    }
}
