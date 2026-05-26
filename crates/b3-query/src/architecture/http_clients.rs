use b3_core::{
    ArchitectureConfidence, ArchitectureConfidenceLevel, ArchitectureSource, ArchitectureSourceKind,
};
use b3_storage::StoredFileContent;

use super::match_keys::{normalize_api_path, normalize_optional_method, RouteMatchKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpClientCall {
    pub project_id: String,
    pub file_path: String,
    pub method: Option<String>,
    pub path: String,
    pub base_url: Option<String>,
    pub service_client_name: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: ArchitectureConfidence,
    pub evidence: String,
}

impl HttpClientCall {
    pub fn key(&self) -> RouteMatchKey {
        RouteMatchKey::new(self.method.as_deref(), &self.path)
    }

    pub fn source(&self) -> ArchitectureSource {
        ArchitectureSource {
            project_id: self.project_id.clone(),
            file_path: self.file_path.clone(),
            symbol_id: None,
            line_start: Some(self.line_start),
            line_end: Some(self.line_end),
            source_kind: ArchitectureSourceKind::Unknown,
            extractor: Some("b3-query-http-client-literal".to_string()),
            metadata_key: Some("http_client.path".to_string()),
        }
    }
}

pub fn extract_http_client_calls(
    project_id: &str,
    files: &[StoredFileContent],
) -> Vec<HttpClientCall> {
    let mut calls = Vec::new();
    for file in files {
        let base_urls = literal_base_urls(&file.content);
        for (line_index, line) in file.content.lines().enumerate() {
            let line_number = line_index + 1;
            collect_fetch(project_id, file, line, line_number, &base_urls, &mut calls);
            collect_member_calls(project_id, file, line, line_number, &base_urls, &mut calls);
            collect_csharp(project_id, file, line, line_number, &base_urls, &mut calls);
            collect_go(project_id, file, line, line_number, &base_urls, &mut calls);
        }
    }
    calls.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then_with(|| left.file_path.cmp(&right.file_path))
            .then_with(|| left.line_start.cmp(&right.line_start))
            .then_with(|| left.method.cmp(&right.method))
            .then_with(|| left.path.cmp(&right.path))
    });
    calls.dedup_by(|left, right| {
        left.project_id == right.project_id
            && left.file_path == right.file_path
            && left.line_start == right.line_start
            && left.method == right.method
            && left.path == right.path
    });
    calls
}

fn collect_fetch(
    project_id: &str,
    file: &StoredFileContent,
    line: &str,
    line_number: usize,
    base_urls: &[String],
    calls: &mut Vec<HttpClientCall>,
) {
    for path in literals_after(line, "fetch(") {
        let method = fetch_method(line).unwrap_or_else(|| "GET".to_string());
        push_call(
            project_id,
            file,
            line_number,
            Some(method),
            path,
            base_urls,
            "fetch literal",
            calls,
        );
    }
}

fn collect_member_calls(
    project_id: &str,
    file: &StoredFileContent,
    line: &str,
    line_number: usize,
    base_urls: &[String],
    calls: &mut Vec<HttpClientCall>,
) {
    for (needle, method) in [
        (".get(", "GET"),
        (".post(", "POST"),
        (".put(", "PUT"),
        (".patch(", "PATCH"),
        (".delete(", "DELETE"),
        (".request(", "UNKNOWN"),
    ] {
        for path in literals_after(line, needle) {
            push_call(
                project_id,
                file,
                line_number,
                (method != "UNKNOWN").then(|| method.to_string()),
                path,
                base_urls,
                "member HTTP client literal",
                calls,
            );
        }
    }
}

