use super::*;

#[test]
fn deleted_file_cleanup_path_removes_record() {
    let root = std::env::temp_dir().join(format!("b3-indexer-delete-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    let path = root.join("lib.rs");
    fs::write(&path, "fn main() {}\n").expect("write");

    let store = MemoryStore::default();
    let indexer = LocalIndexer::new(
        NoopTreeSitterParser,
        store,
        MemoryBus::default(),
        IndexerConfig {
            branch_id: BranchId::new("main"),
            ..IndexerConfig::default()
        },
    );
    let project_id = ProjectId::new("project");
    indexer
        .index_paths(&root, &project_id, std::slice::from_ref(&path))
        .expect("index path");
    fs::remove_file(&path).expect("delete");
    let summary = indexer
        .index_paths(&root, &project_id, std::slice::from_ref(&path))
        .expect("cleanup");
    assert_eq!(summary.files_parsed, 0);
    fs::remove_dir_all(root).expect("cleanup dir");
}

#[test]
fn unchanged_file_skip_works_for_changed_path_indexing() {
    let root =
        std::env::temp_dir().join(format!("b3-indexer-unchanged-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    let path = root.join("lib.rs");
    fs::write(&path, "fn main() {}\n").expect("write");

    let indexer = LocalIndexer::new(
        NoopTreeSitterParser,
        MemoryStore::default(),
        MemoryBus::default(),
        IndexerConfig {
            branch_id: BranchId::new("main"),
            ..IndexerConfig::default()
        },
    );
    let project_id = ProjectId::new("project");
    assert_eq!(
        indexer
            .index_paths(&root, &project_id, std::slice::from_ref(&path))
            .expect("first")
            .files_parsed,
        1
    );
    assert_eq!(
        indexer
            .index_paths(&root, &project_id, std::slice::from_ref(&path))
            .expect("second")
            .files_parsed,
        0
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn scoped_index_preserves_unrelated_files_and_does_not_duplicate_unchanged_symbols() {
    let root = std::env::temp_dir().join(format!("b3-indexer-scope-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src").join("orders")).expect("orders");
    fs::create_dir_all(root.join("src").join("billing")).expect("billing");
    fs::write(
        root.join("src").join("orders").join("lib.rs"),
        "fn order() {}\n",
    )
    .expect("orders file");
    fs::write(
        root.join("src").join("billing").join("lib.rs"),
        "fn billing() {}\n",
    )
    .expect("billing file");

    let indexer = LocalIndexer::new(
        NoopTreeSitterParser,
        MemoryStore::default(),
        MemoryBus::default(),
        IndexerConfig {
            branch_id: BranchId::new("main"),
            ..IndexerConfig::default()
        },
    );
    let project_id = ProjectId::new("project");
    let full = indexer
        .index(IndexJob {
            project_id: project_id.clone(),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("full index");
    assert_eq!(full.files_seen, 2);

    let mut scope = scope::parse_scope("path:src/orders").expect("scope");
    scope.project_id = Some(project_id.as_str().to_string());
    let plan = scope::plan_scope(
        &root,
        project_id.as_str(),
        "main",
        scope,
        &IndexerConfig::default().ignore,
        &scope::EmptyScopeTargetProvider,
    )
    .expect("plan");
    let scoped = indexer.index_scope(plan).expect("scoped index");

    assert_eq!(scoped.files_seen, 1);
    assert_eq!(scoped.files_parsed, 0);
    assert_eq!(indexer.store.files.lock().expect("files").len(), 2);

    fs::remove_dir_all(root).expect("cleanup");
}
