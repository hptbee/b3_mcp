use super::*;

#[test]
fn event_bus_contract_accepts_domain_events() {
    let bus = MemoryBus::default();
    bus.publish(DomainEvent::ConfigReloaded(ConfigReloaded {
        project_id: None,
        source: "test".to_string(),
    }))
    .expect("publish");
}

#[test]
fn debounce_coalesces_same_path() {
    let mut debouncer = WatchDebouncer::new(Duration::from_millis(500), 10);
    let path = PathBuf::from("src/lib.rs");
    assert!(debouncer
        .push(WatchEvent {
            kind: WatchEventKind::Changed,
            path: path.clone(),
            new_path: None,
        })
        .is_none());
    assert!(debouncer
        .push(WatchEvent {
            kind: WatchEventKind::Changed,
            path,
            new_path: None,
        })
        .is_none());
    let batch = debouncer.flush().expect("batch");
    assert_eq!(batch.events.len(), 1);
}

#[test]
fn watch_config_defaults_are_disabled_and_bounded() {
    let config = WatchConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.debounce_ms, 500);
    assert_eq!(config.max_batch_size, 100);
}

#[test]
fn parser_isolation_config_defaults_are_bounded() {
    let config = IndexerConfig::default();
    assert_eq!(config.parser_isolation, ParserIsolation::InProcess);
    assert_eq!(config.parser_timeout_ms, 10_000);
    assert_eq!(config.parser_max_retries, 1);
    assert!(config.parser_worker_path.is_none());
}

#[test]
fn subprocess_worker_request_response_serializes() {
    let request = ParserJobRequest {
        project_id: "project".to_string(),
        branch_id: "main".to_string(),
        file_id: "file".to_string(),
        path: "src/lib.rs".to_string(),
        source: "fn run() {}".to_string(),
    };
    let json = serde_json::to_string(&request).expect("request json");
    let output = parse_worker_json_line(&json);
    let json = serde_json::to_string(&output).expect("response json");
    assert!(json.contains("parsed"));
    assert!(json.contains("run"));
}

#[test]
fn parser_failure_is_recorded_and_events_are_emitted() {
    let root = std::env::temp_dir().join(format!(
        "b3-indexer-parser-failure-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    fs::write(root.join("lib.rs"), "fn main() {}\n").expect("write");

    let store = MemoryStore::default();
    let bus = MemoryBus::default();
    let indexer = LocalIndexer::new(
        FailingParser::new(2),
        store,
        bus,
        IndexerConfig {
            branch_id: BranchId::new("main"),
            parser_max_retries: 1,
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
    assert_eq!(summary.files_parsed, 0);
    assert_eq!(
        indexer
            .store
            .failures
            .lock()
            .expect("failures")
            .first()
            .expect("failure")
            .retry_count,
        0
    );
    let events = indexer.event_bus.events.lock().expect("events");
    assert!(events
        .iter()
        .any(|event| matches!(event, DomainEvent::ParseFailed(_))));
    assert!(events
        .iter()
        .any(|event| matches!(event, DomainEvent::ParseFailureRecorded(_))));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn retry_policy_retries_only_worker_failures() {
    assert!(ParserFailureKind::WorkerCrash.retryable());
    assert!(ParserFailureKind::Timeout.retryable());
    assert!(ParserFailureKind::WorkerIo.retryable());
    assert!(!ParserFailureKind::ParseError.retryable());
}

#[test]
fn parser_timeout_and_crash_failures_are_structured() {
    let timeout = ParserFailure::timeout(10);
    assert_eq!(timeout.kind, ParserFailureKind::Timeout);
    assert!(timeout.message.contains("10ms"));

    let crash = ParserFailure::worker_crash(Some(1), "boom".to_string());
    assert_eq!(crash.kind, ParserFailureKind::WorkerCrash);
    assert_eq!(crash.exit_code, Some(1));
    assert_eq!(crash.stderr_excerpt.as_deref(), Some("boom"));
}

#[test]
fn indexing_continues_after_one_parser_failure() {
    let root = std::env::temp_dir().join(format!(
        "b3-indexer-parser-continue-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    fs::write(root.join("a.rs"), "fn a() {}\n").expect("write a");
    fs::write(root.join("b.rs"), "fn b() {}\n").expect("write b");

    let indexer = LocalIndexer::new(
        FailingParser::new(1),
        MemoryStore::default(),
        MemoryBus::default(),
        IndexerConfig {
            branch_id: BranchId::new("main"),
            parser_max_retries: 0,
            ..IndexerConfig::default()
        },
    );

    let summary = indexer
        .index(IndexJob {
            project_id: ProjectId::new("project"),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("index");

    assert_eq!(summary.files_seen, 2);
    assert_eq!(summary.files_parsed, 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn indexing_skips_files_that_are_not_valid_utf8() {
    let root = std::env::temp_dir().join(format!(
        "b3-indexer-invalid-utf8-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    fs::write(root.join("valid.rs"), "fn valid() {}\n").expect("write valid");
    fs::write(root.join("invalid.rs"), [0xff, 0xfe, 0xfd]).expect("write invalid");

    let event_bus = MemoryBus::default();
    let indexer = LocalIndexer::new(
        NoopTreeSitterParser,
        MemoryStore::default(),
        event_bus,
        IndexerConfig::default(),
    );

    let summary = indexer
        .index(IndexJob {
            project_id: ProjectId::new("project"),
            root_path: root.to_string_lossy().to_string(),
        })
        .expect("index");

    assert_eq!(summary.files_seen, 2);
    assert_eq!(summary.files_parsed, 1);
    assert!(indexer
        .event_bus
        .events
        .lock()
        .expect("events")
        .iter()
        .any(|event| matches!(
            event,
            DomainEvent::FileSkipped(skipped)
                if skipped.path == "invalid.rs" && skipped.reason == "file is not valid UTF-8"
        )));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn ignore_rules_skip_generated_and_local_data() {
    let ignore = IgnoreRules::default();
    assert!(ignore.should_skip(Path::new("target/debug/app")).is_some());
    assert!(ignore
        .should_skip(Path::new("node_modules/pkg/index.js"))
        .is_some());
    assert!(ignore.should_skip(Path::new(".b3/b3.db")).is_some());
    assert!(ignore.should_skip(Path::new("src/lib.rs")).is_none());
}

#[test]
fn event_classification_handles_create_modify_delete() {
    let create = notify::Event::new(NotifyEventKind::Create(CreateKind::File))
        .add_path(PathBuf::from("src/lib.rs"));
    assert_eq!(
        classify_notify_event(&create)[0].kind,
        WatchEventKind::Created
    );

    let modify = notify::Event::new(NotifyEventKind::Modify(ModifyKind::Data(
        notify::event::DataChange::Content,
    )))
    .add_path(PathBuf::from("src/lib.rs"));
    assert_eq!(
        classify_notify_event(&modify)[0].kind,
        WatchEventKind::Changed
    );

    let delete = notify::Event::new(NotifyEventKind::Remove(RemoveKind::File))
        .add_path(PathBuf::from("src/lib.rs"));
    assert_eq!(
        classify_notify_event(&delete)[0].kind,
        WatchEventKind::Deleted
    );
}
