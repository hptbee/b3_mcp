use b3_core::{
    BranchId, BranchMetadata, ContractResult, DomainEvent, EdgeConfidence, EdgeId, EdgeKind,
    EdgeProvenance, EventBus, FileId, FileRecord, FileRepository, GraphEdge, GraphEdgeMetadata,
    GraphNode, GraphRepository, IndexJob, Indexer, NodeId, NodeKind, ProjectId, StorageProvider,
    SymbolId, SymbolRecord, SymbolRepository, TokenSavingsRecord, TokenSavingsRepository,
    ToolCallId,
};
use b3_indexer::{IndexerConfig, LocalIndexer, RustLanguagePack};
use b3_storage::SqliteStorage;
use std::fs;
use tempfile::tempdir;

#[derive(Default)]
struct TestEventBus;

impl EventBus for TestEventBus {
    fn publish(&self, _event: DomainEvent) -> ContractResult<()> {
        Ok(())
    }
}

#[test]
fn initializes_offline_sqlite_database_with_required_schema() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("b3.db");

    let storage = SqliteStorage::open(&path).expect("open sqlite storage");

    assert_eq!(storage.name(), "sqlite");
    assert!(storage.is_local_only());
    assert_eq!(
        storage
            .pragma_value("journal_mode")
            .expect("journal mode")
            .to_lowercase(),
        "wal"
    );
    assert_eq!(storage.pragma_i64("synchronous").expect("synchronous"), 1);
    assert!(storage.migration_applied(1).expect("migration"));

    for table in [
        "projects",
        "branches",
        "files",
        "symbols",
        "nodes",
        "edges",
        "embeddings",
        "sessions",
        "decisions",
        "code_areas",
        "savings_ledger",
        "tool_logs",
        "file_content_fts",
        "symbol_fts",
    ] {
        assert!(
            storage.table_exists(table).expect("table lookup"),
            "{table}"
        );
    }

    for index in [
        "idx_edges_from",
        "idx_edges_to",
        "idx_edges_type",
        "idx_nodes_kind",
        "idx_symbols_name",
        "idx_files_path",
        "idx_files_branch_id",
    ] {
        assert!(
            storage.index_exists(index).expect("index lookup"),
            "{index}"
        );
    }
}

#[test]
fn repositories_round_trip_against_temp_database() {
    let dir = tempdir().expect("temp dir");
    let storage = SqliteStorage::open(dir.path().join("b3.db")).expect("open sqlite storage");
    let project_id = ProjectId::new("project");
    let branch_id = BranchId::new("main");
    let branch = BranchMetadata::new("main");

    storage
        .upsert_project(&project_id, "Project", ".")
        .expect("project");
    storage
        .upsert_branch(&branch_id, &project_id, &branch)
        .expect("branch");

    let file = FileRecord {
        id: FileId::new("file"),
        project_id: project_id.clone(),
        path: "src/lib.rs".to_string(),
        content_hash: "hash".to_string(),
    };
    storage.upsert_file(&file, &branch_id).expect("file");

    let symbol = SymbolRecord::new(
        SymbolId::new("symbol"),
        file.id.clone(),
        "run",
        NodeKind::Function,
    );
    storage
        .upsert_symbol(&project_id, &branch_id, &symbol)
        .expect("symbol");

    let from = GraphNode {
        id: NodeId::new("from"),
        project_id: project_id.clone(),
        label: "from".to_string(),
    };
    let to = GraphNode {
        id: NodeId::new("to"),
        project_id: project_id.clone(),
        label: "to".to_string(),
    };
    storage
        .upsert_node(&from, &branch_id, NodeKind::Function)
        .expect("from node");
    storage
        .upsert_node(&to, &branch_id, NodeKind::Function)
        .expect("to node");

    let edge = GraphEdge {
        id: EdgeId::new("edge"),
        from: from.id.clone(),
        to: to.id.clone(),
        metadata: GraphEdgeMetadata {
            confidence: EdgeConfidence::from_basis_points(9_000),
            provenance: EdgeProvenance::Ast,
            created_at_unix_ms: 1,
            updated_at_unix_ms: 2,
        },
    };
    storage
        .upsert_edge(&project_id, &branch_id, &edge, EdgeKind::Calls)
        .expect("edge");

    assert_eq!(
        storage
            .get_file(&file.id)
            .expect("get file")
            .expect("file")
            .path,
        "src/lib.rs"
    );
    assert_eq!(
        storage
            .find_symbol(&project_id, "run")
            .expect("find symbol")
            .len(),
        1
    );
    assert_eq!(
        storage
            .get_node(&from.id)
            .expect("get node")
            .expect("node")
            .label,
        "from"
    );
    assert_eq!(
        storage
            .get_edge(&edge.id)
            .expect("get edge")
            .expect("edge")
            .metadata
            .provenance,
        EdgeProvenance::Ast
    );

    storage
        .record_savings(TokenSavingsRecord {
            tool_call_id: Some(ToolCallId::new("tool-call")),
            estimated_tokens_saved: 100,
            returned_tokens: 10,
            avoided_file_reads: 2,
            avoided_search_calls: 1,
        })
        .expect("record savings");
}

