use super::*;

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
