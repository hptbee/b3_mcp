//! Local B3 setup helper.
//!
//! This crate only generates and edits local agent config files. It must not
//! execute commands, intercept shells, start daemons, call network services, or
//! touch query/index/storage internals.

use b3_mcp_runtime::ToolProfileName;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    env, fs,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

const DEFAULT_SERVER_NAME: &str = "b3";
const DEFAULT_COMMAND: &str = "b3-mcp-runtime";
const CONTROL_PORT: u16 = 7777;
const WEB_UI_PORT: u16 = 8888;
const REGISTRY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Codex,
    Cursor,
}

impl FromStr for AgentKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "codex" => Ok(Self::Codex),
            "cursor" => Ok(Self::Cursor),
            _ => Err(format!(
                "unsupported agent: {value}; supported agents: codex, cursor"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOptions {
    pub agent: AgentKind,
    pub server_name: String,
    pub command_path: String,
    pub project_path: PathBuf,
    pub database_path: PathBuf,
    pub profile: ToolProfileName,
    pub config_path: Option<PathBuf>,
    pub apply: bool,
    pub backup: bool,
}

impl InstallOptions {
    fn default_for(agent: AgentKind) -> Self {
        let project_path = absolute_path(PathBuf::from("."));
        let database_path = project_path.join(".b3").join("b3.db");
        Self {
            agent,
            server_name: DEFAULT_SERVER_NAME.to_string(),
            command_path: DEFAULT_COMMAND.to_string(),
            project_path,
            database_path,
            profile: ToolProfileName::default(),
            config_path: None,
            apply: false,
            backup: true,
        }
    }

    pub fn target_path(&self) -> PathBuf {
        match (self.agent, &self.config_path) {
            (_, Some(path)) => path.clone(),
            (AgentKind::Codex, None) => codex_config_path(),
            (AgentKind::Cursor, None) => self.project_path.join(".cursor").join("mcp.json"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallOptions {
    pub agent: AgentKind,
    pub server_name: String,
    pub project_path: PathBuf,
    pub config_path: Option<PathBuf>,
    pub apply: bool,
    pub backup: bool,
}

impl UninstallOptions {
    fn default_for(agent: AgentKind) -> Self {
        Self {
            agent,
            server_name: DEFAULT_SERVER_NAME.to_string(),
            project_path: absolute_path(PathBuf::from(".")),
            config_path: None,
            apply: false,
            backup: true,
        }
    }

    pub fn target_path(&self) -> PathBuf {
        match (self.agent, &self.config_path) {
            (_, Some(path)) => path.clone(),
            (AgentKind::Codex, None) => codex_config_path(),
            (AgentKind::Cursor, None) => self.project_path.join(".cursor").join("mcp.json"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorOptions {
    pub project_path: PathBuf,
    pub database_path: PathBuf,
    pub profile: ToolProfileName,
    pub command_path: String,
    pub codex_config_path: PathBuf,
    pub cursor_config_path: PathBuf,
}

impl Default for DoctorOptions {
    fn default() -> Self {
        let project_path = absolute_path(PathBuf::from("."));
        let database_path = project_path.join(".b3").join("b3.db");
        Self {
            cursor_config_path: project_path.join(".cursor").join("mcp.json"),
            project_path,
            database_path,
            profile: ToolProfileName::default(),
            command_path: DEFAULT_COMMAND.to_string(),
            codex_config_path: codex_config_path(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConfigOptions {
    pub agent: AgentKind,
    pub server_name: String,
    pub command_path: String,
    pub project_path: PathBuf,
    pub database_path: PathBuf,
    pub profile: ToolProfileName,
    pub config_path: Option<PathBuf>,
    pub cargo_run: bool,
    pub repo_path: PathBuf,
}

impl McpConfigOptions {
    fn default_for(agent: AgentKind) -> Self {
        let project_path = absolute_path(PathBuf::from("."));
        let database_path = project_path.join(".b3").join("b3.db");
        Self {
            agent,
            server_name: DEFAULT_SERVER_NAME.to_string(),
            command_path: DEFAULT_COMMAND.to_string(),
            project_path,
            database_path,
            profile: ToolProfileName::default(),
            config_path: None,
            cargo_run: false,
            repo_path: absolute_path(PathBuf::from(".")),
        }
    }

    pub fn target_path(&self) -> PathBuf {
        match (self.agent, &self.config_path) {
            (_, Some(path)) => path.clone(),
            (AgentKind::Codex, None) => codex_config_path(),
            (AgentKind::Cursor, None) => self.project_path.join(".cursor").join("mcp.json"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConfigPlan {
    pub target_path: PathBuf,
    pub content: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub target_path: PathBuf,
    pub content: String,
    pub warnings: Vec<String>,
    pub applied: bool,
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub version: u32,
    pub projects: Vec<RegistryProject>,
    pub groups: Vec<RegistryGroup>,
    pub created_at: String,
    pub updated_at: String,
}

impl Registry {
    pub fn empty() -> Self {
        let now = timestamp_string();
        Self {
            version: REGISTRY_VERSION,
            projects: Vec::new(),
            groups: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    fn touch(&mut self) {
        self.updated_at = timestamp_string();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub database: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_indexed_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryGroup {
    pub id: String,
    pub name: String,
    pub description: String,
    pub project_ids: Vec<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterOptions {
    pub project_path: PathBuf,
    pub id: Option<String>,
    pub name: Option<String>,
    pub database_path: Option<PathBuf>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub update: bool,
    pub registry_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryMutationOptions {
    pub registry_path: PathBuf,
    pub apply: bool,
    pub backup: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupCreateOptions {
    pub name: String,
    pub id: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
    pub registry_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMembershipOptions {
    pub group_id: String,
    pub project_id: String,
    pub registry_path: PathBuf,
}

pub fn run_cli(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };

    match command.as_str() {
        "install" => {
            let options = parse_install(args)?;
            let plan = install_plan(&options)?;
            println!("{}", render_install_plan(&options, &plan));
            Ok(())
        }
        "uninstall" => {
            let options = parse_uninstall(args)?;
            let plan = uninstall_plan(&options)?;
            println!("{}", render_uninstall_plan(&options, &plan));
            Ok(())
        }
        "doctor" => {
            let options = parse_doctor(args)?;
            println!("{}", run_doctor(&options));
            Ok(())
        }
        "mcp" => {
            println!("{}", run_mcp(args)?);
            Ok(())
        }
        "register" => {
            let options = parse_register(args)?;
            println!("{}", register_project(&options)?);
            Ok(())
        }
        "unregister" => {
            let (project_id, options) = parse_unregister(args)?;
            println!("{}", unregister_project(&project_id, &options)?);
            Ok(())
        }
        "list" => {
            let registry_path = parse_registry_path_only(args)?;
            println!("{}", list_projects(&registry_path)?);
            Ok(())
        }
        "status" => {
            let (project_id, registry_path) = parse_status(args)?;
            println!("{}", project_status(&project_id, &registry_path)?);
            Ok(())
        }
        "group" => {
            let output = run_group(args)?;
            println!("{output}");
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown b3 command: {other}\n{}", usage())),
    }
}

pub fn install_plan(options: &InstallOptions) -> Result<InstallPlan, String> {
    validate_server_name(&options.server_name)?;
    let target_path = options.target_path();
    let existing = read_optional(&target_path)?;
    let content = match options.agent {
        AgentKind::Codex => update_codex_config(existing.as_deref().unwrap_or(""), options)?,
        AgentKind::Cursor => update_cursor_config(existing.as_deref(), options)?,
    };
    let warnings = path_warnings(
        &options.command_path,
        &options.project_path,
        &options.database_path,
        &target_path,
    );

    let mut backup_path = None;
    if options.apply {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if options.backup && target_path.exists() {
            let backup = backup_path_for(&target_path);
            fs::copy(&target_path, &backup).map_err(|error| error.to_string())?;
            backup_path = Some(backup);
        }
        fs::write(&target_path, &content).map_err(|error| error.to_string())?;
    }

    Ok(InstallPlan {
        target_path,
        content,
        warnings,
        applied: options.apply,
        backup_path,
    })
}

pub fn uninstall_plan(options: &UninstallOptions) -> Result<InstallPlan, String> {
    validate_server_name(&options.server_name)?;
    let target_path = options.target_path();
    let existing = read_optional(&target_path)?.unwrap_or_default();
    let content = match options.agent {
        AgentKind::Codex => remove_codex_server(&existing, &options.server_name)?,
        AgentKind::Cursor => remove_cursor_server(existing.as_str(), &options.server_name)?,
    };
    let warnings = if target_path.exists() {
        Vec::new()
    } else {
        vec![format!(
            "config file does not exist: {}",
            target_path.display()
        )]
    };

    let mut backup_path = None;
    if options.apply {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if options.backup && target_path.exists() {
            let backup = backup_path_for(&target_path);
            fs::copy(&target_path, &backup).map_err(|error| error.to_string())?;
            backup_path = Some(backup);
        }
        fs::write(&target_path, &content).map_err(|error| error.to_string())?;
    }

    Ok(InstallPlan {
        target_path,
        content,
        warnings,
        applied: options.apply,
        backup_path,
    })
}

pub fn codex_snippet(options: &InstallOptions) -> String {
    format!(
        "[mcp_servers.{server}]\ncommand = \"{command}\"\nargs = [\n  \"serve\",\n  \"--project\",\n  \"{project}\",\n  \"--database\",\n  \"{database}\",\n  \"--profile\",\n  \"{profile}\"\n]\nenabled = true\n",
        server = options.server_name,
        command = escape_toml(&options.command_path),
        project = escape_toml(&path_string(&options.project_path)),
        database = escape_toml(&path_string(&options.database_path)),
        profile = options.profile
    )
}

pub fn cursor_server_value(options: &InstallOptions) -> Value {
    json!({
        "command": options.command_path,
        "args": [
            "serve",
            "--project",
            path_string(&options.project_path),
            "--database",
            path_string(&options.database_path),
            "--profile",
            options.profile.to_string()
        ]
    })
}

pub fn mcp_config_plan(options: &McpConfigOptions) -> Result<McpConfigPlan, String> {
    validate_server_name(&options.server_name)?;
    let target_path = options.target_path();
    let content = match options.agent {
        AgentKind::Codex => codex_mcp_template(options),
        AgentKind::Cursor => cursor_mcp_template(options)?,
    };
    let warnings = mcp_config_warnings(options, &target_path);
    Ok(McpConfigPlan {
        target_path,
        content,
        warnings,
    })
}

pub fn cursor_mcp_template(options: &McpConfigOptions) -> Result<String, String> {
    let mut server = json!({
        "command": mcp_command(options),
        "args": mcp_args(options),
    });
    if options.cargo_run {
        server["cwd"] = json!(path_string(&options.repo_path));
    }
    let mut servers = serde_json::Map::new();
    servers.insert(options.server_name.clone(), server);
    let root = json!({ "mcpServers": servers });
    serde_json::to_string_pretty(&root).map_err(|error| error.to_string())
}

pub fn codex_mcp_template(options: &McpConfigOptions) -> String {
    let mut lines = vec![
        format!("[mcp_servers.{}]", options.server_name),
        "enabled = true".to_string(),
        format!("command = \"{}\"", escape_toml(&mcp_command(options))),
        "args = [".to_string(),
    ];
    for arg in mcp_args(options) {
        lines.push(format!("  \"{}\",", escape_toml(&arg)));
    }
    lines.push("]".to_string());
    if options.cargo_run {
        lines.push(format!(
            "cwd = \"{}\"",
            escape_toml(&path_string(&options.repo_path))
        ));
        lines.push("startup_timeout_sec = 30".to_string());
    } else {
        lines.push("startup_timeout_sec = 20".to_string());
    }
    lines.push("tool_timeout_sec = 60".to_string());
    lines.push(String::new());
    lines.join("\n")
}

pub fn mcp_profile_recommendations() -> String {
    [
        "B3 MCP profiles",
        "recommended: optimized - everyday Cursor/Codex use",
        "optional: full - broader tool surface when needed",
        "minimal: tiny - smallest high-value manifest",
        "available: debug, readonly, editing, web-app, enterprise",
        "Git Intelligence MCP tools: not exposed in Phase 21.4.1",
    ]
    .join("\n")
}

pub fn backup_path_for(path: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("config");
    path.with_file_name(format!("{file_name}.b3-backup-{timestamp}"))
}

pub fn run_doctor(options: &DoctorOptions) -> String {
    let mut lines = vec!["B3 doctor".to_string()];
    lines.push(check_line(
        "mcp runtime command",
        command_exists(&options.command_path),
        &options.command_path,
    ));
    lines.push(check_line(
        "project path",
        options.project_path.exists(),
        &path_string(&options.project_path),
    ));
    let database_parent = options.database_path.parent().unwrap_or(Path::new("."));
    lines.push(check_line(
        "database parent",
        database_parent.exists(),
        &path_string(database_parent),
    ));
    lines.push(check_line(
        "database file",
        options.database_path.exists(),
        &path_string(&options.database_path),
    ));
    lines.push(check_line(
        "control port 7777 availability",
        port_available(CONTROL_PORT),
        "127.0.0.1:7777",
    ));
    lines.push(format!(
        "INFO web UI expected port: 127.0.0.1:{WEB_UI_PORT}"
    ));
    let codex_parent = options.codex_config_path.parent().unwrap_or(Path::new("."));
    lines.push(check_line(
        "codex config parent",
        codex_parent.exists(),
        &path_string(codex_parent),
    ));
    let cursor_parent = options
        .cursor_config_path
        .parent()
        .unwrap_or(Path::new("."));
    lines.push(check_line(
        "cursor config parent",
        cursor_parent.exists() || options.project_path.exists(),
        &path_string(cursor_parent),
    ));
    lines.push(format!("OK selected profile: {}", options.profile));
    lines.push(
        "INFO hooks_enabled=false; hooks are not installed or enabled by this command".to_string(),
    );
    lines.push(next_steps(&options.project_path, &options.database_path));
    lines.join("\n")
}

pub fn registry_path() -> PathBuf {
    b3_home().join("registry.json")
}

pub fn load_registry(path: &Path) -> Result<Registry, String> {
    if !path.exists() {
        return Ok(Registry::empty());
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content)
        .map_err(|error| format!("invalid registry JSON at {}: {error}", path.display()))
}

pub fn save_registry(
    path: &Path,
    registry: &Registry,
    backup: bool,
) -> Result<Option<PathBuf>, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let backup_path = if backup && path.exists() {
        let backup = backup_path_for(path);
        fs::copy(path, &backup).map_err(|error| error.to_string())?;
        Some(backup)
    } else {
        None
    };
    let json = serde_json::to_string_pretty(registry).map_err(|error| error.to_string())?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, format!("{json}\n")).map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| error.to_string())?;
    Ok(backup_path)
}

pub fn register_project(options: &RegisterOptions) -> Result<String, String> {
    if !options.project_path.exists() {
        return Err(format!(
            "project path does not exist: {}",
            options.project_path.display()
        ));
    }
    let mut registry = load_registry(&options.registry_path)?;
    let project_path = absolute_path(options.project_path.clone());
    let database_path = options
        .database_path
        .clone()
        .map(absolute_path)
        .unwrap_or_else(|| project_path.join(".b3").join("b3.db"));
    let default_name = project_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project")
        .to_string();
    let name = options.name.clone().unwrap_or(default_name);
    let id = options.id.clone().unwrap_or_else(|| slugify(&name));
    validate_registry_id(&id)?;
    let now = timestamp_string();
    let path_text = path_string(&project_path);
    let existing_index = registry
        .projects
        .iter()
        .position(|project| project.id == id || same_path_text(&project.path, &path_text));

    if let Some(index) = existing_index {
        if !options.update {
            return Err(
                "project already exists; pass --update to update the existing registry entry"
                    .to_string(),
            );
        }
        let created_at = registry.projects[index].created_at.clone();
        registry.projects[index] = RegistryProject {
            id: id.clone(),
            name: name.clone(),
            path: path_text,
            database: path_string(&database_path),
            tags: options.tags.clone(),
            created_at,
            updated_at: now,
            last_indexed_at: registry.projects[index].last_indexed_at.clone(),
            notes: options
                .notes
                .clone()
                .or_else(|| registry.projects[index].notes.clone()),
        };
    } else {
        registry.projects.push(RegistryProject {
            id: id.clone(),
            name: name.clone(),
            path: path_text,
            database: path_string(&database_path),
            tags: options.tags.clone(),
            created_at: now.clone(),
            updated_at: now,
            last_indexed_at: None,
            notes: options.notes.clone(),
        });
    }
    registry
        .projects
        .sort_by(|left, right| left.id.cmp(&right.id));
    registry.touch();
    save_registry(&options.registry_path, &registry, true)?;

    Ok(format!(
        "registered project: {id}\nname: {name}\nregistry: {}\n{}\ninstall next step:\n  b3 install --agent codex --project \"{}\" --database \"{}\" --profile optimized --dry-run",
        options.registry_path.display(),
        next_steps(&project_path, &database_path),
        project_path.display(),
        database_path.display()
    ))
}

pub fn unregister_project(
    project_id: &str,
    options: &RegistryMutationOptions,
) -> Result<String, String> {
    validate_registry_id(project_id)?;
    let mut registry = load_registry(&options.registry_path)?;
    let before = registry.projects.len();
    if options.apply {
        registry.projects.retain(|project| project.id != project_id);
        if registry.projects.len() == before {
            return Err(format!("project not found: {project_id}"));
        }
        for group in &mut registry.groups {
            group.project_ids.retain(|id| id != project_id);
            group.updated_at = timestamp_string();
        }
        registry.touch();
        let backup = save_registry(&options.registry_path, &registry, options.backup)?;
        Ok(format!(
            "unregistered project: {project_id}\nmode: applied\nbackup: {}",
            backup
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        ))
    } else {
        let found = registry
            .projects
            .iter()
            .any(|project| project.id == project_id);
        if !found {
            return Err(format!("project not found: {project_id}"));
        }
        Ok(format!(
            "unregister project: {project_id}\nmode: dry-run\nregistry unchanged; rerun with --apply"
        ))
    }
}

pub fn list_projects(path: &Path) -> Result<String, String> {
    let registry = load_registry(path)?;
    if registry.projects.is_empty() {
        return Ok(format!("registry: {}\nprojects: none", path.display()));
    }
    let mut lines = vec![
        format!("registry: {}", path.display()),
        "projects:".to_string(),
    ];
    for project in registry.projects {
        lines.push(format!(
            "- {} | {} | path_exists={} | db_exists={} | tags={}",
            project.id,
            project.name,
            Path::new(&project.path).exists(),
            Path::new(&project.database).exists(),
            project.tags.join(",")
        ));
        lines.push(format!("  path: {}", project.path));
        lines.push(format!("  database: {}", project.database));
    }
    Ok(lines.join("\n"))
}

pub fn project_status(project_id: &str, path: &Path) -> Result<String, String> {
    let registry = load_registry(path)?;
    let project = registry
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .ok_or_else(|| format!("project not found: {project_id}"))?;
    let project_path = Path::new(&project.path);
    let database_path = Path::new(&project.database);
    let b3_dir = project_path.join(".b3");
    Ok(format!(
        "project: {}\nname: {}\npath: {}\npath_exists: {}\ndatabase: {}\ndatabase_exists: {}\nb3_dir_exists: {}\ntags: {}\nlast_indexed_at: {}",
        project.id,
        project.name,
        project.path,
        project_path.exists(),
        project.database,
        database_path.exists(),
        b3_dir.exists(),
        project.tags.join(","),
        project.last_indexed_at.as_deref().unwrap_or("unknown")
    ))
}

pub fn group_create(options: &GroupCreateOptions) -> Result<String, String> {
    let mut registry = load_registry(&options.registry_path)?;
    let id = options.id.clone().unwrap_or_else(|| slugify(&options.name));
    validate_registry_id(&id)?;
    if registry.groups.iter().any(|group| group.id == id) {
        return Err(format!("group already exists: {id}"));
    }
    let now = timestamp_string();
    registry.groups.push(RegistryGroup {
        id: id.clone(),
        name: options.name.clone(),
        description: options.description.clone(),
        project_ids: Vec::new(),
        tags: options.tags.clone(),
        created_at: now.clone(),
        updated_at: now,
    });
    registry
        .groups
        .sort_by(|left, right| left.id.cmp(&right.id));
    registry.touch();
    save_registry(&options.registry_path, &registry, true)?;
    Ok(format!(
        "created group: {id}\nregistry: {}",
        options.registry_path.display()
    ))
}

pub fn group_add(options: &GroupMembershipOptions) -> Result<String, String> {
    let mut registry = load_registry(&options.registry_path)?;
    if !registry
        .projects
        .iter()
        .any(|project| project.id == options.project_id)
    {
        return Err(format!("project not found: {}", options.project_id));
    }
    let group = registry
        .groups
        .iter_mut()
        .find(|group| group.id == options.group_id)
        .ok_or_else(|| format!("group not found: {}", options.group_id))?;
    if !group.project_ids.contains(&options.project_id) {
        group.project_ids.push(options.project_id.clone());
        group.updated_at = timestamp_string();
    }
    registry.touch();
    save_registry(&options.registry_path, &registry, true)?;
    Ok(format!(
        "added project {} to group {}",
        options.project_id, options.group_id
    ))
}

pub fn group_remove(options: &GroupMembershipOptions) -> Result<String, String> {
    let mut registry = load_registry(&options.registry_path)?;
    let group = registry
        .groups
        .iter_mut()
        .find(|group| group.id == options.group_id)
        .ok_or_else(|| format!("group not found: {}", options.group_id))?;
    group.project_ids.retain(|id| id != &options.project_id);
    group.updated_at = timestamp_string();
    registry.touch();
    save_registry(&options.registry_path, &registry, true)?;
    Ok(format!(
        "removed project {} from group {}",
        options.project_id, options.group_id
    ))
}

pub fn group_list(path: &Path) -> Result<String, String> {
    let registry = load_registry(path)?;
    if registry.groups.is_empty() {
        return Ok(format!("registry: {}\ngroups: none", path.display()));
    }
    let mut lines = vec![
        format!("registry: {}", path.display()),
        "groups:".to_string(),
    ];
    for group in registry.groups {
        lines.push(format!(
            "- {} | {} | projects={} | tags={}",
            group.id,
            group.name,
            group.project_ids.len(),
            group.tags.join(",")
        ));
    }
    Ok(lines.join("\n"))
}

pub fn group_status(group_id: &str, path: &Path) -> Result<String, String> {
    let registry = load_registry(path)?;
    let group = registry
        .groups
        .iter()
        .find(|group| group.id == group_id)
        .ok_or_else(|| format!("group not found: {group_id}"))?;
    let mut lines = vec![
        format!("group: {}", group.id),
        format!("name: {}", group.name),
        format!("description: {}", group.description),
        format!("tags: {}", group.tags.join(",")),
        "projects:".to_string(),
    ];
    for project_id in &group.project_ids {
        match registry
            .projects
            .iter()
            .find(|project| &project.id == project_id)
        {
            Some(project) => lines.push(format!(
                "- {} | {} | path_exists={} | db_exists={}",
                project.id,
                project.name,
                Path::new(&project.path).exists(),
                Path::new(&project.database).exists()
            )),
            None => lines.push(format!("- {project_id} | missing registry project")),
        }
    }
    Ok(lines.join("\n"))
}

fn parse_install(args: impl Iterator<Item = String>) -> Result<InstallOptions, String> {
    let mut agent = None;
    let mut rest = Vec::new();
    let mut iter = args.peekable();
    while let Some(arg) = iter.next() {
        if arg == "--agent" {
            agent = Some(next_arg(&mut iter, "--agent")?.parse()?);
        } else {
            rest.push(arg);
        }
    }
    let mut options = InstallOptions::default_for(
        agent.ok_or_else(|| "install requires --agent <codex|cursor>".to_string())?,
    );
    parse_shared_install_options(rest.into_iter(), &mut options)?;
    Ok(options)
}

fn run_mcp(args: impl Iterator<Item = String>) -> Result<String, String> {
    let mut args = args.peekable();
    let subcommand = next_arg(&mut args, "mcp <config|doctor|profiles>")?;
    match subcommand.as_str() {
        "config" => {
            let agent: AgentKind = next_arg(&mut args, "mcp config <codex|cursor>")?.parse()?;
            let options = parse_mcp_config(agent, args)?;
            let plan = mcp_config_plan(&options)?;
            Ok(render_mcp_config_plan(&options, &plan))
        }
        "doctor" => {
            let options = parse_doctor(args)?;
            Ok(run_doctor(&options))
        }
        "profiles" => Ok(mcp_profile_recommendations()),
        "--help" | "-h" => Err(usage()),
        _ => Err(format!("unknown mcp subcommand: {subcommand}\n{}", usage())),
    }
}

fn parse_mcp_config(
    agent: AgentKind,
    args: impl Iterator<Item = String>,
) -> Result<McpConfigOptions, String> {
    let mut options = McpConfigOptions::default_for(agent);
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server-name" => options.server_name = next_arg(&mut args, "--server-name")?,
            "--command" => options.command_path = next_arg(&mut args, "--command")?,
            "--project" => options.project_path = absolute_path(next_path(&mut args, "--project")?),
            "--database" => {
                options.database_path = absolute_path(next_path(&mut args, "--database")?)
            }
            "--profile" | "--tool-profile" => {
                options.profile = next_arg(&mut args, &arg)?.parse()?;
            }
            "--config" => options.config_path = Some(next_path(&mut args, "--config")?),
            "--cargo-run" => options.cargo_run = true,
            "--repo" => options.repo_path = absolute_path(next_path(&mut args, "--repo")?),
            "--dry-run" => {}
            "--write" | "--apply" => {
                return Err(
                    "b3 mcp config is print-only; use b3 install --agent <codex|cursor> --apply for safe writes with backups"
                        .to_string(),
                );
            }
            "--force" => {
                return Err(
                    "b3 mcp config does not force-overwrite files; use b3 install --apply --backup for safe writes"
                        .to_string(),
                );
            }
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown mcp config argument: {arg}")),
        }
    }
    Ok(options)
}

fn parse_shared_install_options(
    args: impl Iterator<Item = String>,
    options: &mut InstallOptions,
) -> Result<(), String> {
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server-name" => options.server_name = next_arg(&mut args, "--server-name")?,
            "--command" => options.command_path = next_arg(&mut args, "--command")?,
            "--project" => options.project_path = absolute_path(next_path(&mut args, "--project")?),
            "--database" => {
                options.database_path = absolute_path(next_path(&mut args, "--database")?)
            }
            "--profile" | "--tool-profile" => {
                options.profile = next_arg(&mut args, &arg)?.parse()?;
            }
            "--config" => options.config_path = Some(next_path(&mut args, "--config")?),
            "--dry-run" => options.apply = false,
            "--apply" | "--write" => options.apply = true,
            "--backup" => options.backup = true,
            "--no-backup" => options.backup = false,
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown install argument: {arg}")),
        }
    }
    Ok(())
}

fn parse_uninstall(args: impl Iterator<Item = String>) -> Result<UninstallOptions, String> {
    let mut agent = None;
    let mut rest = Vec::new();
    let mut iter = args.peekable();
    while let Some(arg) = iter.next() {
        if arg == "--agent" {
            agent = Some(next_arg(&mut iter, "--agent")?.parse()?);
        } else {
            rest.push(arg);
        }
    }
    let mut options = UninstallOptions::default_for(
        agent.ok_or_else(|| "uninstall requires --agent <codex|cursor>".to_string())?,
    );
    let mut args = rest.into_iter().peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server-name" => options.server_name = next_arg(&mut args, "--server-name")?,
            "--project" => options.project_path = absolute_path(next_path(&mut args, "--project")?),
            "--config" => options.config_path = Some(next_path(&mut args, "--config")?),
            "--dry-run" => options.apply = false,
            "--apply" | "--write" => options.apply = true,
            "--backup" => options.backup = true,
            "--no-backup" => options.backup = false,
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown uninstall argument: {arg}")),
        }
    }
    Ok(options)
}

fn parse_doctor(args: impl Iterator<Item = String>) -> Result<DoctorOptions, String> {
    let mut options = DoctorOptions::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                options.project_path = absolute_path(next_path(&mut args, "--project")?);
                options.cursor_config_path = options.project_path.join(".cursor").join("mcp.json");
            }
            "--database" => {
                options.database_path = absolute_path(next_path(&mut args, "--database")?)
            }
            "--profile" | "--tool-profile" => {
                options.profile = next_arg(&mut args, &arg)?.parse()?;
            }
            "--command" => options.command_path = next_arg(&mut args, "--command")?,
            "--codex-config" => options.codex_config_path = next_path(&mut args, "--codex-config")?,
            "--cursor-config" => {
                options.cursor_config_path = next_path(&mut args, "--cursor-config")?
            }
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown doctor argument: {arg}")),
        }
    }
    Ok(options)
}

fn parse_register(args: impl Iterator<Item = String>) -> Result<RegisterOptions, String> {
    let mut args = args.peekable();
    let project_path = absolute_path(next_path(&mut args, "register <project-path>")?);
    let mut options = RegisterOptions {
        project_path,
        id: None,
        name: None,
        database_path: None,
        tags: Vec::new(),
        notes: None,
        update: false,
        registry_path: registry_path(),
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--id" => options.id = Some(next_arg(&mut args, "--id")?),
            "--name" => options.name = Some(next_arg(&mut args, "--name")?),
            "--database" => options.database_path = Some(next_path(&mut args, "--database")?),
            "--tag" => options.tags.push(next_arg(&mut args, "--tag")?),
            "--notes" => options.notes = Some(next_arg(&mut args, "--notes")?),
            "--update" => options.update = true,
            "--registry" => options.registry_path = next_path(&mut args, "--registry")?,
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown register argument: {arg}")),
        }
    }
    Ok(options)
}

fn parse_unregister(
    args: impl Iterator<Item = String>,
) -> Result<(String, RegistryMutationOptions), String> {
    let mut args = args.peekable();
    let project_id = next_arg(&mut args, "unregister <project-id>")?;
    let mut options = RegistryMutationOptions {
        registry_path: registry_path(),
        apply: false,
        backup: true,
    };
    parse_registry_mutation_options(&mut args, &mut options)?;
    Ok((project_id, options))
}

fn parse_registry_mutation_options(
    args: &mut impl Iterator<Item = String>,
    options: &mut RegistryMutationOptions,
) -> Result<(), String> {
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--registry" => options.registry_path = next_path(args, "--registry")?,
            "--dry-run" => options.apply = false,
            "--apply" | "--write" => options.apply = true,
            "--backup" => options.backup = true,
            "--no-backup" => options.backup = false,
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown registry mutation argument: {arg}")),
        }
    }
    Ok(())
}

fn parse_registry_path_only(args: impl Iterator<Item = String>) -> Result<PathBuf, String> {
    let mut registry = registry_path();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--registry" => registry = next_path(&mut args, "--registry")?,
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    Ok(registry)
}

fn parse_status(args: impl Iterator<Item = String>) -> Result<(String, PathBuf), String> {
    let mut args = args.peekable();
    let project_id = next_arg(&mut args, "status <project-id>")?;
    let registry = parse_registry_path_only(args)?;
    Ok((project_id, registry))
}

fn run_group(args: impl Iterator<Item = String>) -> Result<String, String> {
    let mut args = args.peekable();
    let Some(command) = args.next() else {
        return Err("group requires a subcommand: create, add, remove, list, status".to_string());
    };
    match command.as_str() {
        "create" => {
            let options = parse_group_create(args)?;
            group_create(&options)
        }
        "add" => {
            let options = parse_group_membership(args)?;
            group_add(&options)
        }
        "remove" => {
            let options = parse_group_membership(args)?;
            group_remove(&options)
        }
        "list" => {
            let registry = parse_registry_path_only(args)?;
            group_list(&registry)
        }
        "status" => {
            let mut args = args.peekable();
            let group_id = next_arg(&mut args, "group status <group-id>")?;
            let registry = parse_registry_path_only(args)?;
            group_status(&group_id, &registry)
        }
        "delete" => Err(
            "group delete is deferred; edit registry with explicit backup if needed".to_string(),
        ),
        _ => Err(format!("unknown group command: {command}")),
    }
}

fn parse_group_create(args: impl Iterator<Item = String>) -> Result<GroupCreateOptions, String> {
    let mut args = args.peekable();
    let name = next_arg(&mut args, "group create <group-name>")?;
    let mut options = GroupCreateOptions {
        name,
        id: None,
        description: String::new(),
        tags: Vec::new(),
        registry_path: registry_path(),
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--id" => options.id = Some(next_arg(&mut args, "--id")?),
            "--description" => options.description = next_arg(&mut args, "--description")?,
            "--tag" => options.tags.push(next_arg(&mut args, "--tag")?),
            "--registry" => options.registry_path = next_path(&mut args, "--registry")?,
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown group create argument: {arg}")),
        }
    }
    Ok(options)
}

fn parse_group_membership(
    args: impl Iterator<Item = String>,
) -> Result<GroupMembershipOptions, String> {
    let mut args = args.peekable();
    let group_id = next_arg(&mut args, "group <add|remove> <group-id> <project-id>")?;
    let project_id = next_arg(&mut args, "group <add|remove> <group-id> <project-id>")?;
    let mut registry_path_value = registry_path();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--registry" => registry_path_value = next_path(&mut args, "--registry")?,
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown group membership argument: {arg}")),
        }
    }
    Ok(GroupMembershipOptions {
        group_id,
        project_id,
        registry_path: registry_path_value,
    })
}

fn update_codex_config(existing: &str, options: &InstallOptions) -> Result<String, String> {
    validate_toml_like(existing)?;
    let without_existing = remove_codex_server(existing, &options.server_name)?;
    let mut output = without_existing.trim_end().to_string();
    if !output.is_empty() {
        output.push_str("\n\n");
    }
    output.push_str(&codex_snippet(options));
    Ok(output)
}

fn remove_codex_server(existing: &str, server_name: &str) -> Result<String, String> {
    validate_toml_like(existing)?;
    let header = format!("[mcp_servers.{server_name}]");
    let mut output = Vec::new();
    let mut skipping = false;
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed == header {
            skipping = true;
            continue;
        }
        if skipping && trimmed.starts_with('[') && trimmed.ends_with(']') {
            skipping = false;
        }
        if !skipping {
            output.push(line);
        }
    }
    Ok(output.join("\n").trim_end().to_string())
}

fn update_cursor_config(
    existing: Option<&str>,
    options: &InstallOptions,
) -> Result<String, String> {
    let mut root = match existing {
        Some(text) if !text.trim().is_empty() => {
            serde_json::from_str::<Value>(text).map_err(|error| {
                format!("invalid JSON config; not overwriting without a valid parse: {error}")
            })?
        }
        _ => json!({}),
    };
    if !root.is_object() {
        return Err("cursor config root must be a JSON object".to_string());
    }
    if root.get("mcpServers").is_none() {
        root["mcpServers"] = json!({});
    }
    let Some(servers) = root["mcpServers"].as_object_mut() else {
        return Err("cursor config mcpServers must be an object".to_string());
    };
    servers.insert(options.server_name.clone(), cursor_server_value(options));
    serde_json::to_string_pretty(&root).map_err(|error| error.to_string())
}

fn remove_cursor_server(existing: &str, server_name: &str) -> Result<String, String> {
    if existing.trim().is_empty() {
        return Ok("{\n  \"mcpServers\": {}\n}".to_string());
    }
    let mut root = serde_json::from_str::<Value>(existing).map_err(|error| {
        format!("invalid JSON config; not overwriting without a valid parse: {error}")
    })?;
    if let Some(servers) = root.get_mut("mcpServers").and_then(Value::as_object_mut) {
        servers.remove(server_name);
    }
    serde_json::to_string_pretty(&root).map_err(|error| error.to_string())
}

fn validate_toml_like(existing: &str) -> Result<(), String> {
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.ends_with(']') {
            return Err(format!(
                "invalid TOML-like header; not overwriting without a clear section boundary: {trimmed}"
            ));
        }
    }
    Ok(())
}

fn render_install_plan(options: &InstallOptions, plan: &InstallPlan) -> String {
    let mut lines = vec![
        format!("agent: {:?}", options.agent).to_lowercase(),
        format!("target: {}", plan.target_path.display()),
        format!("mode: {}", if plan.applied { "applied" } else { "dry-run" }),
        format!("profile: {}", options.profile),
    ];
    if let Some(path) = &plan.backup_path {
        lines.push(format!("backup: {}", path.display()));
    }
    lines.extend(
        plan.warnings
            .iter()
            .map(|warning| format!("warning: {warning}")),
    );
    lines.push("generated config:".to_string());
    lines.push(plan.content.clone());
    lines.push(next_steps(&options.project_path, &options.database_path));
    lines.push(
        "hooks: disabled by default; no shell interception or command capture installed"
            .to_string(),
    );
    if !plan.applied {
        lines.push("next step: rerun with --apply to write this config".to_string());
    }
    lines.join("\n")
}

fn render_mcp_config_plan(options: &McpConfigOptions, plan: &McpConfigPlan) -> String {
    let mode = if options.cargo_run {
        "cargo-run"
    } else {
        "binary"
    };
    let mut lines = vec![
        format!("agent: {:?}", options.agent).to_lowercase(),
        format!("target: {}", plan.target_path.display()),
        "mode: dry-run".to_string(),
        format!("runtime mode: {mode}"),
        format!("profile: {}", options.profile),
    ];
    lines.extend(
        plan.warnings
            .iter()
            .map(|warning| format!("warning: {warning}")),
    );
    lines.push("generated config template:".to_string());
    lines.push(plan.content.clone());
    lines.push(
        "write safety: print-only; use b3 install --agent <codex|cursor> --apply to merge config with backups"
            .to_string(),
    );
    lines.push(next_steps(&options.project_path, &options.database_path));
    lines.push(
        "Git MCP tools: not exposed by this setup helper; existing B3 MCP tools are unchanged"
            .to_string(),
    );
    lines.join("\n")
}

fn render_uninstall_plan(options: &UninstallOptions, plan: &InstallPlan) -> String {
    let mut lines = vec![
        format!("agent: {:?}", options.agent).to_lowercase(),
        format!("target: {}", plan.target_path.display()),
        format!("mode: {}", if plan.applied { "applied" } else { "dry-run" }),
        format!("removed server: {}", options.server_name),
    ];
    if let Some(path) = &plan.backup_path {
        lines.push(format!("backup: {}", path.display()));
    }
    lines.extend(
        plan.warnings
            .iter()
            .map(|warning| format!("warning: {warning}")),
    );
    lines.push("resulting config:".to_string());
    lines.push(plan.content.clone());
    if !plan.applied {
        lines.push("next step: rerun with --apply to write this config".to_string());
    }
    lines.join("\n")
}

fn path_warnings(
    command_path: &str,
    project_path: &Path,
    database_path: &Path,
    target_path: &Path,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if !command_exists(command_path) {
        warnings.push(format!(
            "MCP runtime command was not found locally: {command_path}"
        ));
    }
    if !project_path.exists() {
        warnings.push(format!(
            "project path does not exist: {}",
            project_path.display()
        ));
    }
    if let Some(parent) = database_path.parent() {
        if !parent.exists() {
            warnings.push(format!(
                ".b3/database parent does not exist: {}",
                parent.display()
            ));
        }
    }
    if !database_path.exists() {
        warnings.push(format!(
            "database file does not exist yet: {}",
            database_path.display()
        ));
    }
    if let Some(parent) = target_path.parent() {
        if !parent.exists() {
            warnings.push(format!(
                "config parent does not exist yet and will be created on apply: {}",
                parent.display()
            ));
        }
    }
    warnings
}

fn mcp_config_warnings(options: &McpConfigOptions, target_path: &Path) -> Vec<String> {
    let command = mcp_command(options);
    let mut warnings = path_warnings(
        &command,
        &options.project_path,
        &options.database_path,
        target_path,
    );
    if options.cargo_run && !options.repo_path.exists() {
        warnings.push(format!(
            "B3 repo path does not exist yet: {}",
            options.repo_path.display()
        ));
    }
    if options.cargo_run {
        warnings.push(
            "cargo-run templates are useful for local source checkouts but start more slowly than installed binaries"
                .to_string(),
        );
    }
    warnings.push(
        "generated template is dry-run only; no Cursor or Codex config file was written"
            .to_string(),
    );
    warnings
}

fn mcp_command(options: &McpConfigOptions) -> String {
    if options.cargo_run {
        "cargo".to_string()
    } else {
        options.command_path.clone()
    }
}

fn mcp_args(options: &McpConfigOptions) -> Vec<String> {
    let mut args = Vec::new();
    if options.cargo_run {
        args.extend([
            "run".to_string(),
            "-p".to_string(),
            "b3-mcp-runtime".to_string(),
            "--".to_string(),
        ]);
    }
    args.extend([
        "serve".to_string(),
        "--project".to_string(),
        path_string(&options.project_path),
        "--database".to_string(),
        path_string(&options.database_path),
        "--profile".to_string(),
        options.profile.to_string(),
    ]);
    args
}

fn next_steps(project_path: &Path, database_path: &Path) -> String {
    format!(
        "next steps:\n  b3-control-server init --project \"{}\" --database \"{}\"\n  b3-control-server index --project \"{}\" --database \"{}\"\n  b3-control-server serve --project \"{}\" --database \"{}\" --port 7777",
        project_path.display(),
        database_path.display(),
        project_path.display(),
        database_path.display(),
        project_path.display(),
        database_path.display()
    )
}

fn check_line(name: &str, ok: bool, detail: &str) -> String {
    format!("{} {name}: {detail}", if ok { "OK" } else { "WARN" })
}

fn port_available(port: u16) -> bool {
    TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)).is_ok()
}

fn command_exists(command_path: &str) -> bool {
    let path = Path::new(command_path);
    if path.components().count() > 1 || path.is_absolute() {
        return path.exists();
    }
    env::var_os("PATH")
        .map(|paths| {
            env::split_paths(&paths).any(|entry| {
                let candidate = entry.join(command_path);
                candidate.exists() || candidate.with_extension("exe").exists()
            })
        })
        .unwrap_or(false)
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    if path.exists() {
        fs::read_to_string(path)
            .map(Some)
            .map_err(|error| error.to_string())
    } else {
        Ok(None)
    }
}

fn validate_server_name(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("server name must not be empty".to_string());
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        Ok(())
    } else {
        Err("server name must contain only ASCII letters, numbers, '_' or '-'".to_string())
    }
}

fn next_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn next_path(args: &mut impl Iterator<Item = String>, name: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(next_arg(args, name)?))
}

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn b3_home() -> PathBuf {
    env::var_os("B3_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".b3")))
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".b3")))
        .unwrap_or_else(|| PathBuf::from(".b3"))
}

