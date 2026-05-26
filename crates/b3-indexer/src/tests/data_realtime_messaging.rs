use super::*;

#[test]
fn detects_signalr_project_technologies() {
    let detected = detect_csproj_realtime_technologies(
        r#"<Project>
            <ItemGroup>
                <PackageReference Include="Microsoft.AspNetCore.SignalR" Version="1" />
            </ItemGroup>
        </Project>"#,
    )
    .expect("detect signalr csproj");

    assert!(detected.iter().any(|tech| tech.id == "signalr"));
    assert!(detect_csproj_realtime_technologies("<Project><Broken").is_ok());
}

#[test]
fn detects_messaging_project_technologies() {
    let detected = detect_csproj_messaging_technologies(
        r#"<Project>
            <ItemGroup>
                <PackageReference Include="RabbitMQ.Client" Version="6" />
                <PackageReference Include="Confluent.Kafka" Version="2" />
                <PackageReference Include="Google.Cloud.PubSub.V1" Version="3" />
                <PackageReference Include="MassTransit" Version="8" />
            </ItemGroup>
        </Project>"#,
    )
    .expect("detect messaging csproj");

    assert!(detected.iter().any(|tech| tech.id == "rabbitmq"));
    assert!(detected.iter().any(|tech| tech.id == "kafka"));
    assert!(detected.iter().any(|tech| tech.id == "google_pubsub"));
    assert!(detected
        .iter()
        .any(|tech| tech.id == "masstransit"
            && tech.support_level == TechnologySupportLevel::DetectOnly));
    assert!(detect_csproj_messaging_technologies("<Project><Broken").is_ok());
}

#[test]
fn detects_ef_core_and_dapper_project_technologies() {
    let detected = detect_csproj_data_access_technologies(
        r#"<Project>
            <ItemGroup>
                <PackageReference Include="Microsoft.EntityFrameworkCore.Sqlite" Version="8" />
                <PackageReference Include="Dapper" Version="2" />
            </ItemGroup>
        </Project>"#,
    )
    .expect("detect data access csproj");

    assert!(detected.iter().any(|tech| tech.id == "ef_core"));
    assert!(detected.iter().any(|tech| tech.id == "dapper"));
    assert!(detect_csproj_data_access_technologies("<Project><Broken").is_ok());
}

#[test]
fn csharp_data_access_detects_ef_core_and_dapper_calls() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("data-csharp"),
            path: PathBuf::from("Repositories/UserRepository.cs"),
            source: r#"
                using Microsoft.EntityFrameworkCore;
                using Dapper;

                public class AppDbContext : DbContext
                {
                    public DbSet<User> Users { get; set; }
                }

                public class UserRepository
                {
                    public async Task<List<User>> List()
                    {
                        return await _context.Users.Where(u => u.Active).ToListAsync();
                    }

                    public async Task Add(User user)
                    {
                        _context.Users.Add(user);
                        await _context.SaveChangesAsync();
                    }

                    public async Task<User> Find(SqlConnection connection, int id)
                    {
                        return await connection.QueryFirstOrDefaultAsync<User>("SELECT * FROM Users WHERE Id = @id", new { id });
                    }

                    public Task<int> Rename(SqlConnection connection)
                    {
                        return connection.ExecuteAsync("UPDATE Users SET Name = @name");
                    }
                }
            "#
            .to_string(),
        })
        .expect("parse csharp data access");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            data_access_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "kind").as_deref() == Some("DbContext")
            && data_access_metadata_value(metadata, "context").as_deref() == Some("AppDbContext")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "kind").as_deref() == Some("DbSet")
            && data_access_metadata_value(metadata, "entity").as_deref() == Some("User")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "technology").as_deref() == Some("ef_core")
            && data_access_metadata_value(metadata, "operation").as_deref() == Some("read")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "technology").as_deref() == Some("dapper")
            && data_access_metadata_value(metadata, "query")
                .unwrap_or_default()
                .contains("SELECT * FROM Users")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "source").as_deref() == Some("DapperExecute")
    }));
}

#[test]
fn web_data_access_detects_prisma_typeorm_and_sequelize() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("data-web"),
            path: PathBuf::from("src/data.ts"),
            source: r#"
                import { PrismaClient } from "@prisma/client";
                import { Entity, Column } from "typeorm";
                import { Model } from "sequelize";

                const prisma = new PrismaClient();
                export async function loadUsers(repository, dataSource) {
                    await prisma.user.findMany();
                    await prisma.user.create({ data: {} });
                    await prisma.$queryRaw`SELECT * FROM users`;
                    await dataSource.getRepository(User).find();
                    await repository.save(user);
                    await repository.delete(id);
                    await User.findAll();
                    await User.create({});
                    await User.destroy({ where: { id } });
                }

                @Entity()
                export class User {
                    @Column()
                    name: string;
                }

                class AuditLog extends Model {}
                sequelize.define("Account", {});
            "#
            .to_string(),
        })
        .expect("parse web data access");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            data_access_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        data_access_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .as_deref()
            == Some("prisma")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "technology").as_deref() == Some("typeorm")
            && data_access_metadata_value(metadata, "kind").as_deref() == Some("Entity")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        data_access_metadata_value(metadata, "technology").as_deref() == Some("sequelize")
            && data_access_metadata_value(metadata, "operation").as_deref() == Some("delete")
    }));
}

