use b3_core::normalize_route_path;

pub const UNKNOWN_METHOD: &str = "UNKNOWN";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouteMatchKey {
    pub method: String,
    pub path: String,
    pub normalized_key: String,
}

impl RouteMatchKey {
    pub fn new(method: Option<&str>, path: &str) -> Self {
        let method = normalize_optional_method(method);
        let path = normalize_api_path(path);
        let normalized_key = format!("http:{method}:{path}");
        Self {
            method,
            path,
            normalized_key,
        }
    }
}

pub fn normalize_optional_method(method: Option<&str>) -> String {
    let method = method.unwrap_or_default().trim();
    if method.is_empty() {
        UNKNOWN_METHOD.to_string()
    } else {
        method.to_ascii_uppercase()
    }
}

pub fn normalize_api_path(path: &str) -> String {
    let mut path = normalize_route_path(path);
    path = normalize_colon_params(&path);
    path = normalize_bracket_params(&path, '[', ']');
    path = normalize_bracket_params(&path, '<', '>');
    while path.contains("//") {
        path = path.replace("//", "/");
    }
    if path.len() > 1 {
        path = path.trim_end_matches('/').to_string();
    }
    path
}

pub fn route_pattern_matches(route_pattern: &str, concrete_path: &str) -> bool {
    let route_parts = normalize_api_path(route_pattern)
        .split('/')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let concrete_parts = normalize_api_path(concrete_path)
        .split('/')
        .map(str::to_string)
        .collect::<Vec<_>>();
    if route_parts.len() != concrete_parts.len() {
        return false;
    }
    route_parts
        .iter()
        .zip(concrete_parts.iter())
        .all(|(route, concrete)| is_param(route) || route == concrete)
}

fn normalize_colon_params(path: &str) -> String {
    path.split('/')
        .map(|part| {
            if let Some(name) = part.strip_prefix(':') {
                if safe_param_name(name) {
                    return format!("{{{name}}}");
                }
            }
            part.to_string()
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_bracket_params(path: &str, open: char, close: char) -> String {
    path.split('/')
        .map(|part| {
            let trimmed = part
                .strip_prefix(open)
                .and_then(|value| value.strip_suffix(close));
            if let Some(name) = trimmed {
                let name = name.trim_start_matches("...");
                if safe_param_name(name) {
                    return format!("{{{name}}}");
                }
            }
            part.to_string()
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn safe_param_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_param(value: &str) -> bool {
    value.starts_with('{') && value.ends_with('}') && value.len() > 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_route_keys() {
        assert_eq!(normalize_optional_method(Some(" post ")), "POST");
        assert_eq!(normalize_optional_method(None), "UNKNOWN");
        assert_eq!(normalize_api_path("api//Users/:id/?x=1"), "/api/users/{id}");
        assert_eq!(normalize_api_path("/api/users/[id]/"), "/api/users/{id}");
        assert_eq!(normalize_api_path("/api/users/<id>"), "/api/users/{id}");
        assert_eq!(
            RouteMatchKey::new(Some("get"), "/api/users/[id]").normalized_key,
            "http:GET:/api/users/{id}"
        );
    }

    #[test]
    fn route_patterns_match_concrete_paths() {
        assert!(route_pattern_matches("/api/users/{id}", "/api/users/123"));
        assert!(route_pattern_matches("/api/users/:id", "/api/users/123"));
        assert!(!route_pattern_matches(
            "/api/users/{id}",
            "/api/users/123/orders"
        ));
    }
}