fn collect_csharp(
    project_id: &str,
    file: &StoredFileContent,
    line: &str,
    line_number: usize,
    base_urls: &[String],
    calls: &mut Vec<HttpClientCall>,
) {
    for (needle, method) in [
        ("GetAsync(", "GET"),
        ("PostAsync(", "POST"),
        ("PutAsync(", "PUT"),
        ("PatchAsync(", "PATCH"),
        ("DeleteAsync(", "DELETE"),
        ("GetFromJsonAsync", "GET"),
        ("PostAsJsonAsync", "POST"),
    ] {
        for path in literals_after(line, needle) {
            push_call(
                project_id,
                file,
                line_number,
                Some(method.to_string()),
                path,
                base_urls,
                "C# HttpClient literal",
                calls,
            );
        }
    }
    if line.contains("HttpRequestMessage") {
        let method = if line.contains("HttpMethod.Post") {
            Some("POST".to_string())
        } else if line.contains("HttpMethod.Put") {
            Some("PUT".to_string())
        } else if line.contains("HttpMethod.Delete") {
            Some("DELETE".to_string())
        } else if line.contains("HttpMethod.Get") {
            Some("GET".to_string())
        } else {
            None
        };
        for path in quoted_literals(line)
            .into_iter()
            .filter(|literal| looks_like_path(literal))
        {
            push_call(
                project_id,
                file,
                line_number,
                method.clone(),
                path,
                base_urls,
                "C# HttpRequestMessage literal",
                calls,
            );
        }
    }
}

fn collect_go(
    project_id: &str,
    file: &StoredFileContent,
    line: &str,
    line_number: usize,
    base_urls: &[String],
    calls: &mut Vec<HttpClientCall>,
) {
    for (needle, method) in [
        ("http.Get(", "GET"),
        ("http.Post(", "POST"),
        (".Get(", "GET"),
    ] {
        for path in literals_after(line, needle) {
            push_call(
                project_id,
                file,
                line_number,
                Some(method.to_string()),
                path,
                base_urls,
                "Go HTTP client literal",
                calls,
            );
        }
    }
    if line.contains("http.NewRequest") {
        let literals = quoted_literals(line);
        if literals.len() >= 2 {
            push_call(
                project_id,
                file,
                line_number,
                Some(normalize_optional_method(Some(&literals[0]))),
                literals[1].clone(),
                base_urls,
                "Go http.NewRequest literal",
                calls,
            );
        }
    }
}

fn push_call(
    project_id: &str,
    file: &StoredFileContent,
    line_number: usize,
    method: Option<String>,
    raw_path: String,
    base_urls: &[String],
    evidence: &str,
    calls: &mut Vec<HttpClientCall>,
) {
    let Some((base_url, path)) = compose_path(raw_path, base_urls) else {
        return;
    };
    let dynamic = path.contains("${") || path.contains('{') && !path.contains('}');
    let confidence = if dynamic {
        ArchitectureConfidence::new(
            ArchitectureConfidenceLevel::Low,
            3_500,
            "dynamic or interpolated HTTP path literal",
            vec![evidence.to_string()],
        )
    } else {
        ArchitectureConfidence::high("literal HTTP client path").with_evidence(evidence)
    };
    calls.push(HttpClientCall {
        project_id: project_id.to_string(),
        file_path: file.path.clone(),
        method,
        path: normalize_api_path(&path),
        base_url,
        service_client_name: None,
        line_start: line_number,
        line_end: line_number,
        confidence,
        evidence: evidence.to_string(),
    });
}

fn compose_path(raw_path: String, base_urls: &[String]) -> Option<(Option<String>, String)> {
    if raw_path.starts_with("http://") || raw_path.starts_with("https://") {
        let after_scheme = raw_path.split_once("://")?.1;
        let path_start = after_scheme.find('/').unwrap_or(after_scheme.len());
        let host = &raw_path[..raw_path.len() - after_scheme.len() + path_start];
        let path = after_scheme
            .get(path_start..)
            .filter(|path| !path.is_empty())
            .unwrap_or("/");
        return Some((Some(host.to_string()), path.to_string()));
    }
    if looks_like_path(&raw_path) {
        return Some((None, raw_path));
    }
    if let Some(base) = base_urls.first() {
        return Some((Some(base.clone()), join_paths(base, &raw_path)));
    }
    None
}

