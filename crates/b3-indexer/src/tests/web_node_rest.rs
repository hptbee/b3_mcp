use super::*;

#[test]
fn detects_express_routes_and_handler_edges() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("express"),
            path: PathBuf::from("src/server.js"),
            source: r#"
                    const express = require("express");
                    const app = express();
                    const router = express.Router();

                    function listUsers(req, res) {}
                    function createUser(req, res) {}

                    app.get("/users", listUsers);
                    app.post("/users", createUser);
                    router.route("/users/:id").get(listUsers).post(createUser);
                    app.use("/users", router);
                "#
            .to_string(),
        })
        .expect("parse express");

    let routes = parsed
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
        .collect::<Vec<_>>();
    assert!(routes.iter().any(|route| route.name == "GET /users"));
    assert!(routes.iter().any(|route| route.name == "POST /users"));
    assert!(routes.iter().any(|route| route.name == "GET /users/:id"));
    assert!(routes.iter().any(|route| route.name == "ALL /users"));
    assert!(routes.iter().any(|route| route
        .visibility
        .as_deref()
        .unwrap_or_default()
        .contains("route.framework=express")));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::References));
}

#[test]
fn detects_nestjs_controller_routes_with_composed_paths() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("nest"),
            path: PathBuf::from("src/users.controller.ts"),
            source: r#"
                    import { Controller, Get, Post } from "@nestjs/common";

                    @Controller("users")
                    export class UsersController {
                        @Get()
                        findAll() {}

                        @Get(":id")
                        findOne() {}

                        @Post()
                        create() {}
                    }
                "#
            .to_string(),
        })
        .expect("parse nest");

    let route_names = parsed
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
        .map(|symbol| symbol.name.as_str())
        .collect::<Vec<_>>();
    assert!(route_names.contains(&"GET /users"));
    assert!(route_names.contains(&"GET /users/:id"));
    assert!(route_names.contains(&"POST /users"));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && symbol
                .visibility
                .as_deref()
                .unwrap_or_default()
                .contains("route.framework=nestjs")
    }));
}

#[test]
fn detects_fastify_shorthand_and_route_object() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("fastify"),
            path: PathBuf::from("src/server.ts"),
            source: r#"
                    import fastify from "fastify";
                    const app = fastify();
                    function listUsers() {}
                    app.get("/users", listUsers);
                    fastify.route({
                        method: "POST",
                        url: "/users",
                        handler: listUsers
                    });
                "#
            .to_string(),
        })
        .expect("parse fastify");

    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route && symbol.name == "GET /users"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.kind == NodeKind::Route && symbol.name == "POST /users"));
}
