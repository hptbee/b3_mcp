use super::*;

pub(super) fn is_dockerfile(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    file_name == "Dockerfile"
        || file_name.starts_with("Dockerfile.")
        || file_name == ".dockerignore"
}

pub(super) fn collect_dockerfile(input: &ParseInput) -> Vec<ExtractedSymbol> {
    if !is_dockerfile(&input.path) {
        return Vec::new();
    }

    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("FROM ") {
            let mut metadata = metadata(
                input,
                line_number,
                line_number,
                "docker",
                "Image",
                "DockerfileFrom",
            );
            let image = trimmed
                .split_whitespace()
                .nth(1)
                .map(clean_value)
                .unwrap_or_default();
            metadata.name = Some(image.clone());
            metadata.image = Some(image);
            metadata.confidence = 9_000;
            output.push(infrastructure_symbol(
                input,
                line_number,
                "Dockerfile FROM",
                metadata,
            ));
        } else if upper.starts_with("EXPOSE ") {
            let mut metadata = metadata(
                input,
                line_number,
                line_number,
                "docker",
                "Service",
                "DockerfileExpose",
            );
            metadata.ports = trimmed
                .split_whitespace()
                .skip(1)
                .map(clean_value)
                .filter(|value| !value.is_empty())
                .collect();
            metadata.name = metadata.ports.first().cloned();
            metadata.confidence = 8_500;
            output.push(infrastructure_symbol(
                input,
                line_number,
                "Dockerfile EXPOSE",
                metadata,
            ));
        } else if upper.starts_with("ENV ") {
            let mut metadata = metadata(
                input,
                line_number,
                line_number,
                "docker",
                "Container",
                "DockerfileEnv",
            );
            metadata.env_keys = trimmed
                .split_whitespace()
                .skip(1)
                .filter_map(env_key)
                .collect();
            metadata.name = metadata.env_keys.first().cloned();
            metadata.confidence = 8_000;
            output.push(infrastructure_symbol(
                input,
                line_number,
                "Dockerfile ENV",
                metadata,
            ));
        } else if upper.starts_with("CMD ") || upper.starts_with("ENTRYPOINT ") {
            let mut metadata = metadata(
                input,
                line_number,
                line_number,
                "docker",
                "Container",
                if upper.starts_with("CMD ") {
                    "DockerfileCmd"
                } else {
                    "DockerfileEntrypoint"
                },
            );
            metadata.name = Some(trimmed.to_string());
            metadata.confidence = 7_500;
            output.push(infrastructure_symbol(
                input,
                line_number,
                "Dockerfile command",
                metadata,
            ));
        }
    }

    output
}
