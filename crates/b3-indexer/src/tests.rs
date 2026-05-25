use super::*;
use b3_core::{ConfigReloaded, EventBus};
use std::{collections::HashMap, fs};

#[derive(Default)]
struct MemoryStore {
    files: Mutex<HashMap<String, FileRecord>>,
    symbols: Mutex<Vec<SymbolRecord>>,
    failures: Mutex<Vec<ParseFailureRecord>>,
}

impl IndexStore for MemoryStore {
    fn ensure_project_branch(
        &self,
        _project_id: &ProjectId,
        _branch_id: &BranchId,
        _root_path: &str,
    ) -> ContractResult<()> {
        Ok(())
    }

    fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>> {
        Ok(self
            .files
            .lock()
            .map_err(|_| ContractError::new("files lock poisoned"))?
            .get(file_id.as_str())
            .cloned())
    }

    fn cleanup_deleted_files(
        &self,
        _project_id: &ProjectId,
        _branch_id: &BranchId,
        _live_file_ids: &[FileId],
    ) -> ContractResult<()> {
        Ok(())
    }

    fn upsert_indexed_file(
        &self,
        _project_id: &ProjectId,
        _branch_id: &BranchId,
        file: IndexedFileRecord,
    ) -> ContractResult<()> {
        self.files
            .lock()
            .map_err(|_| ContractError::new("files lock poisoned"))?
            .insert(file.file.id.as_str().to_string(), file.file);
        self.symbols
            .lock()
            .map_err(|_| ContractError::new("symbols lock poisoned"))?
            .extend(file.symbols);
        Ok(())
    }

    fn remove_file(
        &self,
        _project_id: &ProjectId,
        _branch_id: &BranchId,
        path: &str,
    ) -> ContractResult<()> {
        self.files
            .lock()
            .map_err(|_| ContractError::new("files lock poisoned"))?
            .retain(|_, file| file.path != path);
        Ok(())
    }

    fn record_parse_failure(&self, failure: ParseFailureRecord) -> ContractResult<()> {
        self.failures
            .lock()
            .map_err(|_| ContractError::new("failures lock poisoned"))?
            .push(failure);
        Ok(())
    }
}

#[derive(Default)]
struct MemoryBus {
    events: Mutex<Vec<DomainEvent>>,
}

impl EventBus for MemoryBus {
    fn publish(&self, event: DomainEvent) -> ContractResult<()> {
        self.events
            .lock()
            .map_err(|_| ContractError::new("events lock poisoned"))?
            .push(event);
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct FailingParser {
    remaining_failures: Arc<AtomicUsize>,
}

impl FailingParser {
    fn new(failures: usize) -> Self {
        Self {
            remaining_failures: Arc::new(AtomicUsize::new(failures)),
        }
    }
}

impl TreeSitterParser for FailingParser {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        if self.remaining_failures.load(Ordering::SeqCst) > 0 {
            self.remaining_failures.fetch_sub(1, Ordering::SeqCst);
            return Err(ContractError::new("synthetic parse failure"));
        }
        NoopTreeSitterParser.parse(input)
    }
}

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

#[test]
fn web_language_detection_maps_js_ts_jsx_tsx_and_csharp() {
    assert_eq!(
        language_from_path(Path::new("app.js")).as_deref(),
        Some("javascript")
    );
    assert_eq!(
        language_from_path(Path::new("app.mjs")).as_deref(),
        Some("javascript")
    );
    assert_eq!(
        language_from_path(Path::new("app.cjs")).as_deref(),
        Some("javascript")
    );
    assert_eq!(
        language_from_path(Path::new("app.ts")).as_deref(),
        Some("typescript")
    );
    assert_eq!(
        language_from_path(Path::new("app.jsx")).as_deref(),
        Some("jsx")
    );
    assert_eq!(
        language_from_path(Path::new("app.tsx")).as_deref(),
        Some("tsx")
    );
    assert_eq!(
        language_from_path(Path::new("Program.cs")).as_deref(),
        Some("csharp")
    );
    assert_eq!(
        language_from_path(Path::new("Api.csproj")).as_deref(),
        Some("csproj")
    );
    assert_eq!(
        language_from_path(Path::new("cmd/server/main.go")).as_deref(),
        Some("go")
    );
    assert_eq!(
        language_from_path(Path::new("go.mod")).as_deref(),
        Some("gomod")
    );
}

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

#[test]
fn detects_csproj_aspnetcore_technologies_without_requiring_valid_xml() {
    let detected = detect_csproj_technologies(
        r#"<Project Sdk="Microsoft.NET.Sdk.Web">
            <PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup>
            <ItemGroup>
                <FrameworkReference Include="Microsoft.AspNetCore.App" />
                <PackageReference Include="Microsoft.AspNetCore.Mvc" Version="2.2.0" />
            </ItemGroup>
        </Project>"#,
    )
    .expect("detect csproj");

    assert!(detected.iter().any(|tech| tech.id == "aspnetcore"));
    assert!(detected.iter().any(|tech| tech.id == "dotnet"));
    assert!(detect_csproj_technologies("<Project><Broken").is_ok());
}

#[test]
fn csharp_language_pack_extracts_aspnetcore_controllers_routes_and_di() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("users-controller"),
            path: PathBuf::from("Controllers/UsersController.cs"),
            source: r#"
                using Microsoft.AspNetCore.Mvc;

                namespace Api.Controllers;

                [ApiController]
                [Route("api/[controller]")]
                public class UsersController : ControllerBase
                {
                    public UsersController(IUserService userService, ILogger<UsersController> logger)
                    {
                    }

                    [HttpGet]
                    public IActionResult List() { return Ok(); }

                    [HttpGet("{id}")]
                    public ActionResult<UserDto> Get(int id) { return Ok(); }

                    [HttpPost]
                    public Task<IActionResult> Create(UserDto user) { return Task.FromResult<IActionResult>(Ok()); }

                    [HttpPut("{id}")]
                    public IActionResult Update(int id, UserDto user) { return Ok(); }

                    [HttpPatch("{id}")]
                    public IActionResult Patch(int id, UserDto user) { return Ok(); }

                    [HttpDelete("{id}")]
                    public IActionResult Delete(int id) { return Ok(); }
                }
            "#
            .to_string(),
        })
        .expect("parse csharp");

    assert_eq!(parsed.language.as_deref(), Some("csharp"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Api.Controllers" && symbol.kind == NodeKind::Namespace));
    assert!(parsed.symbols.iter().any(
        |symbol| symbol.name == "Microsoft.AspNetCore.Mvc" && symbol.kind == NodeKind::Package
    ));

    let controller = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UsersController" && symbol.kind == NodeKind::Class)
        .expect("controller");
    let controller_metadata = controller.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        aspnet_metadata_value(controller_metadata, "controller").as_deref(),
        Some("true")
    );
    assert_eq!(
        aspnet_metadata_value(controller_metadata, "api_controller").as_deref(),
        Some("true")
    );
    assert!(aspnet_metadata_value(controller_metadata, "dependencies")
        .unwrap_or_default()
        .contains("IUserService"));
    assert!(aspnet_metadata_value(controller_metadata, "dependencies")
        .unwrap_or_default()
        .contains("ILogger<UsersController>"));

    let route_names: Vec<String> = parsed
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
        .map(|symbol| symbol.name.clone())
        .collect();
    assert!(route_names.contains(&"GET /api/users".to_string()));
    assert!(route_names.contains(&"GET /api/users/{id}".to_string()));
    assert!(route_names.contains(&"POST /api/users".to_string()));
    assert!(route_names.contains(&"PUT /api/users/{id}".to_string()));
    assert!(route_names.contains(&"PATCH /api/users/{id}".to_string()));
    assert!(route_names.contains(&"DELETE /api/users/{id}".to_string()));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::References));
}

