use b3_indexer::{is_default_ignored_path, IgnoreRules};
use std::path::PathBuf;

#[test]
fn default_ignored_paths_positive() {
    let cases = vec![
        "target/foo.rs",
        "node_modules/pkg/index.js",
        ".git/config",
        "bin/Debug/app.dll",
        "obj/project.assets.json",
        "dist/main.js",
        "build/output.js",
        "coverage/lcov.info",
        ".vs/config/applicationhost.config",
        ".next/server/app.js",
        ".nuxt/dist/server.js",
    ];
    for c in cases {
        assert!(is_default_ignored_path(c), "{} should be ignored", c);
    }
}

#[test]
fn default_ignored_paths_negative() {
    let cases = vec![
        "src/builders/foo.cs",
        "src/objective/foo.m",
        "src/distinct/file.ts",
        "src/binary_tree/mod.rs",
        "src/binance/client.ts",
        "src/build_service/service.cs",
    ];
    for c in cases {
        assert!(!is_default_ignored_path(c), "{} should NOT be ignored", c);
    }
}

#[test]
fn ignored_extensions_and_names() {
    let rules = IgnoreRules::default();
    let positive = vec![
        "app.dll",
        "app.exe",
        "app.pdb",
        "dump.dmp",
        "trace.etl",
        "archive.zip",
        "data.sqlite",
        "project.assets.json",
        "app.deps.json",
        "app.runtimeconfig.json",
        "test.trx",
        "lcov.info",
        "coverage.xml",
    ];
    for name in positive {
        let p = PathBuf::from(name);
        assert!(
            rules.should_skip(&p).is_some(),
            "{} should be skipped",
            name
        );
    }

    let negative = vec![
        "Program.cs",
        "appsettings.json",
        "package.json",
        "Cargo.toml",
        "README.md",
        "schema.sql",
        "query.ksql",
    ];
    for name in negative {
        let p = PathBuf::from(name);
        assert!(
            rules.should_skip(&p).is_none(),
            "{} should NOT be skipped",
            name
        );
    }
}

#[test]
fn file_size_cap_behavior() {
    // field exists and default is set
    let cfg = b3_indexer::IndexerConfig::default();
    assert!(cfg.max_file_bytes >= 1024);
    assert_eq!(cfg.max_text_file_bytes, cfg.max_file_bytes);
    assert_eq!(cfg.max_metadata_value_len, 8 * 1024);
    assert_eq!(cfg.max_snippet_chars, 2 * 1024);
}
