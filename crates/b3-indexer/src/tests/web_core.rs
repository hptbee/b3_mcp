use super::*;

#[test]
fn web_language_detection_maps_js_ts_jsx_tsx_and_csharp() {
    assert_eq!(
        language_from_path(Path::new("app.js")).as_deref(),
        Some("javascript")
    );
    assert_eq!(
        language_from_path(Path::new("app.mjs")).as_deref(),
        Some("javascript")
    );
    assert_eq!(
        language_from_path(Path::new("app.cjs")).as_deref(),
        Some("javascript")
    );
    assert_eq!(
        language_from_path(Path::new("app.ts")).as_deref(),
        Some("typescript")
    );
    assert_eq!(
        language_from_path(Path::new("app.jsx")).as_deref(),
        Some("jsx")
    );
    assert_eq!(
        language_from_path(Path::new("app.tsx")).as_deref(),
        Some("tsx")
    );
    assert_eq!(
        language_from_path(Path::new("Program.cs")).as_deref(),
        Some("csharp")
    );
    assert_eq!(
        language_from_path(Path::new("Api.csproj")).as_deref(),
        Some("csproj")
    );
    assert_eq!(
        language_from_path(Path::new("cmd/server/main.go")).as_deref(),
        Some("go")
    );
    assert_eq!(
        language_from_path(Path::new("go.mod")).as_deref(),
        Some("gomod")
    );
}

#[test]
fn web_language_pack_extracts_javascript_symbols_imports_and_commonjs_exports() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("file"),
            path: PathBuf::from("src/app.js"),
            source: r#"
                    import helper from "./helper";
                    const fs = require("fs");

                    export function run() {}

                    class Runner {
                        start() {}
                    }

                    const Widget = () => null;
                    module.exports = { run };
                "#
            .to_string(),
        })
        .expect("parse javascript");

    assert_eq!(parsed.language.as_deref(), Some("javascript"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "run" && symbol.kind == NodeKind::Function));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Runner" && symbol.kind == NodeKind::Class));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "start" && symbol.kind == NodeKind::Method));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Widget" && symbol.kind == NodeKind::Function));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "./helper" && symbol.kind == NodeKind::Package));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "fs" && symbol.kind == NodeKind::Package));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "module.exports" && symbol.kind == NodeKind::Variable));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::Imports));
}

#[test]
fn detects_package_json_node_rest_technologies() {
    let detected = detect_package_json_technologies(
        r#"{
                "dependencies": {
                    "express": "^4.18.0",
                    "@nestjs/common": "^10.0.0",
                    "fastify": "^4.0.0",
                    "next": "^14.0.0",
                    "@angular/core": "^17.0.0",
                    "@angular/router": "^17.0.0",
                    "react": "^18.0.0",
                    "react-dom": "^18.0.0",
                    "@prisma/client": "^5.0.0",
                    "typeorm": "^0.3.0",
                    "sequelize": "^6.0.0",
                    "ws": "^8.0.0",
                    "socket.io": "^4.0.0",
                    "@microsoft/signalr": "^8.0.0",
                    "rsocket-core": "^0.0.0",
                    "amqplib": "^0.10.0",
                    "kafkajs": "^2.0.0",
                    "@google-cloud/pubsub": "^4.0.0",
                    "@nestjs/microservices": "^10.0.0",
                    "pubsub-js": "^1.0.0"
                },
                "devDependencies": {
                    "typescript": "^5.0.0",
                    "@types/react": "^18.0.0"
                }
            }"#,
    )
    .expect("detect package technologies");
    assert!(detected.iter().any(|tech| tech.id == "express"));
    assert!(detected.iter().any(|tech| tech.id == "nestjs"));
    assert!(detected.iter().any(|tech| tech.id == "fastify"));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "react" && tech.kind == TechnologyKind::WebFrontend));
    let nextjs = detected
        .iter()
        .find(|tech| tech.id == "nextjs")
        .expect("nextjs detected");
    assert_eq!(nextjs.support_level, TechnologySupportLevel::Basic);
    assert!(nextjs
        .capabilities
        .contains(&TechnologyCapability::ExtractRoutes));
    let angular = detected
        .iter()
        .find(|tech| tech.id == "angular")
        .expect("angular detected");
    assert_eq!(angular.support_level, TechnologySupportLevel::Basic);
    assert!(angular
        .capabilities
        .contains(&TechnologyCapability::ExtractRoutes));
    assert!(angular
        .capabilities
        .contains(&TechnologyCapability::ExtractComponents));
    assert!(detected.iter().any(|tech| tech.id == "prisma"));
    assert!(detected.iter().any(|tech| tech.id == "typeorm"));
    assert!(detected.iter().any(|tech| tech.id == "sequelize"));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "websocket" && tech.kind == TechnologyKind::Realtime));
    assert!(detected.iter().any(|tech| tech.id == "socketio"));
    assert!(detected.iter().any(|tech| tech.id == "signalr"));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "rsocket" && tech.support_level == TechnologySupportLevel::Basic));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "amqp" && tech.kind == TechnologyKind::Messaging));
    assert!(detected.iter().any(|tech| tech.id == "kafka"));
    assert!(detected.iter().any(|tech| tech.id == "google_pubsub"));
    assert!(detected.iter().any(|tech| tech.id == "nestjs_messaging"));
    assert!(detected.iter().any(
        |tech| tech.id == "pubsub" && tech.support_level == TechnologySupportLevel::DetectOnly
    ));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "typescript" && tech.kind == TechnologyKind::Language));
    assert!(detect_package_json_technologies("{not-json").is_err());
    assert!(detect_nextjs_config_path(Path::new("next.config.js")).is_some());
    assert!(detect_nextjs_config_path(Path::new("vite.config.ts")).is_none());
    assert!(detect_angular_config_path(Path::new("angular.json")).is_some());
    assert!(detect_angular_config_path(Path::new("tsconfig.app.json")).is_some());
    assert!(detect_angular_config_path(Path::new("next.config.js")).is_none());
}