#[test]
fn csharp_language_pack_handles_route_only_methods_invalid_code_and_non_web_classes() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("route-only"),
            path: PathBuf::from("Controllers/ReportsController.cs"),
            source: r#"
                [Route("api/reports")]
                public class ReportsController
                {
                    [Route("archive")]
                    public IActionResult Archive() { return Ok(); }
                }

                public class PlainService
                {
                    public void Run() {}
                }

                public class Broken {
            "#
            .to_string(),
        })
        .expect("parse invalid partial csharp");

    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route
            && symbol.name == "UNKNOWN /api/reports/archive"));
    let plain = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "PlainService")
        .expect("plain class");
    assert!(aspnet_metadata_value(
        plain.visibility.as_deref().unwrap_or_default(),
        "controller"
    )
    .is_none());
}

#[test]
fn local_indexer_indexes_small_aspnetcore_project_and_ignores_wpf_classification() {
    let root = std::env::temp_dir().join(format!("b3-csharp-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("Controllers")).expect("controllers");
    fs::write(
        root.join("Api.csproj"),
        r#"<Project Sdk="Microsoft.NET.Sdk.Web"><ItemGroup><FrameworkReference Include="Microsoft.AspNetCore.App" /></ItemGroup></Project>"#,
    )
    .expect("write csproj");
    fs::write(
        root.join("Controllers").join("UsersController.cs"),
        r#"
            using Microsoft.AspNetCore.Mvc;
            [ApiController]
            [Route("api/[controller]")]
            public class UsersController : ControllerBase
            {
                public UsersController(IUserService service) {}
                [HttpGet("{id}")]
                public IActionResult Get(int id) { return Ok(); }
            }
        "#,
    )
    .expect("write controller");
    fs::write(
        root.join("MainWindow.xaml.cs"),
        "public partial class MainWindow { public MainWindow() { InitializeComponent(); } }",
    )
    .expect("write wpf code-behind");

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
        .expect("index csharp project");

    assert_eq!(summary.files_seen, 3);
    assert_eq!(summary.files_parsed, 3);
    let symbols = indexer.store.symbols.lock().expect("symbols");
    assert!(symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route && symbol.name == "GET /api/users/{id}"));
    let main_window = symbols
        .iter()
        .find(|symbol| symbol.name == "MainWindow")
        .expect("main window");
    assert!(aspnet_metadata_value(
        main_window.visibility.as_deref().unwrap_or_default(),
        "controller"
    )
    .is_none());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn wpf_project_detection_covers_modern_and_old_project_files() {
    let modern = r#"
        <Project Sdk="Microsoft.NET.Sdk">
          <PropertyGroup>
            <OutputType>WinExe</OutputType>
            <TargetFramework>net8.0-windows</TargetFramework>
            <UseWPF>true</UseWPF>
          </PropertyGroup>
        </Project>
    "#;
    let old = r#"
        <Project ToolsVersion="15.0">
          <PropertyGroup>
            <TargetFrameworkVersion>v4.8</TargetFrameworkVersion>
          </PropertyGroup>
          <ItemGroup>
            <Reference Include="PresentationCore" />
            <Reference Include="PresentationFramework" />
            <Reference Include="WindowsBase" />
            <Reference Include="System.Xaml" />
            <ApplicationDefinition Include="App.xaml" />
            <Page Include="Views/UserView.xaml" />
            <Compile Include="Views/UserView.xaml.cs">
              <DependentUpon>UserView.xaml</DependentUpon>
            </Compile>
          </ItemGroup>
        </Project>
    "#;

    let technologies = detect_wpf_project_technologies(modern).expect("modern detection");
    assert!(technologies.iter().any(|technology| technology.id == "wpf"));
    assert!(technologies
        .iter()
        .any(|technology| technology.id == "dotnet_desktop"));

    for (source, expected_kind) in [
        (modern, "WpfProjectUseWpf"),
        (old, "WpfProjectPresentationFramework"),
    ] {
        let parsed = DefaultLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("project"),
                path: PathBuf::from("Demo.csproj"),
                source: source.to_string(),
            })
            .expect("parse project");
        let project = parsed
            .symbols
            .iter()
            .find(|symbol| {
                wpf_metadata_value(symbol.visibility.as_deref().unwrap_or_default(), "kind")
                    .as_deref()
                    == Some("Project")
            })
            .expect("wpf project metadata");
        let metadata = project.visibility.as_deref().unwrap_or_default();
        assert_eq!(
            wpf_metadata_value(metadata, "technology").as_deref(),
            Some("wpf")
        );
        assert_eq!(
            wpf_metadata_value(metadata, "kind").as_deref(),
            Some("Project")
        );
        assert_eq!(
            wpf_metadata_value(metadata, "source").as_deref(),
            Some(expected_kind)
        );
    }

    let aspnet = r#"<Project Sdk="Microsoft.NET.Sdk.Web"><ItemGroup><FrameworkReference Include="Microsoft.AspNetCore.App" /></ItemGroup></Project>"#;
    assert!(detect_wpf_project_technologies(aspnet)
        .expect("aspnet detection")
        .is_empty());
}

#[test]
fn xaml_extraction_detects_views_bindings_commands_and_resources() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("main-window"),
            path: PathBuf::from("Views/MainWindow.xaml"),
            source: r#"
                <Window x:Class="App.Views.MainWindow"
                        xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
                        xmlns:vm="clr-namespace:App.ViewModels"
                        Title="Orders">
                    <Window.DataContext>
                        <vm:MainViewModel />
                    </Window.DataContext>
                    <Window.Resources>
                        <ResourceDictionary>
                            <ResourceDictionary.MergedDictionaries>
                                <ResourceDictionary Source="Themes/Colors.xaml" />
                            </ResourceDictionary.MergedDictionaries>
                            <SolidColorBrush x:Key="PrimaryBrush" Color="Red" />
                        </ResourceDictionary>
                    </Window.Resources>
                    <TextBox Text="{Binding UserName}" />
                    <Button Command="{Binding SaveCommand}" CommandParameter="{Binding SelectedUser}" />
                    <TextBlock Foreground="{StaticResource PrimaryBrush}" Background="{DynamicResource AccentBrush}" />
                </Window>
            "#
            .to_string(),
        })
        .expect("parse xaml");

    assert_eq!(parsed.language.as_deref(), Some("xaml"));
    let window = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "MainWindow")
        .expect("window metadata");
    let metadata = window.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        wpf_metadata_value(metadata, "kind").as_deref(),
        Some("Window")
    );
    assert_eq!(
        wpf_metadata_value(metadata, "x_class").as_deref(),
        Some("App.Views.MainWindow")
    );
    assert_eq!(
        wpf_metadata_value(metadata, "code_behind").as_deref(),
        Some("Views/MainWindow.xaml.cs")
    );
    assert_eq!(
        wpf_metadata_value(metadata, "data_context").as_deref(),
        Some("MainViewModel")
    );
    assert_eq!(
        wpf_metadata_value(metadata, "view_model").as_deref(),
        Some("MainViewModel")
    );
    assert!(wpf_metadata_value(metadata, "binding_paths")
        .unwrap_or_default()
        .contains("UserName"));
    assert!(wpf_metadata_value(metadata, "binding_paths")
        .unwrap_or_default()
        .contains("SelectedUser"));
    assert!(wpf_metadata_value(metadata, "command_bindings")
        .unwrap_or_default()
        .contains("SaveCommand"));
    assert!(wpf_metadata_value(metadata, "resource_sources")
        .unwrap_or_default()
        .contains("Themes/Colors.xaml"));
    assert!(wpf_metadata_value(metadata, "resource_keys")
        .unwrap_or_default()
        .contains("PrimaryBrush"));
    assert!(wpf_metadata_value(metadata, "resource_keys")
        .unwrap_or_default()
        .contains("AccentBrush"));
}

