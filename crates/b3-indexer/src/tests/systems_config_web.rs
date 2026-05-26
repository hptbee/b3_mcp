use super::*;

#[test]
fn phase15_static_parsers_extract_systems_mobile_config_web_and_data_hints() {
    let c = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("c-file"),
            path: PathBuf::from("src/native.c"),
            source:
                "#include <stdio.h>\nstruct Runner { int id; };\nint run_job(void) { return 1; }\n"
                    .to_string(),
        })
        .expect("parse c");
    assert_eq!(c.language.as_deref(), Some("c"));
    assert!(c
        .symbols
        .iter()
        .any(|s| s.name == "stdio.h" && s.kind == NodeKind::Package));
    assert!(c
        .symbols
        .iter()
        .any(|s| s.name == "Runner" && s.kind == NodeKind::Struct));
    assert!(c
        .symbols
        .iter()
        .any(|s| s.name == "run_job" && s.kind == NodeKind::Function));

    let swift = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("swift-file"),
            path: PathBuf::from("App.swift"),
            source: "import SwiftUI\n@main\nstruct DemoApp: App { var body: some Scene { WindowGroup { ContentView() } } }\nfunc load() { URLSession.shared.dataTask(with: URL(string: \"https://example.test/api\")!) }\n".to_string(),
        })
        .expect("parse swift");
    assert_eq!(swift.language.as_deref(), Some("swift"));
    assert!(swift
        .symbols
        .iter()
        .any(|s| s.name == "SwiftUI" && s.kind == NodeKind::Package));
    assert!(swift.symbols.iter().any(|s| s
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("swift.app_entry=true")));
    assert!(swift.symbols.iter().any(|s| s.kind == NodeKind::Route));

    let objc = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("objc-file"),
            path: PathBuf::from("ViewController.m"),
            source: "#import <UIKit/UIKit.h>\n@interface ViewController : UIViewController\n@property(nonatomic) NSString *title;\n@end\n@implementation ViewController\n- (void)viewDidLoad {}\n@end\n".to_string(),
        })
        .expect("parse objective-c");
    assert_eq!(objc.language.as_deref(), Some("objective_c"));
    assert!(objc.symbols.iter().any(|s| s.name == "UIKit/UIKit.h"));
    assert!(objc.symbols.iter().any(|s| s
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("objective_c.uiviewcontroller=true")));

    let dart = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("dart-file"),
            path: PathBuf::from("lib/main.dart"),
            source: "import 'package:flutter/material.dart';\nclass HomePage extends StatelessWidget { Widget build(context) => Container(); }\nfinal route = GoRoute(path: '/orders');\n".to_string(),
        })
        .expect("parse dart");
    assert_eq!(dart.language.as_deref(), Some("dart"));
    assert!(dart
        .symbols
        .iter()
        .any(|s| s.name == "package:flutter/material.dart"));
    assert!(dart.symbols.iter().any(|s| s
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("flutter.widget=true")));
    assert!(dart.symbols.iter().any(|s| s.kind == NodeKind::Route));

    let json = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("json-file"),
            path: PathBuf::from("package.json"),
            source: r#"{"name":"demo","dependencies":{"three":"1.0.0"},"password":"secret"}"#
                .to_string(),
        })
        .expect("parse json");
    assert_eq!(json.language.as_deref(), Some("json"));
    assert!(json
        .symbols
        .iter()
        .any(|s| s.name == "three" && s.kind == NodeKind::Package));
    assert!(json.symbols.iter().any(|s| s
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("config.value_redacted=true")));

    let html = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("html-file"),
            path: PathBuf::from("index.html"),
            source: r#"<html><head><title>Orders</title><link href="/app.css"></head><body><form action="/orders" method="post"><input id="order-id" class="field primary"></form><script src="/app.js"></script></body></html>"#.to_string(),
        })
        .expect("parse html");
    assert_eq!(html.language.as_deref(), Some("html"));
    assert!(html.symbols.iter().any(|s| s.name == "Orders"));
    assert!(html
        .symbols
        .iter()
        .any(|s| s.kind == NodeKind::Route && s.name == "/orders"));

    let css = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("css-file"),
            path: PathBuf::from("style.scss"),
            source: "$gap: 8px;\n.card, #main { --accent: red; background: url('hero.png'); }\n@mixin panel() {}\n@keyframes fade {}\n".to_string(),
        })
        .expect("parse scss");
    assert_eq!(css.language.as_deref(), Some("scss"));
    assert!(css.symbols.iter().any(|s| s.name == "card"));
    assert!(css.symbols.iter().any(|s| s.name == "hero.png"));

    let ksql = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("ksql-file"),
            path: PathBuf::from("orders.ksql"),
            source: "CREATE STREAM orders WITH (KAFKA_TOPIC='orders.events', VALUE_FORMAT='JSON');\nCREATE TABLE totals AS SELECT id FROM orders EMIT CHANGES;\n".to_string(),
        })
        .expect("parse ksql");
    assert_eq!(ksql.language.as_deref(), Some("ksql"));
    assert!(ksql.symbols.iter().any(|s| s
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("messaging.topic=orders.events")));
    assert!(ksql.symbols.iter().any(|s| s
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("ksqldb.depends_on=orders")));
}

