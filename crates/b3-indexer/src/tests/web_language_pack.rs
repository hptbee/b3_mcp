#[test]
fn web_language_pack_extracts_typescript_symbols_and_exports() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("file"),
            path: PathBuf::from("src/app.ts"),
            source: r#"
                    import { helper } from "./helper";

                    export interface User {
                        id: string;
                    }

                    export type UserId = string;
                    export enum Role { Admin }
                    export const makeUser = (): User => ({ id: "1" });

                    export class Service {
                        load(): User { return makeUser(); }
                    }
                "#
            .to_string(),
        })
        .expect("parse typescript");

    assert_eq!(parsed.language.as_deref(), Some("typescript"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "User" && symbol.kind == NodeKind::Interface));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "UserId" && symbol.kind == NodeKind::Variable));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Role" && symbol.kind == NodeKind::Enum));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "makeUser" && symbol.kind == NodeKind::Function));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Service" && symbol.kind == NodeKind::Class));
}

use super::*;

#[test]
fn web_language_pack_extracts_jsx_and_tsx_component_like_symbols() {
    let jsx = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("jsx-file"),
            path: PathBuf::from("src/App.jsx"),
            source: r#"
                    import React from "react";
                    export default function App() {
                        return <main>Hello</main>;
                    }
                "#
            .to_string(),
        })
        .expect("parse jsx");
    assert_eq!(jsx.language.as_deref(), Some("jsx"));
    assert!(jsx
        .symbols
        .iter()
        .any(|symbol| symbol.name == "App" && symbol.kind == NodeKind::Function));

    let tsx = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("tsx-file"),
            path: PathBuf::from("src/Button.tsx"),
            source: r#"
                    import { Icon } from "./Icon";
                    export const Button = () => <button><Icon /></button>;
                "#
            .to_string(),
        })
        .expect("parse tsx");
    assert_eq!(tsx.language.as_deref(), Some("tsx"));
    assert!(tsx
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Button" && symbol.kind == NodeKind::Function));
}

#[test]
fn web_import_resolution_handles_relative_extensions_and_index_files() {
    let root =
        std::env::temp_dir().join(format!("b3-web-import-resolution-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src").join("feature")).expect("create dirs");
    fs::write(
        root.join("src").join("helper.ts"),
        "export const helper = 1;",
    )
    .expect("write helper");
    fs::write(
        root.join("src").join("feature").join("index.tsx"),
        "export const Feature = () => null;",
    )
    .expect("write index");

    let importer = root.join("src").join("app.tsx");
    let helper_path = root.join("src").join("helper.ts");
    let feature_path = root.join("src").join("feature").join("index.tsx");
    assert_eq!(
        resolve_web_import_path(&importer, "./helper").as_deref(),
        Some(helper_path.as_path())
    );
    assert_eq!(
        resolve_web_import_path(&importer, "./feature").as_deref(),
        Some(feature_path.as_path())
    );
    assert!(resolve_web_import_path(&importer, "react").is_none());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn default_language_pack_keeps_rust_and_fallback_behavior() {
    let rust = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("rust"),
            path: PathBuf::from("src/lib.rs"),
            source: "fn run() {}".to_string(),
        })
        .expect("parse rust");
    assert_eq!(rust.language.as_deref(), Some("rust"));
    assert!(rust.symbols.iter().any(|symbol| symbol.name == "run"));

    let unsupported = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("txt"),
            path: PathBuf::from("README.txt"),
            source: "hello".to_string(),
        })
        .expect("parse unsupported");
    assert_eq!(unsupported.language.as_deref(), Some("txt"));
    assert!(unsupported.symbols.is_empty());
}

#[test]
fn local_indexer_indexes_small_js_and_tsx_project() {
    let root = std::env::temp_dir().join(format!("b3-web-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("src").join("helper.js"),
        "export function helper() { return 1; }",
    )
    .expect("write js");
    fs::write(
        root.join("src").join("App.tsx"),
        r#"import { helper } from "./helper";
               export const App = () => <main>{helper()}</main>;"#,
    )
    .expect("write tsx");

    let store = MemoryStore::default();
    let indexer = LocalIndexer::new(
        DefaultLanguagePack,
        store,
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
        .expect("index web project");
    assert_eq!(summary.files_seen, 2);
    assert_eq!(summary.files_parsed, 2);
    assert!(summary.symbols_indexed > 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn web_language_pack_handles_invalid_syntax_without_panic() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("bad"),
            path: PathBuf::from("src/bad.ts"),
            source: "export function broken(".to_string(),
        })
        .expect("parse invalid syntax as partial tree");
    assert_eq!(parsed.language.as_deref(), Some("typescript"));
}