#[test]
fn xaml_extraction_detects_common_wpf_roots_and_skips_random_xml() {
    for (path, source, kind) in [
        (
            "App.xaml",
            r#"<Application x:Class="App.App" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" />"#,
            "Application",
        ),
        (
            "Views/UserView.xaml",
            r#"<UserControl x:Class="App.Views.UserView" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" />"#,
            "UserControl",
        ),
        (
            "Views/OrdersPage.xaml",
            r#"<Page x:Class="App.Views.OrdersPage" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" />"#,
            "Page",
        ),
        (
            "Themes/Colors.xaml",
            r#"<ResourceDictionary xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"><Color x:Key="Primary">Red</Color></ResourceDictionary>"#,
            "ResourceDictionary",
        ),
    ] {
        let parsed = DefaultLanguagePack
            .parse(ParseInput {
                file_id: FileId::new(path),
                path: PathBuf::from(path),
                source: source.to_string(),
            })
            .expect("parse xaml root");
        let metadata = parsed.symbols[0].visibility.as_deref().unwrap_or_default();
        assert_eq!(wpf_metadata_value(metadata, "kind").as_deref(), Some(kind));
    }

    let random = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("random"),
            path: PathBuf::from("random.xml"),
            source: "<root><Window /></root>".to_string(),
        })
        .expect("parse random xml");
    assert!(random.symbols.is_empty());

    let invalid = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("invalid"),
            path: PathBuf::from("Broken.xaml"),
            source: "<Window x:Class=\"Broken.MainWindow\"><TextBlock".to_string(),
        })
        .expect("parse invalid xaml");
    assert!(!invalid.symbols.is_empty());
}

#[test]
fn wpf_csharp_extraction_detects_code_behind_and_view_model_hints() {
    let code_behind = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("main-window-code-behind"),
            path: PathBuf::from("Views/MainWindow.xaml.cs"),
            source: r#"
                public partial class MainWindow : Window
                {
                    public MainWindow()
                    {
                        this.DataContext = new MainViewModel();
                    }
                }
            "#
            .to_string(),
        })
        .expect("parse code behind");
    let symbol = code_behind
        .symbols
        .iter()
        .find(|symbol| {
            wpf_metadata_value(symbol.visibility.as_deref().unwrap_or_default(), "kind").as_deref()
                == Some("CodeBehind")
        })
        .expect("code behind symbol");
    let metadata = symbol.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        wpf_metadata_value(metadata, "kind").as_deref(),
        Some("CodeBehind")
    );
    assert_eq!(
        wpf_metadata_value(metadata, "data_context").as_deref(),
        Some("MainViewModel")
    );

    let view_model = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("main-viewmodel"),
            path: PathBuf::from("ViewModels/MainViewModel.cs"),
            source: r#"
                using System.ComponentModel;
                using System.Windows.Input;
                public class MainViewModel : INotifyPropertyChanged
                {
                    public ICommand SaveCommand { get; }
                }
            "#
            .to_string(),
        })
        .expect("parse view model");
    let symbol = view_model
        .symbols
        .iter()
        .find(|symbol| {
            wpf_metadata_value(symbol.visibility.as_deref().unwrap_or_default(), "kind").as_deref()
                == Some("ViewModel")
        })
        .expect("view model symbol");
    let metadata = symbol.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        wpf_metadata_value(metadata, "kind").as_deref(),
        Some("ViewModel")
    );
    assert!(wpf_metadata_value(metadata, "command_bindings")
        .unwrap_or_default()
        .contains("SaveCommand"));
}

#[test]
fn framework_wpf_scope_matches_static_wpf_files() {
    let root = std::env::temp_dir().join(format!("b3-wpf-scope-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("Views")).expect("views");
    fs::write(
        root.join("App.csproj"),
        r#"<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><UseWPF>true</UseWPF><TargetFramework>net8.0-windows</TargetFramework></PropertyGroup></Project>"#,
    )
    .expect("csproj");
    fs::write(
        root.join("Views").join("MainWindow.xaml"),
        r#"<Window x:Class="App.Views.MainWindow" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" />"#,
    )
    .expect("xaml");
    fs::write(root.join("lib.rs"), "fn untouched() {}\n").expect("rust");

    let plan = scope::plan_scope(
        &root,
        "project",
        "main",
        scope::parse_scope("framework:wpf").expect("scope"),
        &IndexerConfig::default().ignore,
        &scope::EmptyScopeTargetProvider,
    )
    .expect("wpf scope");

    assert_eq!(plan.preview.matched_files, 2);
    assert!(plan
        .preview
        .matched_frameworks
        .iter()
        .any(|framework| framework == "wpf"));
    assert!(plan
        .preview
        .sample_files
        .iter()
        .any(|file| file.ends_with("MainWindow.xaml")));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn web_language_pack_extracts_javascript_symbols_imports_and_commonjs_exports() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("file"),
            path: PathBuf::from("src/app.js"),
            source: r#"
                    import helper from "./helper";
                    const fs = require("fs");

                    export function run() {}

                    class Runner {
                        start() {}
                    }

                    const Widget = () => null;
                    module.exports = { run };
                "#
            .to_string(),
        })
        .expect("parse javascript");

    assert_eq!(parsed.language.as_deref(), Some("javascript"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "run" && symbol.kind == NodeKind::Function));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Runner" && symbol.kind == NodeKind::Class));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "start" && symbol.kind == NodeKind::Method));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Widget" && symbol.kind == NodeKind::Function));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "./helper" && symbol.kind == NodeKind::Package));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "fs" && symbol.kind == NodeKind::Package));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "module.exports" && symbol.kind == NodeKind::Variable));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::Imports));
}

#[test]
fn detects_package_json_node_rest_technologies() {
    let detected = detect_package_json_technologies(
        r#"{
                "dependencies": {
                    "express": "^4.18.0",
                    "@nestjs/common": "^10.0.0",
                    "fastify": "^4.0.0",
                    "next": "^14.0.0",
                    "@angular/core": "^17.0.0",
                    "@angular/router": "^17.0.0",
                    "react": "^18.0.0",
                    "react-dom": "^18.0.0",
                    "@prisma/client": "^5.0.0",
                    "typeorm": "^0.3.0",
                    "sequelize": "^6.0.0",
                    "ws": "^8.0.0",
                    "socket.io": "^4.0.0",
                    "@microsoft/signalr": "^8.0.0",
                    "rsocket-core": "^0.0.0",
                    "amqplib": "^0.10.0",
                    "kafkajs": "^2.0.0",
                    "@google-cloud/pubsub": "^4.0.0",
                    "@nestjs/microservices": "^10.0.0",
                    "pubsub-js": "^1.0.0"
                },
                "devDependencies": {
                    "typescript": "^5.0.0",
                    "@types/react": "^18.0.0"
                }
            }"#,
    )
    .expect("detect package technologies");
    assert!(detected.iter().any(|tech| tech.id == "express"));
    assert!(detected.iter().any(|tech| tech.id == "nestjs"));
    assert!(detected.iter().any(|tech| tech.id == "fastify"));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "react" && tech.kind == TechnologyKind::WebFrontend));
    let nextjs = detected
        .iter()
        .find(|tech| tech.id == "nextjs")
        .expect("nextjs detected");
    assert_eq!(nextjs.support_level, TechnologySupportLevel::Basic);
    assert!(nextjs
        .capabilities
        .contains(&TechnologyCapability::ExtractRoutes));
    let angular = detected
        .iter()
        .find(|tech| tech.id == "angular")
        .expect("angular detected");
    assert_eq!(angular.support_level, TechnologySupportLevel::Basic);
    assert!(angular
        .capabilities
        .contains(&TechnologyCapability::ExtractRoutes));
    assert!(angular
        .capabilities
        .contains(&TechnologyCapability::ExtractComponents));
    assert!(detected.iter().any(|tech| tech.id == "prisma"));
    assert!(detected.iter().any(|tech| tech.id == "typeorm"));
    assert!(detected.iter().any(|tech| tech.id == "sequelize"));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "websocket" && tech.kind == TechnologyKind::Realtime));
    assert!(detected.iter().any(|tech| tech.id == "socketio"));
    assert!(detected.iter().any(|tech| tech.id == "signalr"));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "rsocket" && tech.support_level == TechnologySupportLevel::Basic));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "amqp" && tech.kind == TechnologyKind::Messaging));
    assert!(detected.iter().any(|tech| tech.id == "kafka"));
    assert!(detected.iter().any(|tech| tech.id == "google_pubsub"));
    assert!(detected.iter().any(|tech| tech.id == "nestjs_messaging"));
    assert!(detected.iter().any(
        |tech| tech.id == "pubsub" && tech.support_level == TechnologySupportLevel::DetectOnly
    ));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "typescript" && tech.kind == TechnologyKind::Language));
    assert!(detect_package_json_technologies("{not-json").is_err());
    assert!(detect_nextjs_config_path(Path::new("next.config.js")).is_some());
    assert!(detect_nextjs_config_path(Path::new("vite.config.ts")).is_none());
    assert!(detect_angular_config_path(Path::new("angular.json")).is_some());
    assert!(detect_angular_config_path(Path::new("tsconfig.app.json")).is_some());
    assert!(detect_angular_config_path(Path::new("next.config.js")).is_none());
}

