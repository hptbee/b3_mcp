use super::*;

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
    assert_eq!(random.language.as_deref(), Some("xml"));
    assert!(random.symbols.iter().any(|symbol| symbol.name == "root"));

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