fn literal_base_urls(content: &str) -> Vec<String> {
    let mut bases = Vec::new();
    for line in content.lines() {
        let has_base_hint = line.contains("API_BASE")
            || line.contains("baseURL")
            || line.contains("BaseAddress")
            || line.contains("apiUrl");
        if has_base_hint {
            if let Some(literal) = quoted_literals(line)
                .into_iter()
                .find(|literal| literal.starts_with('/') || literal.starts_with("http"))
            {
                bases.push(literal);
            }
        }
    }
    bases.sort();
    bases.dedup();
    bases
}

fn fetch_method(line: &str) -> Option<String> {
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
        if line.contains(&format!("method: \"{method}\""))
            || line.contains(&format!("method: '{method}'"))
            || line.contains(&format!("method:\"{method}\""))
            || line.contains(&format!("method:'{method}'"))
        {
            return Some(method.to_string());
        }
    }
    None
}

fn literals_after(line: &str, needle: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut rest = line;
    while let Some(index) = rest.find(needle) {
        rest = &rest[index + needle.len()..];
        if let Some(literal) = first_quoted_literal(rest) {
            literals.push(literal);
        }
        if rest.len() <= 1 {
            break;
        }
        rest = &rest[1..];
    }
    literals
        .into_iter()
        .filter(|literal| looks_like_path(literal) || literal.starts_with("http"))
        .collect()
}

fn quoted_literals(line: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut rest = line;
    while let Some(literal) = first_quoted_literal(rest) {
        if let Some(start) = rest.find(&literal) {
            rest = &rest[start + literal.len()..];
        } else {
            break;
        }
        literals.push(literal);
        if rest.len() <= 1 {
            break;
        }
    }
    literals
}

fn first_quoted_literal(text: &str) -> Option<String> {
    let quote_index = text.find(['"', '\'', '`'])?;
    let quote = text.as_bytes()[quote_index] as char;
    let after = &text[quote_index + 1..];
    let end = after.find(quote)?;
    Some(after[..end].to_string())
}

fn looks_like_path(value: &str) -> bool {
    value.starts_with('/') || value.starts_with("api/") || value.starts_with("v1/")
}

fn join_paths(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(content: &str) -> StoredFileContent {
        StoredFileContent {
            id: "file".to_string(),
            project_id: "web".to_string(),
            branch_id: "main".to_string(),
            path: "src/client.ts".to_string(),
            language: Some("typescript".to_string()),
            content: content.to_string(),
        }
    }

    #[test]
    fn extracts_js_ts_http_literals() {
        let calls = extract_http_client_calls(
            "web",
            &[file(
                r#"
                const API_BASE = "/api";
                fetch("/api/orders");
                fetch("/api/users", { method: "POST" });
                axios.get("/api/orders/123");
                this.http.post("/api/users", body);
                apiClient.get("orders");
                "#,
            )],
        );
        assert!(calls
            .iter()
            .any(|call| { call.method.as_deref() == Some("GET") && call.path == "/api/orders" }));
        assert!(calls
            .iter()
            .any(|call| { call.method.as_deref() == Some("POST") && call.path == "/api/users" }));
        assert!(calls.iter().any(|call| call.path == "/api/orders/123"));
    }

    #[test]
    fn extracts_csharp_and_go_literals() {
        let calls = extract_http_client_calls(
            "client",
            &[file(
                r#"
                await httpClient.GetAsync("/api/orders");
                await httpClient.PostAsync("/api/users", body);
                var request = new HttpRequestMessage(HttpMethod.Post, "/api/orders");
                http.Get("/health")
                http.NewRequest("POST", "/api/users", nil)
                "#,
            )],
        );
        assert!(calls.iter().any(|call| call.evidence.contains("C#")));
        assert!(calls.iter().any(|call| call.evidence.contains("Go")));
        assert!(calls
            .iter()
            .any(|call| { call.method.as_deref() == Some("POST") && call.path == "/api/users" }));
    }
}
