use super::*;

pub(crate) async fn languages(State(state): State<ControlState>) -> Json<Value> {
    let lsp = LspBackend::from(&state.app_config.lsp);
    let language_registry = default_language_backend_registry();
    Json(json!({
        "status": "ok",
        "lsp_enabled": lsp.enabled,
        "known_languages": language_registry.known_languages,
        "languages": [
            {
                "language_id": "rust",
                "tree_sitter": "good",
                "lsp": "disabled_by_default",
                "support": "good",
                "notes": "Rust tree-sitter indexing remains the best supported path."
            },
            {
                "language_id": "typescript",
                "tree_sitter": "basic",
                "lsp": "configurable_local_server",
                "support": "basic",
                "backend": "tree-sitter-typescript",
                "capabilities": ["DetectFile", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRelationships"],
                "notes": "Basic TypeScript symbols, imports, Node REST routes, React components, Next.js static routes, and Angular metadata are indexed locally."
            },
            {
                "language_id": "javascript",
                "tree_sitter": "basic",
                "lsp": "configurable_local_server",
                "support": "basic",
                "backend": "tree-sitter-javascript",
                "capabilities": ["DetectFile", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRelationships"],
                "notes": "Basic JavaScript symbols, imports, Node REST routes, and Next.js static routes are indexed locally."
            },
            {
                "language_id": "jsx",
                "tree_sitter": "basic",
                "lsp": "configurable_local_server",
                "support": "basic",
                "backend": "tree-sitter-javascript",
                "capabilities": ["DetectFile", "Parse", "ExtractSymbols", "ExtractImports"],
                "notes": "Basic JSX component-like function/class symbols are indexed; React runtime graph intelligence is deferred."
            },
            {
                "language_id": "tsx",
                "tree_sitter": "basic",
                "lsp": "configurable_local_server",
                "support": "basic",
                "backend": "tree-sitter-typescript",
                "capabilities": ["DetectFile", "Parse", "ExtractSymbols", "ExtractImports"],
                "notes": "Basic TSX component-like function/class symbols and Next.js static routes are indexed; full RSC semantics are deferred."
            },
            {
                "language_id": "csharp",
                "tree_sitter": "not_required",
                "lsp": "configurable_local_server",
                "support": "basic",
                "backend": "static-csharp",
                "capabilities": ["DetectFile", "Parse", "ExtractSymbols", "ExtractRoutes", "ExtractWpfHints"],
                "notes": "Basic local static C# and ASP.NET Core Web API extraction indexes controllers, route attributes, action methods, constructor dependency type names, and WPF code-behind/ViewModel hints; Roslyn, dotnet CLI execution, full semantic analysis, and full binding type checking are deferred."
            },
            {
                "language_id": "xaml",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic_static",
                "backend": "static-xaml",
                "capabilities": ["DetectFile", "Parse", "ExtractWpfMetadata", "ExtractBindings", "ExtractResources"],
                "notes": "Basic local static XAML extraction indexes WPF Application, Window, UserControl, Page, ResourceDictionary, x:Class, code-behind hints, DataContext hints, Binding paths, command bindings, and resource references without Visual Studio, MSBuild, dotnet, or a XAML compiler."
            },
            {
                "language_id": "go",
                "tree_sitter": "not_required",
                "lsp": "configurable_local_server",
                "support": "basic",
                "backend": "static-go",
                "capabilities": ["DetectFile", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRelationships", "ExtractRoutes"],
                "notes": "Basic local static Go extraction indexes packages, imports, functions, methods, structs, interfaces, type declarations, const/var declarations, local call edges, and conservative net/http plus simple router route hints; no Go toolchain, go command, module download, package registry, or runtime execution is required."
            },
            {
                "language_id": "python",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-python",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRoutes", "ExtractDataAccessHints", "ExtractMessagingHints"],
                "notes": "Basic local static Python extraction indexes modules, imports, functions/classes/methods/decorators, FastAPI/Flask/Django route hints, SQLAlchemy/Django ORM hints, and Celery/Pika hints without Python runtime, pip, migrations, or language servers."
            },
            {
                "language_id": "java",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-java",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRoutes", "ExtractDataAccessHints", "ExtractMessagingHints"],
                "notes": "Basic local static Java extraction indexes packages, imports, classes/interfaces/enums/records, methods, Spring/JAX-RS route hints, JPA/JDBC hints, and listener annotations without JVM, Maven, Gradle, compiler, or language server execution."
            },
            {
                "language_id": "kotlin",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-kotlin",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRoutes", "ExtractDataAccessHints", "ExtractMessagingHints"],
                "notes": "Basic local static Kotlin extraction indexes packages, imports, classes/objects/interfaces/functions, Spring/Ktor route hints, JPA hints, and listener annotations without JVM, Gradle, compiler, or language server execution."
            },
            {
                "language_id": "php",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-php",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRoutes", "ExtractDataAccessHints", "ExtractMessagingHints"],
                "notes": "Basic local static PHP extraction indexes namespaces/use statements, classes/interfaces/traits/enums, functions/methods, Laravel/Symfony/Slim route hints, Eloquent/raw SQL hints, and queue hints without PHP runtime or composer execution."
            },
            {
                "language_id": "ruby",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-ruby",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRoutes", "ExtractDataAccessHints", "ExtractMessagingHints"],
                "notes": "Basic local static Ruby extraction indexes modules/classes/methods/requires, Rails/Sinatra route hints, ActiveRecord hints, and Sidekiq/ActiveJob hints without Ruby runtime or bundle execution."
            },
            {
                "language_id": "c",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-c",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractIncludes"],
                "notes": "Basic local static C extraction indexes includes, macros, structs, enums, typedefs, and obvious functions without clang, gcc, make, CMake, preprocessors, compilers, or language servers."
            },
            {
                "language_id": "cpp",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-cpp",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractIncludes"],
                "notes": "Basic local static C++ extraction indexes includes, namespaces, classes, methods, structs, enums, typedefs, and obvious functions without clang, gcc, make, CMake, preprocessors, compilers, or language servers."
            },
            {
                "language_id": "swift",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-swift",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRoutes"],
                "notes": "Basic local static Swift extraction indexes imports, classes, structs, enums, protocols, extensions, functions, SwiftUI View hints, and URLSession literals without swift, xcodebuild, Xcode, or app execution."
            },
            {
                "language_id": "objective_c",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-objective-c",
                "capabilities": ["DetectFile", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRoutes"],
                "notes": "Basic local static Objective-C extraction indexes imports, interfaces, implementations, protocols, properties, methods, UIViewController hints, and NSURLSession literals without clang, Xcode, compilers, or app execution."
            },
            {
                "language_id": "dart",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-dart",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractSymbols", "ExtractImports", "ExtractRoutes"],
                "notes": "Basic local static Dart/Flutter extraction indexes imports, classes, mixins, enums, functions, Widget/build hints, route literals, and HTTP literals without dart, flutter, package fetch, build, or app execution."
            },
            {
                "language_id": "yaml",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-yaml",
                "capabilities": ["DetectFile", "Parse", "ExtractConfigKeys", "ExtractInfrastructureHints"],
                "notes": "Basic local static YAML extraction indexes key paths and safe config names; Docker Compose and Kubernetes YAML continue through existing infrastructure metadata, and secret-like values are redacted/skipped."
            },
            {
                "language_id": "json",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-json",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractConfigKeys", "ExtractPackageNames"],
                "notes": "Basic local static JSON extraction indexes key paths and package/dependency names where safe; secret-like values and connection strings are not exposed."
            },
            {
                "language_id": "toml",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-toml",
                "capabilities": ["DetectFile", "DetectProject", "Parse", "ExtractConfigKeys", "ExtractPackageNames"],
                "notes": "Basic local static TOML extraction indexes tables, keys, and dependency names without running cargo, pip, poetry, uv, or any package manager."
            },
            {
                "language_id": "xml",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-xml",
                "capabilities": ["DetectFile", "Parse", "ExtractConfigKeys", "ExtractPackageNames"],
                "notes": "Basic local static XML extraction indexes root/element paths, attribute names, and Maven package names without schema fetching, external entities, Maven, Gradle, dotnet, or remote access."
            },
            {
                "language_id": "html",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-html",
                "capabilities": ["DetectFile", "Parse", "ExtractTemplateRefs", "ExtractRoutes"],
                "notes": "Basic local static HTML/template extraction indexes titles, ids/classes, script/style refs, hrefs, and form route hints without browser execution or external resource fetching."
            },
            {
                "language_id": "css",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-css",
                "capabilities": ["DetectFile", "Parse", "ExtractSelectors", "ExtractAssetRefs"],
                "notes": "Basic local static CSS extraction indexes class/id selectors, custom properties, imports, url asset references, and keyframes without CSS processing or fetching."
            },
            {
                "language_id": "scss",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-scss",
                "capabilities": ["DetectFile", "Parse", "ExtractSelectors", "ExtractAssetRefs"],
                "notes": "Basic local static SCSS extraction indexes selectors, variables, mixins, imports, url asset references, and keyframes without Sass compilation."
            },
            {
                "language_id": "threejs_webgl",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic_hints",
                "backend": "static-js-ts-hints",
                "capabilities": ["DetectImport", "ExtractTechnologyHints", "ExtractAssetRefs"],
                "notes": "Basic static Three.js/WebGL hints are extracted from JS/TS imports and WebGL call patterns without browser, WebGL runtime, or asset loading."
            },
            {
                "language_id": "ksql",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-ksql",
                "capabilities": ["DetectFile", "Parse", "ExtractMessagingHints", "ExtractDataFlowHints"],
                "notes": "Basic local static ksqlDB extraction indexes streams, tables, connectors, Kafka topic names, and SELECT/INSERT dependencies without Kafka, ksqlDB, Docker, Confluent Cloud, or query execution."
            },
            {
                "language_id": "sql",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic",
                "backend": "static-sql",
                "capabilities": ["DetectFile", "Parse", "ExtractDataAccessHints"],
                "notes": "Basic local static SQL extraction indexes table/view/procedure/function definitions plus SELECT/FROM/JOIN/INSERT/UPDATE/DELETE table references without database connections, SQL execution, migrations, or schema validation."
            },
            {
                "language_id": "env",
                "tree_sitter": "not_required",
                "lsp": "disabled_by_default",
                "support": "basic_safe",
                "backend": "static-env",
                "capabilities": ["DetectFile", "Parse", "ExtractConfigKeys", "RedactSecrets"],
                "notes": "Env-like files are parsed locally for key names and safe example/default values only; real env files are key-only/redacted and B3 never reads the OS environment."
            }
        ]
    }))
}

pub(crate) async fn lsp_status(State(state): State<ControlState>) -> Json<Value> {
    let lsp = LspBackend::from(&state.app_config.lsp);
    Json(json!({
        "status": lsp.status(),
        "enabled": lsp.enabled,
        "local_only": true,
        "auto_start": false,
        "startup_timeout_ms": lsp.timeout.startup_timeout_ms,
        "request_timeout_ms": lsp.timeout.request_timeout_ms,
        "stderr_capture_bytes": lsp.timeout.stderr_capture_bytes,
        "server_count": lsp.servers.len(),
        "missing_servers_fatal": false,
        "limitations": [
            "LSP is disabled by default",
            "language servers are never installed or downloaded by B3",
            "LSP-based symbolic editing and rename/refactor tools are deferred"
        ]
    }))
}

pub(crate) async fn lsp_servers(State(state): State<ControlState>) -> Json<Value> {
    let lsp = LspBackend::from(&state.app_config.lsp);
    Json(json!({
        "status": "ok",
        "enabled": lsp.enabled,
        "servers": lsp.server_statuses()
    }))
}
