pub(crate) fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "pwd",
        "secret",
        "token",
        "api_key",
        "apikey",
        "private_key",
        "access_key",
        "client_secret",
        "connection_string",
        "conn_string",
        "connectionstring",
        "credential",
        "auth",
        "bearer",
        "jwt",
        "certificate",
        "cert",
        "signing_key",
        "encryption_key",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

pub(crate) fn value_class(key: &str, value: &str, force_redacted: bool) -> &'static str {
    if force_redacted || is_sensitive_key(key) {
        "secret_like"
    } else if value.trim().is_empty() {
        "empty"
    } else if is_placeholder(value) {
        "placeholder"
    } else if value.contains("://") && !looks_like_connection_string(value) {
        "url"
    } else if value.contains('.') && !value.contains(' ') {
        "literal"
    } else {
        "scalar"
    }
}

pub(crate) fn safe_value_hint(key: &str, value: &str, force_redacted: bool) -> Option<String> {
    let class = value_class(key, value, force_redacted);
    if matches!(class, "secret_like" | "empty") {
        return None;
    }
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string();
    if value.len() > 120 || looks_like_connection_string(&value) {
        return Some(class.to_string());
    }
    if matches!(class, "url" | "literal" | "placeholder") {
        Some(value)
    } else {
        Some(class.to_string())
    }
}

pub(crate) fn env_refs(value: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut rest = value;
    while let Some(index) = rest.find("${") {
        rest = &rest[index + 2..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let name = rest[..end].split(':').next().unwrap_or_default().trim();
        if is_env_name(name) {
            refs.push(name.to_string());
        }
        rest = &rest[end + 1..];
    }
    for part in value.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric())) {
        if part.len() > 2
            && part
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
        {
            refs.push(part.to_string());
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn is_env_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn is_placeholder(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("${")
        || trimmed.starts_with('%')
        || trimmed.starts_with("#{")
        || trimmed.eq_ignore_ascii_case("changeme")
        || trimmed.eq_ignore_ascii_case("example")
}

fn looks_like_connection_string(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("user id=")
        || lower.contains("password=")
        || lower.contains("pwd=")
        || lower.contains("server=")
        || lower.contains("data source=")
        || lower.contains("mongodb://")
        || lower.contains("postgres://")
        || lower.contains("mysql://")
}