#[test]
fn detects_signalr_project_technologies() {
    let detected = detect_csproj_realtime_technologies(
        r#"<Project>
            <ItemGroup>
                <PackageReference Include="Microsoft.AspNetCore.SignalR" Version="1" />
            </ItemGroup>
        </Project>"#,
    )
    .expect("detect signalr csproj");

    assert!(detected.iter().any(|tech| tech.id == "signalr"));
    assert!(detect_csproj_realtime_technologies("<Project><Broken").is_ok());
}

#[test]
fn detects_messaging_project_technologies() {
    let detected = detect_csproj_messaging_technologies(
        r#"<Project>
            <ItemGroup>
                <PackageReference Include="RabbitMQ.Client" Version="6" />
                <PackageReference Include="Confluent.Kafka" Version="2" />
                <PackageReference Include="Google.Cloud.PubSub.V1" Version="3" />
                <PackageReference Include="MassTransit" Version="8" />
            </ItemGroup>
        </Project>"#,
    )
    .expect("detect messaging csproj");

    assert!(detected.iter().any(|tech| tech.id == "rabbitmq"));
    assert!(detected.iter().any(|tech| tech.id == "kafka"));
    assert!(detected.iter().any(|tech| tech.id == "google_pubsub"));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "masstransit"
            && tech.support_level == TechnologySupportLevel::DetectOnly));
    assert!(detect_csproj_messaging_technologies("<Project><Broken").is_ok());
}

#[test]
fn detects_ef_core_and_dapper_project_technologies() {
    let detected = detect_csproj_data_access_technologies(
        r#"<Project>
            <ItemGroup>
                <PackageReference Include="Microsoft.EntityFrameworkCore.Sqlite" Version="8" />
                <PackageReference Include="Dapper" Version="2" />
            </ItemGroup>
        </Project>"#,
    )
    .expect("detect data access csproj");

    assert!(detected.iter().any(|tech| tech.id == "ef_core"));
    assert!(detected.iter().any(|tech| tech.id == "dapper"));
    assert!(detect_csproj_data_access_technologies("<Project><Broken").is_ok());
}

#[test]
fn csharp_data_access_detects_ef_core_and_dapper_calls() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("data-csharp"),
            path: PathBuf::from("Repositories/UserRepository.cs"),
            source: r#"
                using Microsoft.EntityFrameworkCore;
                using Dapper;

                public class AppDbContext : DbContext
                {
                    public DbSet<User> Users { get; set; }
                }

                public class UserRepository
                {
                    public async Task<List<User>> List()
                    {
                        return await _context.Users.Where(u => u.Active).ToListAsync();
                    }

                    public async Task Add(User user)
                    {
                        _context.Users.Add(user);
                        await _context.SaveChangesAsync();
                    }

                    public async Task<User> Find(SqlConnection connection, int id)
                    {
                        return await connection.QueryFirstOrDefaultAsync<User>("SELECT * FROM Users WHERE Id = @id", new { id });
                    }

                    public Task<int> Rename(SqlConnection connection)
                    {
                        return connection.ExecuteAsync("UPDATE Users SET Name = @name");
                    }
                }
            "#
            .to_string(),
        })
        .expect("parse csharp data access");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            data_access_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "kind").as_deref() == Some("DbContext")
            && data_access_metadata_value(metadata, "context").as_deref() == Some("AppDbContext")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "kind").as_deref() == Some("DbSet")
            && data_access_metadata_value(metadata, "entity").as_deref() == Some("User")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "technology").as_deref() == Some("ef_core")
            && data_access_metadata_value(metadata, "operation").as_deref() == Some("read")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "technology").as_deref() == Some("dapper")
            && data_access_metadata_value(metadata, "query")
                .unwrap_or_default()
                .contains("SELECT * FROM Users")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "source").as_deref() == Some("DapperExecute")
    }));
}

#[test]
fn web_data_access_detects_prisma_typeorm_and_sequelize() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("data-web"),
            path: PathBuf::from("src/data.ts"),
            source: r#"
                import { PrismaClient } from "@prisma/client";
                import { Entity, Column } from "typeorm";
                import { Model } from "sequelize";

                const prisma = new PrismaClient();
                export async function loadUsers(repository, dataSource) {
                    await prisma.user.findMany();
                    await prisma.user.create({ data: {} });
                    await prisma.$queryRaw`SELECT * FROM users`;
                    await dataSource.getRepository(User).find();
                    await repository.save(user);
                    await repository.delete(id);
                    await User.findAll();
                    await User.create({});
                    await User.destroy({ where: { id } });
                }

                @Entity()
                export class User {
                    @Column()
                    name: string;
                }

                class AuditLog extends Model {}
                sequelize.define("Account", {});
            "#
            .to_string(),
        })
        .expect("parse web data access");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            data_access_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        data_access_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .as_deref()
            == Some("prisma")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "technology").as_deref() == Some("typeorm")
            && data_access_metadata_value(metadata, "kind").as_deref() == Some("Entity")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "technology").as_deref() == Some("sequelize")
            && data_access_metadata_value(metadata, "operation").as_deref() == Some("delete")
    }));
}

#[test]
fn data_access_negative_cases_do_not_classify_plain_sql_words() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain"),
            path: PathBuf::from("src/plain.ts"),
            source: r#"
                export function render() {
                    const text = "SELECT users from a dropdown";
                    return text;
                }
            "#
            .to_string(),
        })
        .expect("parse plain");
    assert!(!parsed.symbols.iter().any(|symbol| {
        data_access_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .is_some()
    }));
}

