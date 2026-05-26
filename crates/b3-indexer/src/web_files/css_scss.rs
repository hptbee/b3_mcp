use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let language = language_from_path(&input.path).unwrap_or_else(|| "css".to_string());
    let mut symbols = vec![module_symbol(&input, &language)];
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        for selector in selector_names(trimmed, '.') {
            symbols.push(symbol(
                &input,
                &language,
                selector.clone(),
                NodeKind::ConfigKey,
                line_number,
                format!(
                    "css.selector=class;css.name={selector};css.file={}",
                    normalized_file(&input)
                ),
            ));
        }
        for selector in selector_names(trimmed, '#') {
            symbols.push(symbol(
                &input,
                &language,
                selector.clone(),
                NodeKind::ConfigKey,
                line_number,
                format!(
                    "css.selector=id;css.name={selector};css.file={}",
                    normalized_file(&input)
                ),
            ));
        }
        if let Some(name) = trimmed.strip_prefix("--").and_then(|v| v.split(':').next()) {
            symbols.push(symbol(
                &input,
                &language,
                name.trim(),
                NodeKind::Variable,
                line_number,
                format!(
                    "css.custom_property=true;css.name={};css.file={}",
                    name.trim(),
                    normalized_file(&input)
                ),
            ));
        }
        if language == "scss" {
            if let Some(name) = trimmed.strip_prefix('$').and_then(|v| v.split(':').next()) {
                symbols.push(symbol(
                    &input,
                    &language,
                    name.trim(),
                    NodeKind::Variable,
                    line_number,
                    format!(
                        "scss.variable=true;scss.name={};scss.file={}",
                        name.trim(),
                        normalized_file(&input)
                    ),
                ));
            }
            if let Some(name) = trimmed
                .strip_prefix("@mixin ")
                .and_then(|v| v.split(['(', ' ']).next())
            {
                symbols.push(symbol(
                    &input,
                    &language,
                    name,
                    NodeKind::Function,
                    line_number,
                    format!(
                        "scss.mixin=true;scss.name={name};scss.file={}",
                        normalized_file(&input)
                    ),
                ));
            }
        }
        if let Some(name) = trimmed
            .strip_prefix("@keyframes ")
            .and_then(|v| v.split_whitespace().next())
        {
            symbols.push(symbol(
                &input,
                &language,
                name,
                NodeKind::Function,
                line_number,
                format!(
                    "css.keyframes=true;css.name={name};css.file={}",
                    normalized_file(&input)
                ),
            ));
        }
        if let Some(condition) = media_condition(trimmed) {
            symbols.push(symbol(
                &input,
                &language,
                condition.clone(),
                NodeKind::ConfigKey,
                line_number,
                format!(
                    "css.media_query={condition};css.file={}",
                    normalized_file(&input)
                ),
            ));
        }
        for marker in ["@import ", "@use ", "@forward ", "url("] {
            if let Some(path) = literal_after(trimmed, marker) {
                symbols.push(symbol(
                    &input,
                    &language,
                    path.clone(),
                    NodeKind::Package,
                    line_number,
                    format!(
                        "css.reference=true;css.path={path};css.file={}",
                        normalized_file(&input)
                    ),
                ));
            }
        }

        fn media_condition(line: &str) -> Option<String> {
            line.strip_prefix("@media ")
                .map(|value| value.trim().trim_end_matches('{').trim().to_string())
                .filter(|value| !value.is_empty())
        }
    }
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some(language),
        symbols,
        relationships: Vec::new(),
    })
}

fn selector_names(line: &str, marker: char) -> Vec<String> {
    let Some(selector_part) = line.split('{').next() else {
        return Vec::new();
    };
    selector_part
        .split([',', ' ', '>', '+', '~', ':'])
        .filter_map(|part| part.strip_prefix(marker))
        .map(|name| {
            name.chars()
                .take_while(|ch| *ch == '-' || *ch == '_' || ch.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .collect()
}