#[test]
fn phase15_threejs_webgl_hints_are_static_js_metadata() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("three-file"),
            path: PathBuf::from("Scene.ts"),
            source: "import * as THREE from 'three';\nconst scene = new THREE.Scene();\nconst loader = new GLTFLoader();\nloader.load('/models/ship.glb');\nrequestAnimationFrame(tick);\n".to_string(),
        })
        .expect("parse ts");

    assert_eq!(parsed.language.as_deref(), Some("typescript"));
    assert!(parsed.symbols.iter().any(|s| s
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("webgl.hint=Scene")));
    assert!(parsed.symbols.iter().any(|s| s
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("webgl.asset=/models/ship.glb")));
}

#[test]
fn phase16_hardens_config_data_web_secret_resolution_and_sql_metadata() {
    let env = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("env-example"),
            path: PathBuf::from(".env.example"),
            source: "ORDER_TOPIC=orders.created\nPASSWORD=super-secret\nAPI_URL=/api/orders\n"
                .to_string(),
        })
        .expect("parse env");
    assert_eq!(env.language.as_deref(), Some("env"));
    assert!(env.symbols.iter().any(|s| {
        s.name == "ORDER_TOPIC"
            && s.visibility
                .as_deref()
                .unwrap_or_default()
                .contains("config.safe_value_hint=orders.created")
    }));
    assert!(!format!("{:?}", env.symbols).contains("super-secret"));
    assert!(env.symbols.iter().any(|s| {
        s.name == "PASSWORD"
            && s.visibility
                .as_deref()
                .unwrap_or_default()
                .contains("config.value_class=secret_like")
    }));

    let prod_env = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("env-prod"),
            path: PathBuf::from(".env.production"),
            source: "ORDER_TOPIC=orders.production\n".to_string(),
        })
        .expect("parse production env");
    assert!(!format!("{:?}", prod_env.symbols).contains("orders.production"));
    assert!(prod_env.symbols.iter().any(|s| {
        s.visibility
            .as_deref()
            .unwrap_or_default()
            .contains("config.env_file_safe=false")
    }));

    let yaml = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("yaml-secret"),
            path: PathBuf::from("secret.yaml"),
            source: "apiVersion: v1\nkind: Secret\ndata:\n  password: dont-store-me\nenv:\n  ORDER_TOPIC: ${ORDER_TOPIC}\n".to_string(),
        })
        .expect("parse yaml secret");
    assert!(!format!("{:?}", yaml.symbols).contains("dont-store-me"));
    assert!(yaml.symbols.iter().any(|s| {
        s.visibility
            .as_deref()
            .unwrap_or_default()
            .contains("config.reference=ORDER_TOPIC")
    }));

    let json = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("appsettings"),
            path: PathBuf::from("appsettings.json"),
            source: r#"{"ConnectionStrings":{"Default":"Server=db;User Id=sa;Password=nope"},"RabbitMq":{"RoutingKey":"orders.created"}}"#.to_string(),
        })
        .expect("parse appsettings");
    assert!(!format!("{:?}", json.symbols).contains("Password=nope"));
    assert!(json.symbols.iter().any(|s| {
        s.visibility
            .as_deref()
            .unwrap_or_default()
            .contains("config.reference_kind=messaging_config")
    }));

    let html = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("template"),
            path: PathBuf::from("Orders.cshtml"),
            source: r#"<form action="/orders" method="post"></form><script>fetch('/api/orders')</script><div data-component="OrderPanel"></div>"#.to_string(),
        })
        .expect("parse template");
    assert_eq!(html.language.as_deref(), Some("html"));
    assert!(html
        .symbols
        .iter()
        .any(|s| s.kind == NodeKind::Route && s.name == "/api/orders"));

    let css = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("styles"),
            path: PathBuf::from("styles.module.scss"),
            source: "@use 'tokens';\n@forward 'mixins';\n@media (min-width: 720px) { .card { background: url(\"hero.webp\"); } }\n".to_string(),
        })
        .expect("parse scss");
    assert!(css.symbols.iter().any(|s| s.name == "tokens"));
    assert!(css.symbols.iter().any(|s| {
        s.visibility
            .as_deref()
            .unwrap_or_default()
            .contains("css.media_query=")
    }));

    let sql = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("sql"),
            path: PathBuf::from("migrations/001_orders.sql"),
            source: "CREATE TABLE orders(id int);\nCREATE VIEW order_summary AS SELECT o.id FROM orders o JOIN customers c ON c.id=o.customer_id;\nINSERT INTO audit_log SELECT id FROM orders;\n".to_string(),
        })
        .expect("parse sql");
    assert_eq!(sql.language.as_deref(), Some("sql"));
    assert!(sql.symbols.iter().any(|s| s.name == "orders"
        && s.visibility
            .as_deref()
            .unwrap_or_default()
            .contains("data_access.operation=create")));
    assert!(sql.symbols.iter().any(|s| s.name == "customers"
        && s.visibility
            .as_deref()
            .unwrap_or_default()
            .contains("data_access.operation=join")));

    let ksql = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("ksql-hardening"),
            path: PathBuf::from("joined.ksql"),
            source: "CREATE STREAM enriched WITH (KAFKA_TOPIC='orders.enriched', KEY_FORMAT='JSON', VALUE_FORMAT='JSON') AS SELECT * FROM orders JOIN customers ON orders.customer_id = customers.id EMIT CHANGES;".to_string(),
        })
        .expect("parse ksql");
    assert!(ksql.symbols.iter().any(|s| {
        s.visibility
            .as_deref()
            .unwrap_or_default()
            .contains("ksqldb.relationship=join")
    }));
    assert!(ksql.symbols.iter().any(|s| {
        s.visibility
            .as_deref()
            .unwrap_or_default()
            .contains("ksqldb.format=JSON")
    }));
}