#[test]
fn web_realtime_detects_websocket_socketio_signalr_and_rsocket() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("realtime-web"),
            path: PathBuf::from("src/realtime.ts"),
            source: r#"
                import WebSocket from "ws";
                import { Server } from "socket.io";
                import * as signalR from "@microsoft/signalr";
                import { RSocketClient } from "rsocket-core";

                const browserSocket = new WebSocket("ws://localhost:3000/ws");
                browserSocket.onmessage = (event) => console.log(event.data);
                browserSocket.addEventListener("message", handler);
                browserSocket.send("hello");

                const io = new Server();
                io.on("connection", socket => {
                    socket.on("join-room", handler);
                    socket.emit("room-joined", data);
                    io.emit("broadcast", data);
                });

                const connection = new signalR.HubConnectionBuilder()
                    .withUrl("/chatHub")
                    .build();
                connection.on("ReceiveMessage", handler);
                connection.invoke("SendMessage", "u", "m");

                client.requestResponse({ metadata: "chat.route" });
                client.fireAndForget(payload);
            "#
            .to_string(),
        })
        .expect("parse realtime web");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            realtime_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "technology").as_deref() == Some("websocket")
            && realtime_metadata_value(metadata, "endpoint").as_deref()
                == Some("ws://localhost:3000/ws")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "technology").as_deref() == Some("socketio")
            && realtime_metadata_value(metadata, "event").as_deref() == Some("join-room")
            && realtime_metadata_value(metadata, "kind").as_deref() == Some("Listener")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "technology").as_deref() == Some("signalr")
            && realtime_metadata_value(metadata, "method").as_deref() == Some("SendMessage")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "technology").as_deref() == Some("rsocket")
            && realtime_metadata_value(metadata, "source").as_deref()
                == Some("RSocketRequestResponse")
    }));
}

#[test]
fn csharp_realtime_detects_signalr_hubs_and_sends() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("signalr-csharp"),
            path: PathBuf::from("Hubs/ChatHub.cs"),
            source: r#"
                using Microsoft.AspNetCore.SignalR;

                public class ChatHub : Hub
                {
                    public async Task SendMessage(string user, string message)
                    {
                        await Clients.All.SendAsync("ReceiveMessage", user, message);
                    }
                }

                public class NotRealtime
                {
                    public void Run() { var message = "message"; }
                }
            "#
            .to_string(),
        })
        .expect("parse signalr csharp");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            realtime_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "kind").as_deref() == Some("Hub")
            && realtime_metadata_value(metadata, "hub").as_deref() == Some("ChatHub")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "kind").as_deref() == Some("HubMethod")
            && realtime_metadata_value(metadata, "method").as_deref() == Some("SendMessage")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "source").as_deref() == Some("SignalRSendAsync")
            && realtime_metadata_value(metadata, "event").as_deref() == Some("ReceiveMessage")
    }));
}

#[test]
fn realtime_negative_cases_do_not_classify_plain_events() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain-events"),
            path: PathBuf::from("src/events.ts"),
            source: r#"
                export function render(emitter) {
                    const message = "message";
                    emitter.on("message", handler);
                    emitter.emit("message", message);
                    return message;
                }
            "#
            .to_string(),
        })
        .expect("parse plain events");
    assert!(!parsed.symbols.iter().any(|symbol| {
        realtime_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .is_some()
    }));
}

#[test]
fn web_messaging_detects_amqp_kafka_pubsub_and_nestjs() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("messaging-web"),
            path: PathBuf::from("src/messaging.ts"),
            source: r#"
                import amqp from "amqplib";
                import { Kafka } from "kafkajs";
                import { PubSub } from "@google-cloud/pubsub";
                import { MessagePattern, EventPattern, ClientProxy } from "@nestjs/microservices";

                export async function run(channel, producer, consumer, client: ClientProxy) {
                    channel.assertExchange("orders.exchange", "topic");
                    channel.assertQueue("orders.queue");
                    channel.bindQueue("orders.queue", "orders.exchange", "order.created");
                    channel.publish("orders.exchange", "order.created", Buffer.from("{}"));
                    channel.sendToQueue("orders.queue", Buffer.from("{}"));
                    channel.consume("orders.queue", handler);
                    await producer.send({ topic: "orders", messages: [] });
                    await consumer.subscribe({ topic: "orders" });
                    await consumer.run({ eachMessage: async () => {} });
                    const pubsub = new PubSub();
                    const topic = pubsub.topic("orders");
                    await topic.publishMessage({ json: {} });
                    const subscription = pubsub.subscription("orders-sub");
                    subscription.on("message", handler);
                    client.emit("order.created", {});
                    client.send("sum", {});
                }

                export class OrdersController {
                    @MessagePattern("order.created")
                    handleOrderCreated() {}

                    @EventPattern({ cmd: "sum" })
                    handleSum() {}
                }
            "#
            .to_string(),
        })
        .expect("parse web messaging");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            messaging_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("AmqpPublish")
            && messaging_metadata_value(metadata, "exchange").as_deref() == Some("orders.exchange")
            && messaging_metadata_value(metadata, "routing_key").as_deref() == Some("order.created")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("AmqpConsume")
            && messaging_metadata_value(metadata, "queue").as_deref() == Some("orders.queue")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("KafkaProducerSend")
            && messaging_metadata_value(metadata, "topic").as_deref() == Some("orders")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref()
            == Some("GooglePubSubSubscriptionHandler")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("NestMessagePattern")
            && messaging_metadata_value(metadata, "pattern").as_deref() == Some("order.created")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("NestEventPattern")
            && messaging_metadata_value(metadata, "pattern").as_deref() == Some("sum")
    }));
}

#[test]
fn csharp_messaging_detects_rabbitmq_kafka_and_pubsub() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("messaging-csharp"),
            path: PathBuf::from("Messaging/Workers.cs"),
            source: r#"
                using RabbitMQ.Client;
                using Confluent.Kafka;
                using Google.Cloud.PubSub.V1;

                public class Workers
                {
                    public async Task Run(IModel channel, IProducer<string, string> producer, IConsumer<string, string> consumer)
                    {
                        channel.ExchangeDeclare(exchange: "orders.exchange", type: "topic");
                        channel.QueueDeclare(queue: "orders.queue");
                        channel.QueueBind(queue: "orders.queue", exchange: "orders.exchange", routingKey: "order.created");
                        channel.BasicPublish(exchange: "orders.exchange", routingKey: "order.created", body: body);
                        channel.BasicConsume(queue: "orders.queue", autoAck: true, consumer: handler);
                        await producer.ProduceAsync("orders", message);
                        consumer.Subscribe("orders");
                        consumer.Consume(token);
                        var publisher = await PublisherClient.CreateAsync("projects/demo/topics/orders");
                        await publisher.PublishAsync("payload");
                        var subscriber = await SubscriberClient.CreateAsync("projects/demo/subscriptions/orders-sub");
                        await subscriber.StartAsync(handler);
                    }
                }
            "#
            .to_string(),
        })
        .expect("parse csharp messaging");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            messaging_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("RabbitMqPublish")
            && messaging_metadata_value(metadata, "routing_key").as_deref() == Some("order.created")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("KafkaProduceAsync")
            && messaging_metadata_value(metadata, "topic").as_deref() == Some("orders")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref()
            == Some("GooglePubSubSubscriberClient")
            && messaging_metadata_value(metadata, "queue")
                .unwrap_or_default()
                .contains("orders-sub")
    }));
}

#[test]
fn messaging_negative_cases_do_not_classify_plain_event_emitters() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain-messaging"),
            path: PathBuf::from("src/plain.ts"),
            source: r#"
                export function render(emitter) {
                    const topic = "orders";
                    const queue = "orders.queue";
                    emitter.on("message", handler);
                    emitter.emit("order.created", {});
                    return `${topic}:${queue}`;
                }
            "#
            .to_string(),
        })
        .expect("parse plain messaging");
    assert!(!parsed.symbols.iter().any(|symbol| {
        messaging_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .is_some()
    }));
}

