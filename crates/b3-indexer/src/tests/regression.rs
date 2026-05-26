use super::*;

#[test]
fn queue_is_bounded() {
    let queue = LocalIndexJobQueue::new(1);
    let job = IndexJob {
        project_id: ProjectId::new("project"),
        root_path: ".".to_string(),
    };

    assert!(queue.enqueue(job.clone()).is_ok());
    assert!(queue.enqueue(job).is_err());
    assert!(queue.pop().expect("pop").is_some());
}

#[test]
fn cancellation_token_can_cancel() {
    let token = CancellationToken::default();
    assert!(!token.is_cancelled());
    token.cancel();
    assert!(token.is_cancelled());
}

#[test]
fn worker_pool_is_bounded() {
    let pool = BoundedWorkerPool::new(2);
    let items = [1, 2, 3, 4, 5];
    let batches = pool.batches(&items);

    assert_eq!(pool.max_workers(), 2);
    assert_eq!(batches.len(), 2);
}

#[test]
fn local_indexer_skips_ignored_and_unchanged_files() {
    let root = std::env::temp_dir().join(format!("b3-indexer-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".git")).expect("create ignored dir");
    fs::write(root.join("lib.rs"), "fn main() {}\n").expect("write file");
    fs::write(root.join(".git").join("HEAD"), "ignored").expect("write ignored file");

    let indexer = LocalIndexer::new(
        NoopTreeSitterParser,
        MemoryStore::default(),
        MemoryBus::default(),
        IndexerConfig {
            branch_id: BranchId::new("main"),
            ..IndexerConfig::default()
        },
    );

    let summary = indexer
        .index(IndexJob {
            project_id: ProjectId::new("project"),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("index");

    assert_eq!(summary.files_seen, 1);
    assert_eq!(summary.files_parsed, 1);

    let summary = indexer
        .index(IndexJob {
            project_id: ProjectId::new("project"),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("second index");

    assert_eq!(summary.files_seen, 1);
    assert_eq!(summary.files_parsed, 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn tree_sitter_pipeline_uses_extractor_contracts() {
    let parser = TreeSitterPipelineParser::new(NoopSymbolExtractor, NoopRelationshipExtractor);
    let parsed = parser
        .parse(ParseInput {
            file_id: FileId::new("file"),
            path: PathBuf::from("lib.rs"),
            source: "fn main() {}".to_string(),
        })
        .expect("parse");

    assert_eq!(parsed.language.as_deref(), Some("rs"));
    assert!(parsed.symbols.is_empty());
    assert!(parsed.relationships.is_empty());
}

#[test]
fn rust_language_pack_extracts_basic_symbols_and_calls() {
    let parsed = RustLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("file"),
            path: PathBuf::from("lib.rs"),
            source: r#"
                    use std::fmt;

                    pub struct Runner;

                    impl Runner {
                        pub fn run(&self) {
                            helper();
                        }
                    }

                    fn helper() {}

                    #[test]
                    fn helper_test() {}
                "#
            .to_string(),
        })
        .expect("parse rust");

    assert_eq!(parsed.language.as_deref(), Some("rust"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Runner" && symbol.kind == NodeKind::Struct));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "run" && symbol.kind == NodeKind::Method));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "helper_test" && symbol.kind == NodeKind::Test));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::Calls));
    assert!(!parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::References));
}

#[test]
fn rust_language_pack_reports_backend_metadata() {
    let metadata = RustLanguagePack::backend_metadata();

    assert_eq!(metadata.backend_id.0, "tree-sitter-rust");
    assert_eq!(metadata.language_id.as_str(), "rust");
    assert!(metadata.available);
    assert!(metadata
        .capabilities
        .contains(&b3_core::LanguageBackendCapability::ExtractSymbols));
}