#[test]
fn indexes_rust_file_into_sqlite_storage_and_reindexes_changes() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path().join("project");
    fs::create_dir_all(root.join("src")).expect("create project");
    let lib_rs = root.join("src").join("lib.rs");
    fs::write(
        &lib_rs,
        r#"
            use std::fmt;

            pub struct Runner;

            impl Runner {
                pub fn run(&self) {
                    helper();
                }
            }

            fn helper() {}
        "#,
    )
    .expect("write rust");

    let storage = SqliteStorage::open(dir.path().join("b3.db")).expect("open sqlite storage");
    let project_id = ProjectId::new("project");
    let branch_id = BranchId::new("main");
    let branch = BranchMetadata::new("main");
    storage
        .upsert_project(&project_id, "Project", root.to_string_lossy().as_ref())
        .expect("project");
    storage
        .upsert_branch(&branch_id, &project_id, &branch)
        .expect("branch");

    let indexer = LocalIndexer::new(
        RustLanguagePack,
        &storage,
        TestEventBus,
        IndexerConfig {
            branch_id: branch_id.clone(),
            ..IndexerConfig::default()
        },
    );

    let first = indexer
        .index(IndexJob {
            project_id: project_id.clone(),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("first index");

    assert_eq!(first.files_seen, 1);
    assert_eq!(first.files_parsed, 1);
    assert!(first.symbols_indexed >= 4);
    assert_eq!(storage.count_rows("files").expect("files"), 1);
    assert!(storage.count_rows("symbols").expect("symbols") >= 4);
    assert!(storage.count_rows("nodes").expect("nodes") >= 5);
    assert!(storage.count_rows("edges").expect("edges") >= 4);
    assert_eq!(
        storage
            .count_edges_by_kind(EdgeKind::References)
            .expect("references edges"),
        0
    );
    assert_eq!(storage.count_rows("file_content_fts").expect("file fts"), 1);
    assert!(storage.count_rows("symbol_fts").expect("symbol fts") >= 4);
    assert_eq!(
        storage
            .find_symbol(&project_id, "Runner")
            .expect("runner symbol")
            .len(),
        1
    );

    let second = indexer
        .index(IndexJob {
            project_id: project_id.clone(),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("second index");

    assert_eq!(second.files_seen, 1);
    assert_eq!(second.files_parsed, 0);
    let symbol_count_after_skip = storage.count_rows("symbols").expect("symbols after skip");

    fs::write(
        &lib_rs,
        r#"
            pub enum Mode {
                Fast,
            }

            pub fn replacement() {}
        "#,
    )
    .expect("rewrite rust");

    let third = indexer
        .index(IndexJob {
            project_id: project_id.clone(),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("third index");

    assert_eq!(third.files_seen, 1);
    assert_eq!(third.files_parsed, 1);
    assert_eq!(
        storage
            .find_symbol(&project_id, "Runner")
            .expect("old symbol replaced")
            .len(),
        0
    );
    assert_eq!(
        storage
            .find_symbol(&project_id, "replacement")
            .expect("new symbol")
            .len(),
        1
    );
    assert!(
        storage
            .count_rows("symbols")
            .expect("symbols after reindex")
            < symbol_count_after_skip
    );
    assert_eq!(storage.count_rows("file_content_fts").expect("file fts"), 1);
}

#[test]
fn indexing_auto_creates_project_and_branch_rows() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path().join("project");
    fs::create_dir_all(root.join("src")).expect("create project");
    fs::write(root.join("src").join("lib.rs"), "pub fn run() {}\n").expect("write rust");

    let storage = SqliteStorage::open(dir.path().join("b3.db")).expect("open sqlite storage");
    let project_id = ProjectId::new("auto-project");
    let branch_id = BranchId::new("auto-main");
    let indexer = LocalIndexer::new(
        RustLanguagePack,
        &storage,
        TestEventBus,
        IndexerConfig {
            branch_id,
            ..IndexerConfig::default()
        },
    );

    let summary = indexer
        .index(IndexJob {
            project_id: project_id.clone(),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("index with auto metadata");

    assert_eq!(summary.files_parsed, 1);
    assert_eq!(storage.count_rows("projects").expect("projects"), 1);
    assert_eq!(storage.count_rows("branches").expect("branches"), 1);
    assert_eq!(
        storage
            .find_symbol(&project_id, "run")
            .expect("auto symbol")
            .len(),
        1
    );
}

#[test]
fn indexing_cleans_up_deleted_files_for_current_branch() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path().join("project");
    fs::create_dir_all(root.join("src")).expect("create project");
    let kept = root.join("src").join("kept.rs");
    let removed = root.join("src").join("removed.rs");
    fs::write(&kept, "pub fn kept() {}\n").expect("write kept");
    fs::write(&removed, "pub fn removed() {}\n").expect("write removed");

    let storage = SqliteStorage::open(dir.path().join("b3.db")).expect("open sqlite storage");
    let project_id = ProjectId::new("delete-project");
    let branch_id = BranchId::new("main");
    let indexer = LocalIndexer::new(
        RustLanguagePack,
        &storage,
        TestEventBus,
        IndexerConfig {
            branch_id,
            ..IndexerConfig::default()
        },
    );

    let first = indexer
        .index(IndexJob {
            project_id: project_id.clone(),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("first index");
    assert_eq!(first.files_seen, 2);
    assert_eq!(storage.count_rows("files").expect("files"), 2);
    assert_eq!(
        storage
            .find_symbol(&project_id, "removed")
            .expect("removed exists")
            .len(),
        1
    );

    fs::remove_file(&removed).expect("delete file");
    let second = indexer
        .index(IndexJob {
            project_id: project_id.clone(),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("second index");

    assert_eq!(second.files_seen, 1);
    assert_eq!(second.files_parsed, 0);
    assert_eq!(storage.count_rows("files").expect("files after delete"), 1);
    assert_eq!(
        storage
            .find_symbol(&project_id, "removed")
            .expect("removed gone")
            .len(),
        0
    );
    assert_eq!(
        storage
            .find_symbol(&project_id, "kept")
            .expect("kept")
            .len(),
        1
    );
    assert_eq!(
        storage
            .count_rows("file_content_fts")
            .expect("file fts after delete"),
        1
    );
    assert_eq!(
        storage
            .count_edges_by_kind(EdgeKind::References)
            .expect("references edges after delete"),
        0
    );
}