#[test]
fn infrastructure_detects_docker_compose_kubernetes_and_terraform() {
    let docker = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("dockerfile"),
            path: PathBuf::from("Dockerfile"),
            source:
                "FROM node:20\nENV NODE_ENV=production\nEXPOSE 3000\nCMD [\"npm\", \"start\"]\n"
                    .to_string(),
        })
        .expect("parse dockerfile");
    assert_eq!(docker.language.as_deref(), Some("dockerfile"));
    assert!(docker.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "source").as_deref() == Some("DockerfileFrom")
            && infrastructure_metadata_value(metadata, "image").as_deref() == Some("node:20")
    }));
    assert!(docker.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "source").as_deref() == Some("DockerfileExpose")
            && infrastructure_metadata_value(metadata, "ports")
                .unwrap_or_default()
                .contains("3000")
    }));

    let compose = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("compose"),
            path: PathBuf::from("compose.yaml"),
            source: r#"
services:
  api:
    image: my-api:latest
    ports:
      - "8080:8080"
    environment:
      - ASPNETCORE_ENVIRONMENT=Development
    depends_on:
      - db
"#
            .to_string(),
        })
        .expect("parse compose");
    assert!(compose.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "source").as_deref() == Some("ComposeService")
            && infrastructure_metadata_value(metadata, "name").as_deref() == Some("api")
            && infrastructure_metadata_value(metadata, "image").as_deref() == Some("my-api:latest")
            && infrastructure_metadata_value(metadata, "env_keys")
                .unwrap_or_default()
                .contains("ASPNETCORE_ENVIRONMENT")
    }));

    let kubernetes = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("kubernetes"),
            path: PathBuf::from("deploy/k8s.yaml"),
            source: r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
  namespace: default
  labels:
    app: api
spec:
  selector:
    matchLabels:
      app: api
  template:
    spec:
      containers:
        - name: api
          image: my-api:latest
          ports:
            - containerPort: 8080
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: api
  annotations:
    iam.gke.io/gcp-service-account: api@demo.iam.gserviceaccount.com
spec:
  rules:
    - host: api.example.test
      http:
        paths:
          - path: /
            backend:
              service:
                name: api
                port:
                  number: 80
"#
            .to_string(),
        })
        .expect("parse kubernetes");
    assert_eq!(kubernetes.language.as_deref(), Some("kubernetes"));
    assert!(kubernetes.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "source").as_deref() == Some("KubernetesDeployment")
            && infrastructure_metadata_value(metadata, "name").as_deref() == Some("api")
            && infrastructure_metadata_value(metadata, "image").as_deref() == Some("my-api:latest")
            && infrastructure_metadata_value(metadata, "selectors")
                .unwrap_or_default()
                .contains("app=api")
    }));
    assert!(kubernetes.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "technology").as_deref() == Some("gke")
            && infrastructure_metadata_value(metadata, "source").as_deref()
                == Some("GkeKubernetesManifest")
    }));

    let terraform = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("terraform"),
            path: PathBuf::from("main.tf"),
            source: r#"
provider "google" {
  project = "demo"
  region = "asia-southeast1"
}

resource "google_container_cluster" "primary" {
  name = "b3-cluster"
  location = "asia-southeast1"
}

resource "google_pubsub_topic" "orders" {
  name = "orders"
}

module "network" {
  source = "./modules/network"
}

variable "project_id" {}
output "cluster_name" {
  value = google_container_cluster.primary.name
}
"#
            .to_string(),
        })
        .expect("parse terraform");
    assert_eq!(terraform.language.as_deref(), Some("terraform"));
    assert!(terraform.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "technology").as_deref() == Some("gke")
            && infrastructure_metadata_value(metadata, "source").as_deref()
                == Some("GkeTerraformCluster")
            && infrastructure_metadata_value(metadata, "resource_type").as_deref()
                == Some("google_container_cluster")
    }));
    assert!(terraform.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "kind").as_deref() == Some("Module")
            && infrastructure_metadata_value(metadata, "name").as_deref() == Some("network")
    }));
    assert!(terraform.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "kind").as_deref() == Some("Variable")
            && infrastructure_metadata_value(metadata, "name").as_deref() == Some("project_id")
    }));
    assert!(terraform.symbols.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        infrastructure_metadata_value(metadata, "kind").as_deref() == Some("Output")
            && infrastructure_metadata_value(metadata, "name").as_deref() == Some("cluster_name")
    }));
}

#[test]
fn infrastructure_negative_and_invalid_cases_do_not_panic_or_overclassify() {
    let random_yaml = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("random-yaml"),
            path: PathBuf::from("notes.yaml"),
            source: "message: Deployment\nmetadata: not-kubernetes\n".to_string(),
        })
        .expect("parse random yaml");
    assert!(random_yaml.symbols.iter().all(|symbol| {
        infrastructure_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .is_none()
    }));

    let random_code = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("docker-strings"),
            path: PathBuf::from("src/docker.ts"),
            source: r#"const text = "FROM node:20 EXPOSE 3000";"#.to_string(),
        })
        .expect("parse docker strings");
    assert!(random_code.symbols.iter().all(|symbol| {
        infrastructure_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .is_none()
    }));

    let invalid_tf = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("invalid-tf"),
            path: PathBuf::from("broken.tf"),
            source: "resource \"google_container_cluster\" {\n  name = \n".to_string(),
        })
        .expect("parse invalid terraform");
    assert_eq!(invalid_tf.language.as_deref(), Some("terraform"));
}

#[test]
fn detects_angular_components_decorators_and_template_metadata() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("angular-component"),
            path: PathBuf::from("src/app/user-card.component.ts"),
            source: r#"
                    import { Component, Directive, Pipe } from "@angular/core";

                    @Component({
                        selector: "app-user-card",
                        templateUrl: "./user-card.component.html",
                        styleUrls: ["./user-card.component.scss"],
                        standalone: true,
                        imports: [CommonModule, UserBadgeComponent],
                        providers: [UserService]
                    })
                    export class UserCardComponent {}

                    @Component({
                        selector: "app-inline",
                        template: `<span>Inline</span>`,
                        styleUrl: "./inline.css"
                    })
                    export class InlineComponent {}

                    @Directive({ selector: "[appHighlight]" })
                    export class HighlightDirective {}

                    @Pipe({ name: "initials", standalone: true })
                    export class InitialsPipe {}
                "#
            .to_string(),
        })
        .expect("parse angular component");

    let user_card = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UserCardComponent")
        .expect("component symbol");
    let metadata = user_card.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        component_metadata_value(metadata, "framework").as_deref(),
        Some("angular")
    );
    assert_eq!(
        angular_metadata_value(metadata, "selector").as_deref(),
        Some("app-user-card")
    );
    assert_eq!(
        angular_metadata_value(metadata, "template_url").as_deref(),
        Some("./user-card.component.html")
    );
    assert!(angular_metadata_value(metadata, "style_urls")
        .as_deref()
        .unwrap_or_default()
        .contains("./user-card.component.scss"));
    assert_eq!(
        angular_metadata_value(metadata, "standalone").as_deref(),
        Some("true")
    );
    assert!(angular_metadata_value(metadata, "imports")
        .as_deref()
        .unwrap_or_default()
        .contains("UserBadgeComponent"));
    assert!(angular_metadata_value(metadata, "providers")
        .as_deref()
        .unwrap_or_default()
        .contains("UserService"));

    let inline = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "InlineComponent")
        .expect("inline component");
    let inline_metadata = inline.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        angular_metadata_value(inline_metadata, "inline_template_present").as_deref(),
        Some("true")
    );
    assert!(angular_metadata_value(inline_metadata, "style_urls")
        .as_deref()
        .unwrap_or_default()
        .contains("./inline.css"));

    let directive = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "HighlightDirective")
        .expect("directive");
    assert_eq!(
        angular_metadata_value(directive.visibility.as_deref().unwrap_or_default(), "kind")
            .as_deref(),
        Some("directive")
    );
    let pipe = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "InitialsPipe")
        .expect("pipe");
    assert_eq!(
        angular_metadata_value(pipe.visibility.as_deref().unwrap_or_default(), "pipe_name")
            .as_deref(),
        Some("initials")
    );
}