#[test]
fn data_access_negative_cases_do_not_classify_plain_sql_words() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain"),
            path: PathBuf::from("src/plain.ts"),
            source: r#"
                export function render() {
                    const text = "SELECT users from a dropdown";
                    return text;
                }
            "#
            .to_string(),
        })
        .expect("parse plain");
    assert!(!parsed.symbols.iter().any(|symbol| {
        data_access_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .is_some()
    }));
}

#[test]
fn web_realtime_detects_websocket_socketio_signalr_and_rsocket() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("realtime-web"),
            path: PathBuf::from("src/realtime.ts"),
            source: r#"
                import WebSocket from "ws";
                import { Server } from "socket.io";
                import * as signalR from "@microsoft/signalr";
                import { RSocketClient } from "rsocket-core";

                const browserSocket = new WebSocket("ws://localhost:3000/ws");
                browserSocket.onmessage = (event) => console.log(event.data);
                browserSocket.addEventListener("message", handler);
                browserSocket.send("hello");

                const io = new Server();
                io.on("connection", socket => {
                    socket.on("join-room", handler);
                    socket.emit("room-joined", data);
                    io.emit("broadcast", data);
                });

                const connection = new signalR.HubConnectionBuilder()
                    .withUrl("/chatHub")
                    .build();
                connection.on("ReceiveMessage", handler);
                connection.invoke("SendMessage", "u", "m");

                client.requestResponse({ metadata: "chat.route" });
                client.fireAndForget(payload);
            "#
            .to_string(),
        })
        .expect("parse realtime web");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            realtime_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "technology").as_deref() == Some("websocket")
            && realtime_metadata_value(metadata, "endpoint").as_deref()
                == Some("ws://localhost:3000/ws")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "technology").as_deref() == Some("socketio")
            && realtime_metadata_value(metadata, "event").as_deref() == Some("join-room")
            && realtime_metadata_value(metadata, "kind").as_deref() == Some("Listener")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "technology").as_deref() == Some("signalr")
            && realtime_metadata_value(metadata, "method").as_deref() == Some("SendMessage")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "technology").as_deref() == Some("rsocket")
            && realtime_metadata_value(metadata, "source").as_deref()
                == Some("RSocketRequestResponse")
    }));
}

#[test]
fn csharp_realtime_detects_signalr_hubs_and_sends() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("signalr-csharp"),
            path: PathBuf::from("Hubs/ChatHub.cs"),
            source: r#"
                using Microsoft.AspNetCore.SignalR;

                public class ChatHub : Hub
                {
                    public async Task SendMessage(string user, string message)
                    {
                        await Clients.All.SendAsync("ReceiveMessage", user, message);
                    }
                }

                public class NotRealtime
                {
                    public void Run() { var message = "message"; }
                }
            "#
            .to_string(),
        })
        .expect("parse signalr csharp");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            realtime_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "kind").as_deref() == Some("Hub")
            && realtime_metadata_value(metadata, "hub").as_deref() == Some("ChatHub")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "kind").as_deref() == Some("HubMethod")
            && realtime_metadata_value(metadata, "method").as_deref() == Some("SendMessage")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        realtime_metadata_value(metadata, "source").as_deref() == Some("SignalRSendAsync")
            && realtime_metadata_value(metadata, "event").as_deref() == Some("ReceiveMessage")
    }));
}

#[test]
fn realtime_negative_cases_do_not_classify_plain_events() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain-events"),
            path: PathBuf::from("src/events.ts"),
            source: r#"
                export function render(emitter) {
                    const message = "message";
                    emitter.on("message", handler);
                    emitter.emit("message", message);
                    return message;
                }
            "#
            .to_string(),
        })
        .expect("parse plain events");
    assert!(!parsed.symbols.iter().any(|symbol| {
        realtime_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .is_some()
    }));
}

