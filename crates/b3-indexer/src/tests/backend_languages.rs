use super::*;

#[test]
fn backend_language_detection_maps_phase_14_files_and_projects() {
    for (path, expected) in [
        ("app.py", "python"),
        ("pyproject.toml", "python_project"),
        ("requirements.txt", "python_project"),
        ("src/main/java/App.java", "java"),
        ("pom.xml", "java_project"),
        ("build.gradle", "java_project"),
        ("src/main/kotlin/App.kt", "kotlin"),
        ("build.gradle.kts", "kotlin_project"),
        ("routes/api.php", "php"),
        ("composer.json", "php_project"),
        ("app/controllers/orders_controller.rb", "ruby"),
        ("Gemfile", "ruby_project"),
    ] {
        assert_eq!(
            language_from_path(Path::new(path)).as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn python_backend_language_pack_extracts_symbols_routes_data_and_messaging() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("python-fastapi"),
            path: PathBuf::from("app.py"),
            source: r#"
                from fastapi import FastAPI, APIRouter
                import pika

                app = FastAPI()
                router = APIRouter(prefix="/api")
                DEFAULT_QUEUE = "orders"

                class Order:
                    pass

                @router.get("/orders")
                async def list_orders():
                    session.query(Order)
                    channel.basic_publish(exchange="orders.exchange", routing_key="order.created", body=b"{}")

                @app.post("/users")
                def create_user():
                    User.objects.create(name="Ada")
            "#.to_string(),
        })
        .expect("parse python");

    assert_eq!(parsed.language.as_deref(), Some("python"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "list_orders"
            && matches!(symbol.kind, NodeKind::Function | NodeKind::Method)));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "fastapi" && symbol.kind == NodeKind::Package));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && symbol
                .visibility
                .as_deref()
                .and_then(|metadata| backend_route_metadata_value(metadata, "path"))
                .as_deref()
                == Some("/api/orders")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "data_access", "technology"))
            .as_deref()
            == Some("sqlalchemy")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "messaging", "routing_key"))
            .as_deref()
            == Some("order.created")
    }));
}

#[test]
fn java_backend_language_pack_extracts_spring_jaxrs_jpa_and_listeners() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("java-spring"),
            path: PathBuf::from("src/main/java/OrdersController.java"),
            source: r#"
                package com.acme.orders;
                import org.springframework.web.bind.annotation.GetMapping;
                import org.springframework.kafka.annotation.KafkaListener;

                @RestController
                @RequestMapping("/api")
                class OrdersController {
                    @GetMapping("/orders")
                    public List<Order> listOrders() { return jdbc.query("select * from orders"); }

                    @KafkaListener(topics = "orders")
                    public void consume(String body) {}
                }

                @Entity
                class Order {}

                interface OrderRepository extends JpaRepository<Order, Long> {}
            "#
            .to_string(),
        })
        .expect("parse java");

    assert_eq!(parsed.language.as_deref(), Some("java"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "OrdersController" && symbol.kind == NodeKind::Class));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && symbol
                .visibility
                .as_deref()
                .and_then(|metadata| backend_route_metadata_value(metadata, "path"))
                .as_deref()
                == Some("/api/orders")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "data_access", "kind"))
            .as_deref()
            == Some("Entity")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "messaging", "topic"))
            .as_deref()
            == Some("orders")
    }));
}

#[test]
fn kotlin_backend_language_pack_extracts_spring_ktor_and_messaging() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("kotlin-service"),
            path: PathBuf::from("src/main/kotlin/OrdersController.kt"),
            source: r#"
                package com.acme.orders
                import org.springframework.web.bind.annotation.PostMapping

                @RestController
                @RequestMapping("/api")
                class OrdersController {
                    @PostMapping("/orders")
                    fun createOrder() {}

                    @RabbitListener(queues = "orders.queue")
                    fun consume(body: String) {}
                }

                fun routes() {
                    route("/api") {
                        get("/health") {}
                    }
                }
            "#
            .to_string(),
        })
        .expect("parse kotlin");

    assert_eq!(parsed.language.as_deref(), Some("kotlin"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "OrdersController" && symbol.kind == NodeKind::Class));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && symbol
                .visibility
                .as_deref()
                .and_then(|metadata| backend_route_metadata_value(metadata, "framework"))
                .as_deref()
                == Some("ktor")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "messaging", "queue"))
            .as_deref()
            == Some("orders.queue")
    }));
}

#[test]
fn php_backend_language_pack_extracts_laravel_symfony_data_and_queue_hints() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("php-api"),
            path: PathBuf::from("routes/api.php"),
            source: r#"
                <?php
                namespace App\Http\Controllers;
                use Illuminate\Support\Facades\Route;

                Route::get('/orders', [OrderController::class, 'index']);

                class Order extends Model {}

                class OrderController {
                    #[Route('/users', methods: ['POST'])]
                    public function create() {
                        DB::select('select * from orders');
                        dispatch(new ShipOrder())->onQueue('orders');
                    }
                }
            "#
            .to_string(),
        })
        .expect("parse php");

    assert_eq!(parsed.language.as_deref(), Some("php"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "OrderController" && symbol.kind == NodeKind::Class));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && symbol
                .visibility
                .as_deref()
                .and_then(|metadata| backend_route_metadata_value(metadata, "framework"))
                .as_deref()
                == Some("laravel")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "data_access", "technology"))
            .as_deref()
            == Some("eloquent")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "messaging", "technology"))
            .as_deref()
            == Some("laravel_queue")
    }));
}

#[test]
fn ruby_backend_language_pack_extracts_rails_sinatra_active_record_and_jobs() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("ruby-routes"),
            path: PathBuf::from("config/routes.rb"),
            source: r#"
                require "sidekiq"

                Rails.application.routes.draw do
                  get "/orders", to: "orders#index"
                  resources :users
                end

                class Order < ApplicationRecord
                  has_many :items
                end

                class OrderWorker
                  include Sidekiq::Worker
                  def perform
                    Order.where(status: "new")
                  end
                end
            "#
            .to_string(),
        })
        .expect("parse ruby");

    assert_eq!(parsed.language.as_deref(), Some("ruby"));
    assert!(parsed
        .symbols
        .iter()
        .any(|symbol| symbol.name == "Order" && symbol.kind == NodeKind::Class));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.kind == NodeKind::Route
            && symbol
                .visibility
                .as_deref()
                .and_then(|metadata| backend_route_metadata_value(metadata, "framework"))
                .as_deref()
                == Some("rails")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "data_access", "technology"))
            .as_deref()
            == Some("active_record")
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol
            .visibility
            .as_deref()
            .and_then(|metadata| backend_metadata_value(metadata, "messaging", "technology"))
            .as_deref()
            == Some("sidekiq")
    }));
}
