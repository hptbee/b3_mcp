use b3_core::{
    BranchId, BranchMetadata, EdgeConfidence, EdgeId, EdgeKind, EdgeProvenance, FileId, FileRecord,
    FileRepository, GraphEdge, GraphEdgeMetadata, GraphNode, GraphRepository, NodeId, NodeKind,
    ProjectId, StorageProvider, SymbolId, SymbolRecord, SymbolRepository, TokenSavingsRecord,
    TokenSavingsRepository, ToolCallId,
};
use b3_storage::SqliteStorage;
use tempfile::tempdir;

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

    let symbol = SymbolRecord {
        id: SymbolId::new("symbol"),
        file_id: file.id.clone(),
        name: "run".to_string(),
    };
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