fn codex_config_path() -> PathBuf {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("config.toml")
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn escape_toml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn slugify(value: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !output.is_empty() {
            output.push('-');
            previous_dash = true;
        }
    }
    while output.ends_with('-') {
        output.pop();
    }
    if output.is_empty() {
        "project".to_string()
    } else {
        output
    }
}

fn validate_registry_id(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("id must not be empty".to_string());
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        Ok(())
    } else {
        Err("id must contain only lowercase ASCII letters, numbers, or '-'".to_string())
    }
}

fn same_path_text(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn usage() -> String {
    "usage:\n  b3 mcp config <codex|cursor> --project <path> --database <path> --profile <profile> [--cargo-run --repo <path>]\n  b3 mcp doctor --project <path> --database <path> --profile <profile>\n  b3 mcp profiles\n  b3 install --agent <codex|cursor> --project <path> --database <path> --profile <profile> [--dry-run|--apply] [--backup|--no-backup]\n  b3 uninstall --agent <codex|cursor> [--dry-run|--apply]\n  b3 doctor --project <path> --database <path> --profile <profile>\n  b3 register <project-path> [--name <name>] [--id <id>] [--database <path>] [--tag <tag>] [--update]\n  b3 unregister <project-id> [--dry-run|--apply]\n  b3 list\n  b3 status <project-id>\n  b3 group create <group-name> [--id <id>] [--description <text>]\n  b3 group add <group-id> <project-id>\n  b3 group remove <group-id> <project-id>\n  b3 group list\n  b3 group status <group-id>".to_string()
}

fn print_help() {
    println!("{}", usage());
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn options(agent: AgentKind, config_path: PathBuf) -> InstallOptions {
        let dir = tempdir().expect("tempdir");
        InstallOptions {
            agent,
            server_name: "b3".to_string(),
            command_path: "C:\\Tools\\b3\\b3-mcp-runtime.exe".to_string(),
            project_path: dir.path().join("project"),
            database_path: dir.path().join("project").join(".b3").join("b3.db"),
            profile: ToolProfileName::Optimized,
            config_path: Some(config_path),
            apply: false,
            backup: true,
        }
    }

    #[test]
    fn codex_config_snippet_includes_profile_and_scope_paths() {
        let dir = tempdir().expect("tempdir");
        let opts = options(AgentKind::Codex, dir.path().join("config.toml"));
        let snippet = codex_snippet(&opts);

        assert!(snippet.contains("[mcp_servers.b3]"));
        assert!(snippet.contains("--profile"));
        assert!(snippet.contains("optimized"));
        assert!(snippet.contains("enabled = true"));
    }

    #[test]
    fn cursor_config_snippet_includes_profile_and_server_name() {
        let dir = tempdir().expect("tempdir");
        let mut opts = options(AgentKind::Cursor, dir.path().join("mcp.json"));
        opts.server_name = "local_b3".to_string();
        opts.profile = ToolProfileName::Tiny;
        let content = update_cursor_config(None, &opts).expect("json");

        assert!(content.contains("\"local_b3\""));
        assert!(content.contains("\"--profile\""));
        assert!(content.contains("\"tiny\""));
    }

    #[test]
    fn mcp_cursor_template_generates_binary_json() {
        let dir = tempdir().expect("tempdir");
        let options = McpConfigOptions {
            agent: AgentKind::Cursor,
            server_name: "b3".to_string(),
            command_path: "C:\\Tools\\b3\\b3-mcp-runtime.exe".to_string(),
            project_path: dir.path().join("project with spaces"),
            database_path: dir
                .path()
                .join("project with spaces")
                .join(".b3")
                .join("b3.db"),
            profile: ToolProfileName::Optimized,
            config_path: Some(dir.path().join(".cursor").join("mcp.json")),
            cargo_run: false,
            repo_path: dir.path().to_path_buf(),
        };
        let content = cursor_mcp_template(&options).expect("template");
        let parsed: Value = serde_json::from_str(&content).expect("json");
        let server = &parsed["mcpServers"]["b3"];

        assert_eq!(server["command"], "C:\\Tools\\b3\\b3-mcp-runtime.exe");
        assert!(server["args"].as_array().unwrap().contains(&json!("serve")));
        assert!(server["args"]
            .as_array()
            .unwrap()
            .contains(&json!("optimized")));
        assert!(content.contains("\\\\Tools\\\\b3"));
    }

    #[test]
    fn mcp_codex_template_generates_cargo_run_toml() {
        let dir = tempdir().expect("tempdir");
        let options = McpConfigOptions {
            agent: AgentKind::Codex,
            server_name: "b3".to_string(),
            command_path: "b3-mcp-runtime".to_string(),
            project_path: dir.path().join("project"),
            database_path: dir.path().join("project").join(".b3").join("b3.db"),
            profile: ToolProfileName::Full,
            config_path: Some(dir.path().join("config.toml")),
            cargo_run: true,
            repo_path: PathBuf::from("C:\\Repos\\b3_mcp"),
        };
        let content = codex_mcp_template(&options);

        assert!(content.contains("[mcp_servers.b3]"));
        assert!(content.contains("command = \"cargo\""));
        assert!(content.contains("\"run\","));
        assert!(content.contains("\"b3-mcp-runtime\","));
        assert!(content.contains("\"full\","));
        assert!(content.contains("cwd = \"C:\\\\Repos\\\\b3_mcp\""));
        assert!(content.contains("startup_timeout_sec = 30"));
        assert!(content.contains("tool_timeout_sec = 60"));
    }

    #[test]
    fn mcp_config_plan_warns_without_writing() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join(".cursor").join("mcp.json");
        let options = McpConfigOptions {
            agent: AgentKind::Cursor,
            server_name: "b3".to_string(),
            command_path: "definitely-not-b3-mcp-runtime".to_string(),
            project_path: dir.path().join("missing"),
            database_path: dir.path().join("missing").join(".b3").join("b3.db"),
            profile: ToolProfileName::Optimized,
            config_path: Some(target.clone()),
            cargo_run: false,
            repo_path: dir.path().to_path_buf(),
        };
        let plan = mcp_config_plan(&options).expect("plan");

        assert!(!target.exists());
        assert!(plan
            .warnings
            .iter()
            .any(|line| line.contains("project path does not exist")));
        assert!(plan
            .warnings
            .iter()
            .any(|line| line.contains("database file does not exist")));
        assert!(plan
            .warnings
            .iter()
            .any(|line| line.contains("dry-run only")));
    }

    #[test]
    fn mcp_config_rejects_write_to_preserve_explicit_install_safety() {
        let result = run_cli([
            "mcp".to_string(),
            "config".to_string(),
            "cursor".to_string(),
            "--write".to_string(),
        ]);

        assert!(result.expect_err("write error").contains("print-only"));
    }

    #[test]
    fn mcp_profile_recommendations_do_not_add_git_tools() {
        let output = mcp_profile_recommendations();

        assert!(output.contains("optimized"));
        assert!(output.contains("full"));
        assert!(output.contains("tiny"));
        assert!(!output.contains("git_status"));
        assert!(!output.contains("git_changed_files"));
    }

    #[test]
    fn dry_run_does_not_write_files() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let opts = options(AgentKind::Codex, path.clone());

        let plan = install_plan(&opts).expect("plan");

        assert!(!plan.applied);
        assert!(!path.exists());
    }

    #[test]
    fn backup_filename_contains_marker() {
        let path = PathBuf::from("config.toml");
        let backup = backup_path_for(&path);

        assert!(backup.to_string_lossy().contains(".b3-backup-"));
    }

    #[test]
    fn codex_update_is_idempotent_and_preserves_unrelated_config() {
        let dir = tempdir().expect("tempdir");
        let opts = options(AgentKind::Codex, dir.path().join("config.toml"));
        let first = update_codex_config("theme = \"dark\"\n", &opts).expect("first");
        let second = update_codex_config(&first, &opts).expect("second");

        assert!(second.contains("theme = \"dark\""));
        assert_eq!(second.matches("[mcp_servers.b3]").count(), 1);
    }

    #[test]
    fn cursor_update_is_idempotent_and_preserves_other_servers() {
        let dir = tempdir().expect("tempdir");
        let opts = options(AgentKind::Cursor, dir.path().join("mcp.json"));
        let existing = r#"{"mcpServers":{"other":{"command":"x","args":[]}}}"#;
        let first = update_cursor_config(Some(existing), &opts).expect("first");
        let second = update_cursor_config(Some(&first), &opts).expect("second");
        let parsed: Value = serde_json::from_str(&second).expect("json");

        assert!(parsed["mcpServers"].get("other").is_some());
        assert!(parsed["mcpServers"].get("b3").is_some());
        assert_eq!(parsed["mcpServers"].as_object().unwrap().len(), 2);
    }

    #[test]
    fn invalid_profile_is_rejected() {
        let result = run_cli([
            "doctor".to_string(),
            "--profile".to_string(),
            "invalid".to_string(),
        ]);

        assert!(result.expect_err("error").contains("invalid tool profile"));
    }

    #[test]
    fn custom_profile_and_server_name_are_used() {
        let dir = tempdir().expect("tempdir");
        let mut opts = options(AgentKind::Codex, dir.path().join("config.toml"));
        opts.server_name = "workspace".to_string();
        opts.profile = ToolProfileName::Enterprise;
        let snippet = codex_snippet(&opts);

        assert!(snippet.contains("[mcp_servers.workspace]"));
        assert!(snippet.contains("\"enterprise\""));
    }

    #[test]
    fn invalid_json_is_not_overwritten() {
        let dir = tempdir().expect("tempdir");
        let opts = options(AgentKind::Cursor, dir.path().join("mcp.json"));
        let error = update_cursor_config(Some("{ invalid"), &opts).expect_err("invalid");

        assert!(error.contains("invalid JSON config"));
    }

    #[test]
    fn invalid_toml_like_header_is_not_overwritten() {
        let dir = tempdir().expect("tempdir");
        let opts = options(AgentKind::Codex, dir.path().join("config.toml"));
        let error = update_codex_config("[broken\nvalue = true", &opts).expect_err("invalid");

        assert!(error.contains("invalid TOML-like header"));
    }

    #[test]
    fn apply_creates_backup_and_repeated_apply_does_not_duplicate() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        fs::write(&path, "existing = true\n").expect("write");
        let mut opts = options(AgentKind::Codex, path.clone());
        opts.apply = true;

        let first = install_plan(&opts).expect("first");
        let second = install_plan(&opts).expect("second");
        let content = fs::read_to_string(&path).expect("content");

        assert!(first.backup_path.is_some());
        assert!(second.backup_path.is_some());
        assert_eq!(content.matches("[mcp_servers.b3]").count(), 1);
    }

    #[test]
    fn doctor_reports_local_path_checks() {
        let dir = tempdir().expect("tempdir");
        let options = DoctorOptions {
            project_path: dir.path().to_path_buf(),
            database_path: dir.path().join(".b3").join("b3.db"),
            profile: ToolProfileName::Optimized,
            command_path: "definitely-not-a-b3-command".to_string(),
            codex_config_path: dir.path().join(".codex").join("config.toml"),
            cursor_config_path: dir.path().join(".cursor").join("mcp.json"),
        };
        let output = run_doctor(&options);

        assert!(output.contains("B3 doctor"));
        assert!(output.contains("selected profile: optimized"));
        assert!(output.contains("hooks_enabled=false"));
    }

    #[test]
    fn uninstall_dry_run_removes_only_named_cursor_server() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"b3":{"command":"b3","args":[]},"other":{"command":"x","args":[]}}}"#,
        )
        .expect("write");
        let opts = UninstallOptions {
            agent: AgentKind::Cursor,
            server_name: "b3".to_string(),
            project_path: dir.path().to_path_buf(),
            config_path: Some(path.clone()),
            apply: false,
            backup: true,
        };

        let plan = uninstall_plan(&opts).expect("plan");
        let original = fs::read_to_string(&path).expect("original");

        assert!(!plan.applied);
        assert!(!plan.content.contains("\"b3\""));
        assert!(plan.content.contains("\"other\""));
        assert!(original.contains("\"b3\""));
    }

    #[test]
    fn no_external_network_dependency_is_required() {
        let dir = tempdir().expect("tempdir");
        let options = DoctorOptions {
            project_path: dir.path().to_path_buf(),
            database_path: dir.path().join(".b3").join("b3.db"),
            profile: ToolProfileName::Optimized,
            command_path: "b3-mcp-runtime".to_string(),
            codex_config_path: dir.path().join(".codex").join("config.toml"),
            cursor_config_path: dir.path().join(".cursor").join("mcp.json"),
        };

        let output = run_doctor(&options);
        assert!(!output.contains("http://"));
        assert!(!output.contains("https://"));
    }

    #[test]
    fn registry_file_creation_round_trips_json() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("registry.json");
        let registry = Registry::empty();

        save_registry(&path, &registry, false).expect("save");
        let loaded = load_registry(&path).expect("load");

        assert_eq!(loaded.version, REGISTRY_VERSION);
        assert!(loaded.projects.is_empty());
        assert!(loaded.groups.is_empty());
    }

    #[test]
    fn register_project_creates_entry_and_status_checks_paths() {
        let dir = tempdir().expect("tempdir");
        let project = dir.path().join("Backend API");
        fs::create_dir_all(project.join(".b3")).expect("project");
        let database = project.join(".b3").join("b3.db");
        fs::write(&database, "").expect("db");
        let registry = dir.path().join("registry.json");

        let output = register_project(&RegisterOptions {
            project_path: project.clone(),
            id: None,
            name: Some("Backend API".to_string()),
            database_path: Some(database.clone()),
            tags: vec!["api".to_string(), "backend".to_string()],
            notes: None,
            update: false,
            registry_path: registry.clone(),
        })
        .expect("register");
        let status = project_status("backend-api", &registry).expect("status");

        assert!(output.contains("registered project: backend-api"));
        assert!(status.contains("path_exists: true"));
        assert!(status.contains("database_exists: true"));
        assert!(status.contains("api,backend"));
    }

    #[test]
    fn duplicate_project_requires_update() {
        let dir = tempdir().expect("tempdir");
        let project = dir.path().join("app");
        fs::create_dir_all(&project).expect("project");
        let registry = dir.path().join("registry.json");
        let options = RegisterOptions {
            project_path: project,
            id: Some("app".to_string()),
            name: Some("App".to_string()),
            database_path: None,
            tags: Vec::new(),
            notes: None,
            update: false,
            registry_path: registry,
        };

        register_project(&options).expect("first");
        let error = register_project(&options).expect_err("duplicate");

        assert!(error.contains("--update"));
    }

    #[test]
    fn unregister_dry_run_and_apply_preserve_files() {
        let dir = tempdir().expect("tempdir");
        let project = dir.path().join("app");
        fs::create_dir_all(project.join(".b3")).expect("project");
        let db = project.join(".b3").join("b3.db");
        fs::write(&db, "db").expect("db");
        let registry = dir.path().join("registry.json");
        register_project(&RegisterOptions {
            project_path: project.clone(),
            id: Some("app".to_string()),
            name: Some("App".to_string()),
            database_path: Some(db.clone()),
            tags: Vec::new(),
            notes: None,
            update: false,
            registry_path: registry.clone(),
        })
        .expect("register");

        let dry = unregister_project(
            "app",
            &RegistryMutationOptions {
                registry_path: registry.clone(),
                apply: false,
                backup: true,
            },
        )
        .expect("dry");
        assert!(dry.contains("dry-run"));
        assert!(project_status("app", &registry).is_ok());

        unregister_project(
            "app",
            &RegistryMutationOptions {
                registry_path: registry.clone(),
                apply: true,
                backup: true,
            },
        )
        .expect("apply");
        assert!(project_status("app", &registry).is_err());
        assert!(db.exists());
    }

    #[test]
    fn project_list_output_includes_registered_project() {
        let dir = tempdir().expect("tempdir");
        let project = dir.path().join("app");
        fs::create_dir_all(&project).expect("project");
        let registry = dir.path().join("registry.json");
        register_project(&RegisterOptions {
            project_path: project,
            id: Some("app".to_string()),
            name: Some("App".to_string()),
            database_path: None,
            tags: vec!["rust".to_string()],
            notes: None,
            update: false,
            registry_path: registry.clone(),
        })
        .expect("register");

        let list = list_projects(&registry).expect("list");
        assert!(list.contains("app | App"));
        assert!(list.contains("tags=rust"));
    }

    #[test]
    fn group_create_add_remove_list_and_status() {
        let dir = tempdir().expect("tempdir");
        let project = dir.path().join("frontend");
        fs::create_dir_all(&project).expect("project");
        let registry = dir.path().join("registry.json");
        register_project(&RegisterOptions {
            project_path: project,
            id: Some("frontend-app".to_string()),
            name: Some("Frontend App".to_string()),
            database_path: None,
            tags: Vec::new(),
            notes: None,
            update: false,
            registry_path: registry.clone(),
        })
        .expect("register");
        group_create(&GroupCreateOptions {
            name: "Business Application".to_string(),
            id: Some("business-app".to_string()),
            description: "metadata only".to_string(),
            tags: vec!["workspace".to_string()],
            registry_path: registry.clone(),
        })
        .expect("group");
        group_add(&GroupMembershipOptions {
            group_id: "business-app".to_string(),
            project_id: "frontend-app".to_string(),
            registry_path: registry.clone(),
        })
        .expect("add");

        let list = group_list(&registry).expect("list");
        let status = group_status("business-app", &registry).expect("status");
        assert!(list.contains("business-app | Business Application | projects=1"));
        assert!(status.contains("frontend-app | Frontend App"));

        group_remove(&GroupMembershipOptions {
            group_id: "business-app".to_string(),
            project_id: "frontend-app".to_string(),
            registry_path: registry.clone(),
        })
        .expect("remove");
        let status = group_status("business-app", &registry).expect("status");
        assert!(!status.contains("frontend-app | Frontend App"));
    }

    #[test]
    fn group_add_requires_project_exists() {
        let dir = tempdir().expect("tempdir");
        let registry = dir.path().join("registry.json");
        group_create(&GroupCreateOptions {
            name: "Group".to_string(),
            id: Some("group".to_string()),
            description: String::new(),
            tags: Vec::new(),
            registry_path: registry.clone(),
        })
        .expect("group");
        let error = group_add(&GroupMembershipOptions {
            group_id: "group".to_string(),
            project_id: "missing".to_string(),
            registry_path: registry,
        })
        .expect_err("missing project");

        assert!(error.contains("project not found"));
    }

    #[test]
    fn invalid_registry_json_fails_clearly() {
        let dir = tempdir().expect("tempdir");
        let registry = dir.path().join("registry.json");
        fs::write(&registry, "{ invalid").expect("write");

        let error = load_registry(&registry).expect_err("invalid");
        assert!(error.contains("invalid registry JSON"));
    }

    #[test]
    fn registry_commands_do_not_scan_without_explicit_command() {
        let dir = tempdir().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let list = list_projects(&registry).expect("empty");

        assert!(list.contains("projects: none"));
        assert!(!registry.exists());
    }

    #[test]
    fn registry_helpers_are_offline_and_local_only() {
        let dir = tempdir().expect("tempdir");
        let project = dir.path().join("app");
        fs::create_dir_all(&project).expect("project");
        let registry = dir.path().join("registry.json");
        let output = register_project(&RegisterOptions {
            project_path: project,
            id: Some("app".to_string()),
            name: Some("App".to_string()),
            database_path: None,
            tags: Vec::new(),
            notes: None,
            update: false,
            registry_path: registry,
        })
        .expect("register");

        assert!(!output.contains("http://"));
        assert!(!output.contains("https://"));
    }
}
