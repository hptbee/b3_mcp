use super::*;

#[test]
fn detects_angular_components_decorators_and_template_metadata() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("angular-component"),
            path: PathBuf::from("src/app/user-card.component.ts"),
            source: r#"
                    import { Component, Directive, Pipe } from "@angular/core";

                    @Component({
                        selector: "app-user-card",
                        templateUrl: "./user-card.component.html",
                        styleUrls: ["./user-card.component.scss"],
                        standalone: true,
                        imports: [CommonModule, UserBadgeComponent],
                        providers: [UserService]
                    })
                    export class UserCardComponent {}

                    @Component({
                        selector: "app-inline",
                        template: `<span>Inline</span>`,
                        styleUrl: "./inline.css"
                    })
                    export class InlineComponent {}

                    @Directive({ selector: "[appHighlight]" })
                    export class HighlightDirective {}

                    @Pipe({ name: "initials", standalone: true })
                    export class InitialsPipe {}
                "#
            .to_string(),
        })
        .expect("parse angular component");

    let user_card = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UserCardComponent")
        .expect("component symbol");
    let metadata = user_card.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        component_metadata_value(metadata, "framework").as_deref(),
        Some("angular")
    );
    assert_eq!(
        angular_metadata_value(metadata, "selector").as_deref(),
        Some("app-user-card")
    );
    assert_eq!(
        angular_metadata_value(metadata, "template_url").as_deref(),
        Some("./user-card.component.html")
    );
    assert!(angular_metadata_value(metadata, "style_urls")
        .as_deref()
        .unwrap_or_default()
        .contains("./user-card.component.scss"));
    assert_eq!(
        angular_metadata_value(metadata, "standalone").as_deref(),
        Some("true")
    );
    assert!(angular_metadata_value(metadata, "imports")
        .as_deref()
        .unwrap_or_default()
        .contains("UserBadgeComponent"));
    assert!(angular_metadata_value(metadata, "providers")
        .as_deref()
        .unwrap_or_default()
        .contains("UserService"));

    let inline = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "InlineComponent")
        .expect("inline component");
    let inline_metadata = inline.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        angular_metadata_value(inline_metadata, "inline_template_present").as_deref(),
        Some("true")
    );
    assert!(angular_metadata_value(inline_metadata, "style_urls")
        .as_deref()
        .unwrap_or_default()
        .contains("./inline.css"));

    let directive = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "HighlightDirective")
        .expect("directive");
    assert_eq!(
        angular_metadata_value(directive.visibility.as_deref().unwrap_or_default(), "kind")
            .as_deref(),
        Some("directive")
    );
    let pipe = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "InitialsPipe")
        .expect("pipe");
    assert_eq!(
        angular_metadata_value(pipe.visibility.as_deref().unwrap_or_default(), "pipe_name")
            .as_deref(),
        Some("initials")
    );
}

#[test]
fn detects_angular_services_modules_routes_and_di() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("angular-router"),
            path: PathBuf::from("src/app/app-routing.module.ts"),
            source: r#"
                    import { Injectable, NgModule, Component } from "@angular/core";
                    import { HttpClient } from "@angular/common/http";
                    import { RouterModule, Routes } from "@angular/router";

                    @Component({ selector: "app-home", template: "<p>Home</p>" })
                    export class HomeComponent {}
                    @Component({ selector: "app-user-detail", template: "<p>User</p>" })
                    export class UserDetailComponent {}
                    export class CacheService {}

                    @Injectable({ providedIn: "root" })
                    export class UserService {
                        constructor(private http: HttpClient, readonly cache: CacheService) {}
                    }

                    const routes: Routes = [
                        { path: "", component: HomeComponent },
                        { path: "users/:id", component: UserDetailComponent },
                        { path: "admin", loadChildren: () => import("./admin/admin.module").then(m => m.AdminModule) },
                        { path: "profile", loadComponent: () => import("./profile/profile.component").then(m => m.ProfileComponent) },
                        { path: "**", redirectTo: "" }
                    ];

                    @NgModule({
                        declarations: [HomeComponent, UserDetailComponent],
                        imports: [RouterModule.forRoot(routes)],
                        providers: [UserService],
                        exports: [RouterModule],
                        bootstrap: [HomeComponent]
                    })
                    export class AppRoutingModule {}
                "#
            .to_string(),
        })
        .expect("parse angular router");

    let service = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UserService")
        .expect("service symbol");
    let service_metadata = service.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        angular_metadata_value(service_metadata, "kind").as_deref(),
        Some("service")
    );
    assert_eq!(
        angular_metadata_value(service_metadata, "provided_in").as_deref(),
        Some("root")
    );
    assert!(angular_metadata_value(service_metadata, "dependencies")
        .as_deref()
        .unwrap_or_default()
        .contains("HttpClient"));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| { edge.from_symbol == service.id && edge.kind == EdgeKind::References }));

    let module = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "AppRoutingModule")
        .expect("module symbol");
    let module_metadata = module.visibility.as_deref().unwrap_or_default();
    assert!(angular_metadata_value(module_metadata, "declarations")
        .as_deref()
        .unwrap_or_default()
        .contains("HomeComponent"));
    assert!(angular_metadata_value(module_metadata, "imports")
        .as_deref()
        .unwrap_or_default()
        .contains("RouterModule.forRoot"));
    assert!(angular_metadata_value(module_metadata, "providers")
        .as_deref()
        .unwrap_or_default()
        .contains("UserService"));

    let routes = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            symbol.kind == NodeKind::Route
                && route_metadata_value(
                    symbol.visibility.as_deref().unwrap_or_default(),
                    "framework",
                )
                .as_deref()
                    == Some("angular")
        })
        .collect::<Vec<_>>();
    assert_eq!(routes.len(), 5);
    assert!(routes.iter().any(|route| {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        route_metadata_value(metadata, "path").as_deref() == Some("/users/:id")
            && route_metadata_value(metadata, "class").as_deref() == Some("UserDetailComponent")
    }));
    assert!(routes.iter().any(|route| {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        route_metadata_value(metadata, "path").as_deref() == Some("/admin")
            && route_metadata_value(metadata, "source").as_deref() == Some("AngularLazyRoute")
    }));
    assert!(routes.iter().any(|route| {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        route_metadata_value(metadata, "path").as_deref() == Some("/profile")
            && route_metadata_value(metadata, "source").as_deref()
                == Some("AngularLoadComponentRoute")
    }));
    assert!(routes.iter().any(|route| {
        let metadata = route.visibility.as_deref().unwrap_or_default();
        route_metadata_value(metadata, "path").as_deref() == Some("/**")
            && route_metadata_value(metadata, "source").as_deref() == Some("AngularRedirectRoute")
    }));
    assert!(parsed.relationships.iter().any(|edge| {
        edge.kind == EdgeKind::References
            && routes.iter().any(|route| edge.from_symbol == route.id)
            && parsed
                .symbols
                .iter()
                .any(|symbol| symbol.id == edge.to_symbol && symbol.name == "HomeComponent")
    }));
}

#[test]
fn does_not_misclassify_plain_typescript_as_angular() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain-ts"),
            path: PathBuf::from("src/plain.ts"),
            source: r#"
                    class Component {}
                    export class PlainService {
                        constructor(private value: string) {}
                    }
                    const routes = [{ path: "x", component: Component }];
                "#
            .to_string(),
        })
        .expect("parse plain ts");

    assert!(!parsed.symbols.iter().any(|symbol| {
        angular_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "framework",
        )
        .as_deref()
            == Some("angular")
    }));
    assert!(!parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && route_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "framework",
            )
            .as_deref()
                == Some("angular")
    }));
}