#[test]
fn web_messaging_detects_amqp_kafka_pubsub_and_nestjs() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("messaging-web"),
            path: PathBuf::from("src/messaging.ts"),
            source: r#"
                import amqp from "amqplib";
                import { Kafka } from "kafkajs";
                import { PubSub } from "@google-cloud/pubsub";
                import { MessagePattern, EventPattern, ClientProxy } from "@nestjs/microservices";

                export async function run(channel, producer, consumer, client: ClientProxy) {
                    channel.assertExchange("orders.exchange", "topic");
                    channel.assertQueue("orders.queue");
                    channel.bindQueue("orders.queue", "orders.exchange", "order.created");
                    channel.publish("orders.exchange", "order.created", Buffer.from("{}"));
                    channel.sendToQueue("orders.queue", Buffer.from("{}"));
                    channel.consume("orders.queue", handler);
                    await producer.send({ topic: "orders", messages: [] });
                    await consumer.subscribe({ topic: "orders" });
                    await consumer.run({ eachMessage: async () => {} });
                    const pubsub = new PubSub();
                    const topic = pubsub.topic("orders");
                    await topic.publishMessage({ json: {} });
                    const subscription = pubsub.subscription("orders-sub");
                    subscription.on("message", handler);
                    client.emit("order.created", {});
                    client.send("sum", {});
                }

                export class OrdersController {
                    @MessagePattern("order.created")
                    handleOrderCreated() {}

                    @EventPattern({ cmd: "sum" })
                    handleSum() {}
                }
            "#
            .to_string(),
        })
        .expect("parse web messaging");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            messaging_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("AmqpPublish")
            && messaging_metadata_value(metadata, "exchange").as_deref() == Some("orders.exchange")
            && messaging_metadata_value(metadata, "routing_key").as_deref() == Some("order.created")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("AmqpConsume")
            && messaging_metadata_value(metadata, "queue").as_deref() == Some("orders.queue")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("KafkaProducerSend")
            && messaging_metadata_value(metadata, "topic").as_deref() == Some("orders")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref()
            == Some("GooglePubSubSubscriptionHandler")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("NestMessagePattern")
            && messaging_metadata_value(metadata, "pattern").as_deref() == Some("order.created")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("NestEventPattern")
            && messaging_metadata_value(metadata, "pattern").as_deref() == Some("sum")
    }));
}

#[test]
fn csharp_messaging_detects_rabbitmq_kafka_and_pubsub() {
    let parsed = DefaultLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("messaging-csharp"),
            path: PathBuf::from("Messaging/Workers.cs"),
            source: r#"
                using RabbitMQ.Client;
                using Confluent.Kafka;
                using Google.Cloud.PubSub.V1;

                public class Workers
                {
                    public async Task Run(IModel channel, IProducer<string, string> producer, IConsumer<string, string> consumer)
                    {
                        channel.ExchangeDeclare(exchange: "orders.exchange", type: "topic");
                        channel.QueueDeclare(queue: "orders.queue");
                        channel.QueueBind(queue: "orders.queue", exchange: "orders.exchange", routingKey: "order.created");
                        channel.BasicPublish(exchange: "orders.exchange", routingKey: "order.created", body: body);
                        channel.BasicConsume(queue: "orders.queue", autoAck: true, consumer: handler);
                        await producer.ProduceAsync("orders", message);
                        consumer.Subscribe("orders");
                        consumer.Consume(token);
                        var publisher = await PublisherClient.CreateAsync("projects/demo/topics/orders");
                        await publisher.PublishAsync("payload");
                        var subscriber = await SubscriberClient.CreateAsync("projects/demo/subscriptions/orders-sub");
                        await subscriber.StartAsync(handler);
                    }
                }
            "#
            .to_string(),
        })
        .expect("parse csharp messaging");

    let records: Vec<&ExtractedSymbol> = parsed
        .symbols
        .iter()
        .filter(|symbol| {
            messaging_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "technology",
            )
            .is_some()
        })
        .collect();
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("RabbitMqPublish")
            && messaging_metadata_value(metadata, "routing_key").as_deref() == Some("order.created")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref() == Some("KafkaProduceAsync")
            && messaging_metadata_value(metadata, "topic").as_deref() == Some("orders")
    }));
    assert!(records.iter().any(|symbol| {
        let metadata = symbol.visibility.as_deref().unwrap_or_default();
        messaging_metadata_value(metadata, "source").as_deref()
            == Some("GooglePubSubSubscriberClient")
            && messaging_metadata_value(metadata, "queue")
                .unwrap_or_default()
                .contains("orders-sub")
    }));
}

#[test]
fn messaging_negative_cases_do_not_classify_plain_event_emitters() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("plain-messaging"),
            path: PathBuf::from("src/plain.ts"),
            source: r#"
                export function render(emitter) {
                    const topic = "orders";
                    const queue = "orders.queue";
                    emitter.on("message", handler);
                    emitter.emit("order.created", {});
                    return `${topic}:${queue}`;
                }
            "#
            .to_string(),
        })
        .expect("parse plain messaging");
    assert!(!parsed.symbols.iter().any(|symbol| {
        messaging_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "technology",
        )
        .is_some()
    }));
}
