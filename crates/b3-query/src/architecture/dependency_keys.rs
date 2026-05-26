#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageEcosystem {
    Npm,
    Dotnet,
    Go,
    Rust,
    Python,
    Unknown,
}

impl PackageEcosystem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Dotnet => "dotnet",
            Self::Go => "go",
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_filter(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "npm" | "node" | "javascript" | "typescript" => Self::Npm,
            "dotnet" | ".net" | "csharp" | "csproj" => Self::Dotnet,
            "go" | "golang" => Self::Go,
            "rust" | "cargo" | "crate" => Self::Rust,
            "python" | "py" => Self::Python,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageMatchKey {
    pub ecosystem: PackageEcosystem,
    pub name: String,
    pub normalized_key: String,
}

impl PackageMatchKey {
    pub fn new(ecosystem: PackageEcosystem, name: &str) -> Self {
        let name = normalize_package_name_for_ecosystem(ecosystem, name);
        let normalized_key = format!("package:{}:{name}", ecosystem.as_str());
        Self {
            ecosystem,
            name,
            normalized_key,
        }
    }
}

pub fn normalize_package_name_for_ecosystem(ecosystem: PackageEcosystem, value: &str) -> String {
    let trimmed = value.trim().trim_matches(['"', '\'', '`']);
    match ecosystem {
        PackageEcosystem::Npm => trimmed.to_ascii_lowercase(),
        PackageEcosystem::Dotnet => trimmed.to_ascii_lowercase(),
        PackageEcosystem::Go => trimmed.trim_end_matches('/').to_ascii_lowercase(),
        PackageEcosystem::Rust => trimmed.to_ascii_lowercase().replace('_', "-"),
        PackageEcosystem::Python => trimmed.to_ascii_lowercase().replace('_', "-"),
        PackageEcosystem::Unknown => trimmed.to_ascii_lowercase(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContractKind {
    Dto,
    Model,
    Interface,
    Type,
    Enum,
    OpenApi,
    Graphql,
    Protobuf,
    Avro,
    JsonSchema,
    Unknown,
}

impl ContractKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dto => "dto",
            Self::Model => "model",
            Self::Interface => "interface",
            Self::Type => "type",
            Self::Enum => "enum",
            Self::OpenApi => "openapi",
            Self::Graphql => "graphql",
            Self::Protobuf => "protobuf",
            Self::Avro => "avro",
            Self::JsonSchema => "json_schema",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_filter(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "dto" => Self::Dto,
            "model" => Self::Model,
            "interface" => Self::Interface,
            "type" => Self::Type,
            "enum" => Self::Enum,
            "openapi" | "swagger" => Self::OpenApi,
            "graphql" | "gql" => Self::Graphql,
            "protobuf" | "proto" => Self::Protobuf,
            "avro" => Self::Avro,
            "json_schema" | "json-schema" | "schema" => Self::JsonSchema,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractMatchKey {
    pub kind: ContractKind,
    pub name: String,
    pub normalized_name: String,
    pub normalized_key: String,
}

impl ContractMatchKey {
    pub fn new(kind: ContractKind, name: &str) -> Self {
        let normalized_name = normalize_contract_name(name);
        let normalized_key = format!("contract:{}:{normalized_name}", kind.as_str());
        Self {
            kind,
            name: name.trim().to_string(),
            normalized_name,
            normalized_key,
        }
    }
}

pub fn normalize_contract_name(value: &str) -> String {
    value
        .trim()
        .trim_matches(['"', '\'', '`'])
        .replace('\\', "/")
        .split('/')
        .next_back()
        .unwrap_or_default()
        .trim_end_matches(".schema.json")
        .trim_end_matches(".json")
        .trim_end_matches(".graphql")
        .trim_end_matches(".proto")
        .trim_end_matches(".avsc")
        .to_ascii_lowercase()
}

pub fn is_generic_contract_name(value: &str) -> bool {
    matches!(
        normalize_contract_name(value).as_str(),
        "user" | "request" | "response" | "model" | "item" | "data"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InfraKind {
    DockerComposeService,
    DockerImage,
    K8sService,
    K8sDeployment,
    K8sConfigMap,
    K8sSecret,
    TerraformResource,
    TerraformModule,
    Database,
    Cache,
    Queue,
    Pubsub,
    Unknown,
}

impl InfraKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DockerComposeService => "docker_compose_service",
            Self::DockerImage => "docker_image",
            Self::K8sService => "k8s_service",
            Self::K8sDeployment => "k8s_deployment",
            Self::K8sConfigMap => "k8s_configmap",
            Self::K8sSecret => "k8s_secret",
            Self::TerraformResource => "terraform_resource",
            Self::TerraformModule => "terraform_module",
            Self::Database => "database",
            Self::Cache => "cache",
            Self::Queue => "queue",
            Self::Pubsub => "pubsub",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_filter(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "docker_compose_service" | "compose_service" => Self::DockerComposeService,
            "docker_image" | "image" => Self::DockerImage,
            "k8s_service" | "kubernetes_service" | "service" => Self::K8sService,
            "k8s_deployment" | "deployment" => Self::K8sDeployment,
            "k8s_configmap" | "configmap" => Self::K8sConfigMap,
            "k8s_secret" | "secret" => Self::K8sSecret,
            "terraform_resource" | "terraform" | "resource" => Self::TerraformResource,
            "terraform_module" | "module" => Self::TerraformModule,
            "database" | "db" => Self::Database,
            "cache" => Self::Cache,
            "queue" => Self::Queue,
            "pubsub" | "topic" => Self::Pubsub,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InfraMatchKey {
    pub kind: InfraKind,
    pub name: String,
    pub namespace: Option<String>,
    pub normalized_key: String,
}

impl InfraMatchKey {
    pub fn new(kind: InfraKind, name: &str, namespace: Option<&str>) -> Self {
        let name = normalize_infra_name(name);
        let namespace = namespace
            .map(normalize_infra_name)
            .filter(|value| !value.is_empty());
        let normalized_key = match &namespace {
            Some(namespace) => format!("infra:{}:{namespace}:{name}", kind.as_str()),
            None => format!("infra:{}:{name}", kind.as_str()),
        };
        Self {
            kind,
            name,
            namespace,
            normalized_key,
        }
    }
}

pub fn normalize_infra_name(value: &str) -> String {
    value
        .trim()
        .trim_matches(['"', '\'', '`'])
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_package_keys() {
        assert_eq!(
            PackageMatchKey::new(PackageEcosystem::Npm, " @Local/Shared-UI ").normalized_key,
            "package:npm:@local/shared-ui"
        );
        assert_eq!(
            PackageMatchKey::new(PackageEcosystem::Dotnet, "Company.Shared.Contracts")
                .normalized_key,
            "package:dotnet:company.shared.contracts"
        );
        assert_eq!(
            PackageMatchKey::new(PackageEcosystem::Go, "github.com/Example/Contracts/")
                .normalized_key,
            "package:go:github.com/example/contracts"
        );
        assert_eq!(
            PackageMatchKey::new(PackageEcosystem::Rust, "b3_core").normalized_key,
            "package:rust:b3-core"
        );
        assert_eq!(
            PackageMatchKey::new(PackageEcosystem::Unknown, " Shared ").normalized_key,
            "package:unknown:shared"
        );
    }

    #[test]
    fn normalizes_contract_and_infra_keys() {
        assert_eq!(
            ContractMatchKey::new(ContractKind::Dto, "CreateOrderRequest").normalized_key,
            "contract:dto:createorderrequest"
        );
        assert!(is_generic_contract_name("Response"));
        assert_eq!(
            ContractMatchKey::new(
                ContractKind::JsonSchema,
                "schemas/order-created.schema.json"
            )
            .normalized_name,
            "order-created"
        );
        assert_eq!(
            InfraMatchKey::new(InfraKind::DockerComposeService, " API ", None).normalized_key,
            "infra:docker_compose_service:api"
        );
        assert_eq!(
            InfraMatchKey::new(InfraKind::K8sService, "orders-api", Some("Default")).normalized_key,
            "infra:k8s_service:default:orders-api"
        );
    }
}
