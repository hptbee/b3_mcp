use super::*;

#[test]
fn go_language_pack_extracts_packages_imports_symbols_calls_and_routes() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("go-file"),
            path: PathBuf::from("cmd/server/main.go"),
            source: r#"
                package orders

                import (
                    "net/http"
                    alias "github.com/acme/pkg"
                    _ "github.com/lib/pq"
                    . "fmt"
                )

                type Order struct {}
                type Repository interface {
                    FindByID(id string) (*Order, error)
                }
                type OrderID = string
                const DefaultLimit = 10
                var cache = map[string]Order{}

                func NewOrderService() *OrderService {
                    helper()
                    http.HandleFunc("/health", healthHandler)
                    return &OrderService{}
                }

                type OrderService struct {}

                func (s *OrderService) GetOrder(id string) (*Order, error) {
                    alias.Do()
                    return nil, nil
                }

                func helper() {}
                func healthHandler(w http.ResponseWriter, r *http.Request) {}

                func routes() {
                    router := gin.Default()
                    router.GET("/orders", healthHandler)
                    e := echo.New()
                    e.POST("/users", healthHandler)
                    app := fiber.New()
                    app.Get("/fiber", healthHandler)
                    r := chi.NewRouter()
                    r.Post("/chi", healthHandler)
                }
            "#
            .to_string(),
        })
        .expect("parse go");

    assert_eq!(parsed.language.as_deref(), Some("go"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "orders" && symbol.kind == NodeKind::Namespace));
    let net_http = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "net/http" && symbol.kind == NodeKind::Package)
        .expect("net/http import");
    assert_eq!(
        go_metadata_value(net_http.visibility.as_deref().unwrap_or_default(), "stdlib").as_deref(),
        Some("true")
    );
    let aliased = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "github.com/acme/pkg")
        .expect("aliased import");
    assert_eq!(
        go_metadata_value(aliased.visibility.as_deref().unwrap_or_default(), "alias").as_deref(),
        Some("alias")
    );
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Order" && symbol.kind == NodeKind::Struct));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Repository" && symbol.kind == NodeKind::Interface));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.name == "OrderID"
            && symbol.kind == NodeKind::Variable
            && go_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "type_kind",
            )
            .as_deref()
                == Some("alias")
    }));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "DefaultLimit" && symbol.kind == NodeKind::Variable));
    let method = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "GetOrder" && symbol.kind == NodeKind::Method)
        .expect("receiver method");
    assert_eq!(
        go_metadata_value(method.visibility.as_deref().unwrap_or_default(), "receiver").as_deref(),
        Some("OrderService")
    );
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::Imports));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::Calls));

    for (name, framework) in [
        ("GET /health", "go_net_http"),
        ("GET /orders", "gin"),
        ("POST /users", "echo"),
        ("GET /fiber", "fiber"),
        ("POST /chi", "chi"),
    ] {
        let route = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.kind == NodeKind::Route && symbol.name == name)
            .unwrap_or_else(|| panic!("route {name}"));
        assert_eq!(
            route_metadata_value(route.visibility.as_deref().unwrap_or_default(), "framework")
                .as_deref(),
            Some(framework)
        );
    }
}
#[test]
fn go_language_pack_ignores_comments_and_route_like_strings_and_handles_invalid_go() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("go-invalid"),
            path: PathBuf::from("broken.go"),
            source: r#"
                package broken
                // func Commented() {}
                /*
                type Hidden struct {}
                */
                func Visible() {
                    println("http.HandleFunc(\"/string\", handler)")
                }
                func Broken(
            "#
            .to_string(),
        })
        .expect("parse invalid go");

    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Visible" && symbol.kind == NodeKind::Function));
    assert!(!parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Commented"));
    assert!(!parsed.symbols.iter().any(|symbol| symbol.name == "Hidden"));
    assert!(!parsed
        .symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route && symbol.name.contains("/string")));
}

#[test]
fn go_mod_detection_extracts_module_requires_and_replaces_without_go_command() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("go-mod"),
            path: PathBuf::from("go.mod"),
            source: r#"
                module github.com/acme/orders

                go 1.22

                require (
                    github.com/gin-gonic/gin v1.9.1
                    golang.org/x/net v0.1.0
                )

                replace github.com/acme/local => ../local
            "#
            .to_string(),
        })
        .expect("parse go.mod");

    assert_eq!(parsed.language.as_deref(), Some("gomod"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "github.com/acme/orders"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "github.com/gin-gonic/gin"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "github.com/acme/local"));
    assert!(detect_go_mod_technologies("module github.com/acme/orders")
        .expect("detect go mod")
        .iter()
        .any(|technology| technology.id == "go"));
    assert!(detect_go_mod_technologies("not a module").is_ok());
}

#[test]
fn local_indexer_indexes_small_go_project() {
    let root = std::env::temp_dir().join(format!("b3-go-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("cmd").join("server")).expect("create dirs");
    fs::write(
        root.join("go.mod"),
        "module github.com/acme/orders\n\ngo 1.22\nrequire github.com/go-chi/chi/v5 v5.0.0\n",
    )
    .expect("write go.mod");
    fs::write(
        root.join("cmd").join("server").join("main.go"),
        r#"
            package main
            import "net/http"
            type Server struct {}
            func main() { http.Handle("/health", nil) }
        "#,
    )
    .expect("write go");

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
        .expect("index go project");

    assert_eq!(summary.files_seen, 2);
    assert_eq!(summary.files_parsed, 2);
    let symbols = indexer.store.symbols.lock().expect("symbols");
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "Server" && symbol.kind == NodeKind::Struct));
    assert!(symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route && symbol.name == "GET /health"));

    fs::remove_dir_all(root).expect("cleanup");
}
