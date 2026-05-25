use super::*;

#[derive(Debug, Clone)]
struct TerraformBlock {
    block_type: String,
    resource_type: Option<String>,
    name: String,
    line_start: usize,
    line_end: usize,
    provider: Option<String>,
    image: Option<String>,
    location: Option<String>,
}

impl TerraformBlock {
    fn new(
        block_type: String,
        resource_type: Option<String>,
        name: String,
        line_start: usize,
    ) -> Self {
        Self {
            block_type,
            resource_type,
            name,
            line_start,
            line_end: line_start,
            provider: None,
            image: None,
            location: None,
        }
    }
}

pub(super) fn is_terraform_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("tf" | "tfvars")
    )
}

pub(super) fn collect_terraform(input: &ParseInput) -> Vec<ExtractedSymbol> {
    if !is_terraform_file(&input.path) {
        return Vec::new();
    }
    if input.path.extension().and_then(|value| value.to_str()) == Some("tfvars") {
        return Vec::new();
    }

    let mut output = Vec::new();
    let mut current: Option<TerraformBlock> = None;
    let mut brace_depth = 0usize;

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        if current.is_none() {
            current = parse_block_start(trimmed, line_number);
            if current.is_some() {
                brace_depth = trimmed
                    .matches('{')
                    .count()
                    .saturating_sub(trimmed.matches('}').count());
                if brace_depth == 0 {
                    if let Some(block) = current.take() {
                        push_block(input, &mut output, block);
                    }
                }
            }
        } else if let Some(block) = current.as_mut() {
            block.line_end = line_number;
            if let Some(value) = literal_assignment(trimmed, "provider") {
                block.provider = Some(value);
            }
            if let Some(value) = literal_assignment(trimmed, "image") {
                block.image = Some(value);
            }
            if let Some(value) = literal_assignment(trimmed, "location")
                .or_else(|| literal_assignment(trimmed, "region"))
                .or_else(|| literal_assignment(trimmed, "project"))
                .or_else(|| literal_assignment(trimmed, "name"))
            {
                block.location = Some(value);
            }
            brace_depth = brace_depth
                .saturating_add(trimmed.matches('{').count())
                .saturating_sub(trimmed.matches('}').count());
            if brace_depth == 0 {
                if let Some(block) = current.take() {
                    push_block(input, &mut output, block);
                }
            }
        }
    }

    if let Some(block) = current {
        push_block(input, &mut output, block);
    }
    output
}

fn parse_block_start(line: &str, line_number: usize) -> Option<TerraformBlock> {
    let tokens = quoted_tokens(line);
    if line.starts_with("provider ") {
        return tokens.first().map(|name| {
            TerraformBlock::new("provider".to_string(), None, name.clone(), line_number)
        });
    }
    if line.starts_with("resource ") && tokens.len() >= 2 {
        return Some(TerraformBlock::new(
            "resource".to_string(),
            tokens.first().cloned(),
            tokens.get(1).cloned().unwrap_or_default(),
            line_number,
        ));
    }
    if line.starts_with("module ") {
        return tokens.first().map(|name| {
            TerraformBlock::new("module".to_string(), None, name.clone(), line_number)
        });
    }
    if line.starts_with("variable ") {
        return tokens.first().map(|name| {
            TerraformBlock::new("variable".to_string(), None, name.clone(), line_number)
        });
    }
    if line.starts_with("output ") {
        return tokens.first().map(|name| {
            TerraformBlock::new("output".to_string(), None, name.clone(), line_number)
        });
    }
    if line.starts_with("locals ") {
        return Some(TerraformBlock::new(
            "locals".to_string(),
            None,
            "locals".to_string(),
            line_number,
        ));
    }
    None
}

fn push_block(input: &ParseInput, output: &mut Vec<ExtractedSymbol>, block: TerraformBlock) {
    let resource_type = block.resource_type.clone();
    let (technology, source_kind) = classify_terraform(&block);
    let kind = match block.block_type.as_str() {
        "provider" => "Provider",
        "resource" if resource_type.as_deref() == Some("google_container_cluster") => "Cluster",
        "resource" => "Resource",
        "module" => "Module",
        "variable" => "Variable",
        "output" => "Output",
        _ => "Unknown",
    };

    let mut metadata = metadata(
        input,
        block.line_start,
        block.line_end,
        technology,
        kind,
        source_kind,
    );
    metadata.name = Some(block.name.clone());
    metadata.resource_type = resource_type;
    metadata.provider = block.provider.or_else(|| {
        if technology == "gcp" || technology == "gke" {
            Some("google".to_string())
        } else if block.block_type == "provider" {
            Some(block.name.clone())
        } else {
            None
        }
    });
    metadata.image = block.image;
    if let Some(location) = block.location {
        metadata.labels.push(format!("literal={location}"));
    }
    metadata.confidence = 8_500;
    output.push(infrastructure_symbol(
        input,
        block.line_start,
        &format!("Terraform {} {}", block.block_type, block.name),
        metadata,
    ));
}

fn classify_terraform(block: &TerraformBlock) -> (&'static str, &'static str) {
    match block.resource_type.as_deref() {
        Some("google_container_cluster") => ("gke", "GkeTerraformCluster"),
        Some("google_container_node_pool") => ("gke", "GkeTerraformNodePool"),
        Some(value) if value.starts_with("google_") => ("gcp", "GcpTerraformResource"),
        _ if block.block_type == "provider" && block.name == "google" => {
            ("gcp", "TerraformProvider")
        }
        _ => ("terraform", source_kind_for_block(&block.block_type)),
    }
}

fn source_kind_for_block(block_type: &str) -> &'static str {
    match block_type {
        "provider" => "TerraformProvider",
        "resource" => "TerraformResource",
        "module" => "TerraformModule",
        "variable" => "TerraformVariable",
        "output" => "TerraformOutput",
        _ => "TerraformBlock",
    }
}

fn quoted_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else {
            break;
        };
        tokens.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    tokens
}

fn literal_assignment(line: &str, key: &str) -> Option<String> {
    let (left, right) = line.split_once('=')?;
    if left.trim() != key {
        return None;
    }
    let value = clean_value(right);
    if value.is_empty() || value.contains('(') || value.contains('.') {
        None
    } else {
        Some(value)
    }
}
