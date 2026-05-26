use super::*;

#[test]
fn detects_nextjs_app_router_routes_dynamic_segments_and_groups() {
    let cases = [
        ("app/page.tsx", "GET /", "NextAppPage", "page"),
        ("app/users/page.tsx", "GET /users", "NextAppPage", "page"),
        (
            "app/users/[id]/page.tsx",
            "GET /users/:id",
            "NextAppPage",
            "page",
        ),
        (
            "app/blog/[...slug]/page.tsx",
            "GET /blog/*slug",
            "NextAppPage",
            "page",
        ),
        (
            "app/docs/[[...slug]]/page.tsx",
            "GET /docs/*slug?",
            "NextAppPage",
            "page",
        ),
        ("app/(marketing)/page.tsx", "GET /", "NextAppPage", "page"),
        ("app/layout.tsx", "GET /", "NextAppLayout", "layout"),
        ("app/loading.tsx", "GET /", "NextAppLoading", "loading"),
        ("app/error.tsx", "GET /", "NextAppError", "error"),
        ("app/not-found.tsx", "GET /", "NextAppNotFound", "not_found"),
    ];
    for (path, name, source_kind, route_kind) in cases {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new(path),
                path: PathBuf::from(path),
                source: "export default function Page() { return <main />; }".to_string(),
            })
            .expect("parse next app route");
        let route = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.kind == NodeKind::Route && symbol.name == name)
            .unwrap_or_else(|| panic!("route {name} for {path}"));
        let metadata = route.visibility.as_deref().unwrap_or_default();
        assert_eq!(
            route_metadata_value(metadata, "framework").as_deref(),
            Some("nextjs")
        );
        assert_eq!(
            route_metadata_value(metadata, "source").as_deref(),
            Some(source_kind)
        );
        assert_eq!(
            route_metadata_value(metadata, "kind").as_deref(),
            Some(route_kind)
        );
    }
}

#[test]
fn detects_nextjs_app_route_handler_methods_and_edges() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("next-route"),
            path: PathBuf::from("app/api/users/[id]/route.ts"),
            source: r#"
                    export async function GET() {
                        return Response.json([]);
                    }

                    export const PATCH = async () => Response.json({});
                    export function DELETE() {}
                "#
            .to_string(),
        })
        .expect("parse next route handler");
    for name in [
        "GET /api/users/:id",
        "PATCH /api/users/:id",
        "DELETE /api/users/:id",
    ] {
        let route = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.kind == NodeKind::Route && symbol.name == name)
            .unwrap_or_else(|| panic!("route {name}"));
        let metadata = route.visibility.as_deref().unwrap_or_default();
        assert_eq!(
            route_metadata_value(metadata, "framework").as_deref(),
            Some("nextjs")
        );
        assert_eq!(
            route_metadata_value(metadata, "source").as_deref(),
            Some("NextAppRouteHandler")
        );
        assert_eq!(
            route_metadata_value(metadata, "kind").as_deref(),
            Some("api")
        );
    }
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::References));
}

#[test]
fn detects_nextjs_pages_router_pages_and_api_routes() {
    let cases = [
        ("pages/index.tsx", "GET /", "NextPagesPage", "page"),
        (
            "pages/users/index.tsx",
            "GET /users",
            "NextPagesPage",
            "page",
        ),
        (
            "pages/users/[id].tsx",
            "GET /users/:id",
            "NextPagesPage",
            "page",
        ),
        (
            "pages/blog/[...slug].tsx",
            "GET /blog/*slug",
            "NextPagesPage",
            "page",
        ),
        (
            "pages/api/users.ts",
            "GET /api/users",
            "NextPagesApiRoute",
            "api",
        ),
        (
            "pages/api/users/[id].ts",
            "GET /api/users/:id",
            "NextPagesApiRoute",
            "api",
        ),
    ];
    for (path, name, source_kind, route_kind) in cases {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new(path),
                path: PathBuf::from(path),
                source: "export default function Page() { return <main />; }".to_string(),
            })
            .expect("parse next pages route");
        let route = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.kind == NodeKind::Route && symbol.name == name)
            .unwrap_or_else(|| panic!("route {name} for {path}"));
        let metadata = route.visibility.as_deref().unwrap_or_default();
        assert_eq!(
            route_metadata_value(metadata, "source").as_deref(),
            Some(source_kind)
        );
        assert_eq!(
            route_metadata_value(metadata, "kind").as_deref(),
            Some(route_kind)
        );
    }
    let special = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("app"),
            path: PathBuf::from("pages/_app.tsx"),
            source: "export default function App() { return null; }".to_string(),
        })
        .expect("parse pages special");
    assert!(!special
        .symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route));
}