#[test]
fn detects_angular_services_modules_routes_and_di() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("angular-router"),
            path: PathBuf::from("src/app/app-routing.module.ts"),
            source: r#"
                    import { Injectable, NgModule, Component } from "@angular/core";
                    import { HttpClient } from "@angular/common/http";
                    import { RouterModule, Routes } from "@angular/router";

                    @Component({ selector: "app-home", template: "<p>Home</p>" })
                    export class HomeComponent {}
                    @Component({ selector: "app-user-detail", template: "<p>User</p>" })
                    export class UserDetailComponent {}
                    export class CacheService {}

                    @Injectable({ providedIn: "root" })
                    export class UserService {
                        constructor(private http: HttpClient, readonly cache: CacheService) {}
                    }

                    const routes: Routes = [
                        { path: "", component: HomeComponent },
                        { path: "users/:id", component: UserDetailComponent },
                        { path: "admin", loadChildren: () => import("./admin/admin.module").then(m => m.AdminModule) },
                        { path: "profile", loadComponent: () => import("./profile/profile.component").then(m => m.ProfileComponent) },
                        { path: "**", redirectTo: "" }
                    ];

                    @NgModule({
                        declarations: [HomeComponent, UserDetailComponent],
                        imports: [RouterModule.forRoot(routes)],
                        providers: [UserService],
                        exports: [RouterModule],
                        bootstrap: [HomeComponent]
                    })
                    export class AppRoutingModule {}
                "#
            .to_string(),
        })
        .expect("parse angular router");

    let service = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UserService")
        .expect("service symbol");
    let service_metadata = service.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        angular_metadata_value(service_metadata, "kind").as_deref(),
        Some("service")
    );
    assert_eq!(
        angular_metadata_value(service_metadata, "provided_in").as_deref(),
        Some("root")
    );
    assert!(angular_metadata_value(service_metadata, "dependencies")
        .as_deref()
        .unwrap_or_default()
        .contains("HttpClient"));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| { edge.from_symbol == service.id && edge.kind == EdgeKind::References }));

    let module = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "AppRoutingModule")
        .expect("module symbol");
    let module_metadata = module.visibility.as_deref().unwrap_or_default();
    assert!(angular_metadata_value(module_metadata, "declarations")
        .as_deref()
        .unwrap_or_default()
        .contains("HomeComponent"));
    assert!(angular_metadata_value(module_metadata, "imports")
        .as_deref()
        .unwrap_or_default()
        .contains("RouterModule.forRoot"));
    assert!(angular_metadata_value(module_metadata, "providers")
        .as_deref()
        .unwrap_or_default()
        .contains("UserService"));

    let routes = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            symbol.kind == NodeKind::Route
                && route_metadata_value(
                    symbol.visibility.as_deref().unwrap_or_default(),
                    "framework",
                )
                .as_deref()
                    == Some("angular")
        })
        .collect::<Vec<_>>();
    assert_eq!(routes.len(), 5);
    assert!(routes.iter().any(|route| {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        route_metadata_value(metadata, "path").as_deref() == Some("/users/:id")
            && route_metadata_value(metadata, "class").as_deref() == Some("UserDetailComponent")
    }));
    assert!(routes.iter().any(|route| {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        route_metadata_value(metadata, "path").as_deref() == Some("/admin")
            && route_metadata_value(metadata, "source").as_deref() == Some("AngularLazyRoute")
    }));
    assert!(routes.iter().any(|route| {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        route_metadata_value(metadata, "path").as_deref() == Some("/profile")
            && route_metadata_value(metadata, "source").as_deref()
                == Some("AngularLoadComponentRoute")
    }));
    assert!(routes.iter().any(|route| {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        route_metadata_value(metadata, "path").as_deref() == Some("/**")
            && route_metadata_value(metadata, "source").as_deref() == Some("AngularRedirectRoute")
    }));
    assert!(parsed.relationships.iter().any(|edge| {
        edge.kind == EdgeKind::References
            && routes.iter().any(|route| edge.from_symbol == route.id)
            && parsed
                .symbols
                .iter()
                .any(|symbol| symbol.id == edge.to_symbol && symbol.name == "HomeComponent")
    }));
}

#[test]
fn does_not_misclassify_plain_typescript_as_angular() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain-ts"),
            path: PathBuf::from("src/plain.ts"),
            source: r#"
                    class Component {}
                    export class PlainService {
                        constructor(private value: string) {}
                    }
                    const routes = [{ path: "x", component: Component }];
                "#
            .to_string(),
        })
        .expect("parse plain ts");

    assert!(!parsed.symbols.iter().any(|symbol| {
        angular_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "framework",
        )
        .as_deref()
            == Some("angular")
    }));
    assert!(!parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && route_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "framework",
            )
            .as_deref()
                == Some("angular")
    }));
}

#[test]
fn detects_express_routes_and_handler_edges() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("express"),
            path: PathBuf::from("src/server.js"),
            source: r#"
                    const express = require("express");
                    const app = express();
                    const router = express.Router();

                    function listUsers(req, res) {}
                    function createUser(req, res) {}

                    app.get("/users", listUsers);
                    app.post("/users", createUser);
                    router.route("/users/:id").get(listUsers).post(createUser);
                    app.use("/users", router);
                "#
            .to_string(),
        })
        .expect("parse express");

    let routes = parsed
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
        .collect::<Vec<_>>();
    assert!(routes.iter().any(|route| route.name == "GET /users"));
    assert!(routes.iter().any(|route| route.name == "POST /users"));
    assert!(routes.iter().any(|route| route.name == "GET /users/:id"));
    assert!(routes.iter().any(|route| route.name == "ALL /users"));
    assert!(routes.iter().any(|route| route
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("route.framework=express")));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::References));
}

#[test]
fn detects_nestjs_controller_routes_with_composed_paths() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("nest"),
            path: PathBuf::from("src/users.controller.ts"),
            source: r#"
                    import { Controller, Get, Post } from "@nestjs/common";

                    @Controller("users")
                    export class UsersController {
                        @Get()
                        findAll() {}

                        @Get(":id")
                        findOne() {}

                        @Post()
                        create() {}
                    }
                "#
            .to_string(),
        })
        .expect("parse nest");

    let route_names = parsed
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
        .map(|symbol| symbol.name.as_str())
        .collect::<Vec<_>>();
    assert!(route_names.contains(&"GET /users"));
    assert!(route_names.contains(&"GET /users/:id"));
    assert!(route_names.contains(&"POST /users"));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && symbol
                .visibility
                .as_deref()
                .unwrap_or_default()
                .contains("route.framework=nestjs")
    }));
}

#[test]
fn detects_fastify_shorthand_and_route_object() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("fastify"),
            path: PathBuf::from("src/server.ts"),
            source: r#"
                    import fastify from "fastify";
                    const app = fastify();
                    function listUsers() {}
                    app.get("/users", listUsers);
                    fastify.route({
                        method: "POST",
                        url: "/users",
                        handler: listUsers
                    });
                "#
            .to_string(),
        })
        .expect("parse fastify");

    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route && symbol.name == "GET /users"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route && symbol.name == "POST /users"));
}