#[test]
fn phase17_audit_guards_comments_and_secret_metadata_across_static_files() {
    let sql = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("sql-comments"),
            path: PathBuf::from("plain.sql"),
            source: "-- CREATE TABLE commented_out(id int);\nCREATE TABLE orders(id int);\n-- SELECT * FROM not_real;\n".to_string(),
        })
        .expect("parse sql comments");
    assert_eq!(sql.language.as_deref(), Some("sql"));
    assert!(sql.symbols.iter().any(|s| s.name == "orders"));
    assert!(!sql.symbols.iter().any(|s| s.name == "commented_out"));
    assert!(!sql.symbols.iter().any(|s| s.name == "not_real"));

    let plain_sql_with_ksql_comment = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain-sql"),
            path: PathBuf::from("plain.sql"),
            source:
                "-- CREATE STREAM fake WITH (KAFKA_TOPIC='leak');\nCREATE TABLE orders(id int);\n"
                    .to_string(),
        })
        .expect("parse plain sql");
    assert_eq!(plain_sql_with_ksql_comment.language.as_deref(), Some("sql"));
    assert!(!format!("{:?}", plain_sql_with_ksql_comment.symbols).contains("leak"));

    let ksql = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("ksql-comments"),
            path: PathBuf::from("orders.ksql"),
            source: "-- CREATE STREAM fake WITH (KAFKA_TOPIC='do.not.store');\nCREATE STREAM orders WITH (KAFKA_TOPIC='orders.events', VALUE_FORMAT='JSON');\n".to_string(),
        })
        .expect("parse ksql comments");
    assert_eq!(ksql.language.as_deref(), Some("ksql"));
    let debug = format!("{:?}", ksql.symbols);
    assert!(debug.contains("orders.events"));
    assert!(!debug.contains("do.not.store"));

    let html = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("html-secret"),
            path: PathBuf::from("index.html"),
            source: r#"<a href="https://example.test/api/orders">remote</a><button data-auth="raw-secret-token"></button><script>fetch("https://example.test/api");fetch('/api/orders')</script>"#.to_string(),
        })
        .expect("parse html audit");
    assert_eq!(html.language.as_deref(), Some("html"));
    let html_debug = format!("{:?}", html.symbols);
    assert!(html_debug.contains("/api/orders"));
    assert!(!html
        .symbols
        .iter()
        .any(|s| { s.kind == NodeKind::Route && s.name.contains("https://example.test/api") }));
    assert!(!html_debug.contains("raw-secret-token"));
}