#[test]
fn detects_nextjs_app_router_routes_dynamic_segments_and_groups() {
    let cases = [
        ("app/page.tsx", "GET /", "NextAppPage", "page"),
        ("app/users/page.tsx", "GET /users", "NextAppPage", "page"),
        (
            "app/users/[id]/page.tsx",
            "GET /users/:id",
            "NextAppPage",
            "page",
        ),
        (
            "app/blog/[...slug]/page.tsx",
            "GET /blog/*slug",
            "NextAppPage",
            "page",
        ),
        (
            "app/docs/[[...slug]]/page.tsx",
            "GET /docs/*slug?",
            "NextAppPage",
            "page",
        ),
        ("app/(marketing)/page.tsx", "GET /", "NextAppPage", "page"),
        ("app/layout.tsx", "GET /", "NextAppLayout", "layout"),
        ("app/loading.tsx", "GET /", "NextAppLoading", "loading"),
        ("app/error.tsx", "GET /", "NextAppError", "error"),
        ("app/not-found.tsx", "GET /", "NextAppNotFound", "not_found"),
    ];
    for (path, name, source_kind, route_kind) in cases {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new(path),
                path: PathBuf::from(path),
                source: "export default function Page() { return <main />; }".to_string(),
            })
            .expect("parse next app route");
        let route = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.kind == NodeKind::Route && symbol.name == name)
            .unwrap_or_else(|| panic!("route {name} for {path}"));
        let metadata = route.visibility.as_deref().unwrap_or_default();
        assert_eq!(
            route_metadata_value(metadata, "framework").as_deref(),
            Some("nextjs")
        );
        assert_eq!(
            route_metadata_value(metadata, "source").as_deref(),
            Some(source_kind)
        );
        assert_eq!(
            route_metadata_value(metadata, "kind").as_deref(),
            Some(route_kind)
        );
    }
}

#[test]
fn detects_nextjs_app_route_handler_methods_and_edges() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("next-route"),
            path: PathBuf::from("app/api/users/[id]/route.ts"),
            source: r#"
                    export async function GET() {
                        return Response.json([]);
                    }

                    export const PATCH = async () => Response.json({});
                    export function DELETE() {}
                "#
            .to_string(),
        })
        .expect("parse next route handler");
    for name in [
        "GET /api/users/:id",
        "PATCH /api/users/:id",
        "DELETE /api/users/:id",
    ] {
        let route = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.kind == NodeKind::Route && symbol.name == name)
            .unwrap_or_else(|| panic!("route {name}"));
        let metadata = route.visibility.as_deref().unwrap_or_default();
        assert_eq!(
            route_metadata_value(metadata, "framework").as_deref(),
            Some("nextjs")
        );
        assert_eq!(
            route_metadata_value(metadata, "source").as_deref(),
            Some("NextAppRouteHandler")
        );
        assert_eq!(
            route_metadata_value(metadata, "kind").as_deref(),
            Some("api")
        );
    }
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::References));
}

#[test]
fn detects_nextjs_pages_router_pages_and_api_routes() {
    let cases = [
        ("pages/index.tsx", "GET /", "NextPagesPage", "page"),
        (
            "pages/users/index.tsx",
            "GET /users",
            "NextPagesPage",
            "page",
        ),
        (
            "pages/users/[id].tsx",
            "GET /users/:id",
            "NextPagesPage",
            "page",
        ),
        (
            "pages/blog/[...slug].tsx",
            "GET /blog/*slug",
            "NextPagesPage",
            "page",
        ),
        (
            "pages/api/users.ts",
            "GET /api/users",
            "NextPagesApiRoute",
            "api",
        ),
        (
            "pages/api/users/[id].ts",
            "GET /api/users/:id",
            "NextPagesApiRoute",
            "api",
        ),
    ];
    for (path, name, source_kind, route_kind) in cases {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new(path),
                path: PathBuf::from(path),
                source: "export default function Page() { return <main />; }".to_string(),
            })
            .expect("parse next pages route");
        let route = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.kind == NodeKind::Route && symbol.name == name)
            .unwrap_or_else(|| panic!("route {name} for {path}"));
        let metadata = route.visibility.as_deref().unwrap_or_default();
        assert_eq!(
            route_metadata_value(metadata, "source").as_deref(),
            Some(source_kind)
        );
        assert_eq!(
            route_metadata_value(metadata, "kind").as_deref(),
            Some(route_kind)
        );
    }
    let special = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("app"),
            path: PathBuf::from("pages/_app.tsx"),
            source: "export default function App() { return null; }".to_string(),
        })
        .expect("parse pages special");
    assert!(!special
        .symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route));
}

#[test]
fn detects_react_tsx_components_props_hooks_and_usages() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("react"),
            path: PathBuf::from("src/ProductCard.tsx"),
            source: r#"
                    import React, { useEffect, useState, memo } from "react";

                    interface ProductCardProps {
                        name: string;
                    }

                    type BadgeProps = {
                        label: string;
                    };

                    export function ProductCard(props: ProductCardProps) {
                        const [open, setOpen] = useState(false);
                        useEffect(() => {}, []);
                        return <Badge label={props.name} />;
                    }

                    const Badge = ({ label }: BadgeProps) => <span>{label}</span>;
                    export default memo(ProductCard);

                    function helper() {
                        return "not jsx";
                    }
                "#
            .to_string(),
        })
        .expect("parse react tsx");

    let product = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "ProductCard")
        .expect("ProductCard symbol");
    let product_metadata = product.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        component_metadata_value(product_metadata, "framework").as_deref(),
        Some("react")
    );
    assert_eq!(
        component_metadata_value(product_metadata, "props").as_deref(),
        Some("ProductCardProps")
    );
    assert!(component_metadata_value(product_metadata, "hooks")
        .unwrap_or_default()
        .contains("useState"));
    assert!(component_metadata_value(product_metadata, "usages")
        .unwrap_or_default()
        .contains("Badge"));

    let badge = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Badge")
        .expect("Badge symbol");
    assert_eq!(
        component_metadata_value(badge.visibility.as_deref().unwrap_or_default(), "props")
            .as_deref(),
        Some("BadgeProps")
    );
    assert!(!parsed.symbols.iter().any(|symbol| {
        symbol.name == "helper"
            && component_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "framework",
            )
            .is_some()
    }));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::References));
}

#[test]
fn detects_react_jsx_components_and_class_components() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("jsx"),
            path: PathBuf::from("src/App.jsx"),
            source: r#"
                    import * as React from "react";

                    class ProductCard extends React.Component {
                        render() {
                            return <section />;
                        }
                    }

                    export const App = () => <ProductCard />;
                    const value = () => "plain";
                "#
            .to_string(),
        })
        .expect("parse react jsx");

    let app = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "App")
        .expect("App symbol");
    assert_eq!(
        component_metadata_value(app.visibility.as_deref().unwrap_or_default(), "framework")
            .as_deref(),
        Some("react")
    );
    let product = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "ProductCard")
        .expect("ProductCard symbol");
    assert_eq!(
        component_metadata_value(product.visibility.as_deref().unwrap_or_default(), "kind")
            .as_deref(),
        Some("class")
    );
    assert!(!parsed.symbols.iter().any(|symbol| {
        symbol.name == "value"
            && component_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "framework",
            )
            .is_some()
    }));
}

#[test]
fn detects_nextjs_use_client_and_server_component_classification() {
    let client = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("client"),
            path: PathBuf::from("app/users/UserPanel.tsx"),
            source: r#"
                    "use client";
                    export function UserPanel() {
                        return <section />;
                    }
                "#
            .to_string(),
        })
        .expect("parse client component");
    let client_component = client
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UserPanel")
        .expect("client component");
    let client_metadata = client_component.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        component_metadata_value(client_metadata, "framework").as_deref(),
        Some("nextjs")
    );
    assert_eq!(
        component_metadata_value(client_metadata, "kind").as_deref(),
        Some("client_component")
    );

    let server = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("server"),
            path: PathBuf::from("app/users/UserList.tsx"),
            source: r#"
                    export default function UserList() {
                        return <section />;
                    }
                "#
            .to_string(),
        })
        .expect("parse server component");
    let server_component = server
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UserList")
        .expect("server component");
    assert_eq!(
        component_metadata_value(
            server_component.visibility.as_deref().unwrap_or_default(),
            "kind"
        )
        .as_deref(),
        Some("server_component")
    );
}

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
