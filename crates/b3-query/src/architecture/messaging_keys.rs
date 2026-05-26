use b3_core::normalize_message_key;

pub const UNKNOWN_BROKER: &str = "unknown";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessagingMatchKey {
    pub broker: String,
    pub channel_kind: String,
    pub name: String,
    pub normalized_key: String,
}

impl MessagingMatchKey {
    pub fn new(broker: Option<&str>, channel_kind: &str, name: &str) -> Self {
        let broker = normalize_broker_kind(broker);
        let channel_kind = normalize_channel_kind(channel_kind);
        let name = normalize_channel_name(name);
        let normalized_key = format!("messaging:{broker}:{channel_kind}:{name}");
        Self {
            broker,
            channel_kind,
            name,
            normalized_key,
        }
    }
}

pub fn normalize_broker_kind(value: Option<&str>) -> String {
    let value = value.unwrap_or_default().trim().to_ascii_lowercase();
    match value.as_str() {
        "" => UNKNOWN_BROKER.to_string(),
        "amqp" | "rabbit" | "rabbitmq.client" => "rabbitmq".to_string(),
        "google_pubsub" | "google-pubsub" | "google.cloud.pubsub.v1" | "pub/sub" => {
            "pubsub".to_string()
        }
        "clientproxy" | "nest" | "nestjs_microservice" => "nestjs".to_string(),
        other => other.replace([' ', '_'], "-"),
    }
}

pub fn normalize_channel_kind(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "topics" => "topic".to_string(),
        "queues" => "queue".to_string(),
        "routingkey" | "routing-key" => "routing_key".to_string(),
        "message_pattern" | "event_pattern" => "pattern".to_string(),
        "" => "unknown".to_string(),
        other => other.replace([' ', '-'], "_"),
    }
}

pub fn normalize_channel_name(value: &str) -> String {
    let trimmed = value
        .trim()
        .trim_matches(['"', '\'', '`'])
        .trim_matches('/');
    let collapsed_spaces = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    normalize_message_key("channel", &collapsed_spaces)
        .strip_prefix("messaging.channel:")
        .unwrap_or(&collapsed_spaces)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_messaging_keys() {
        assert_eq!(normalize_broker_kind(Some(" RabbitMQ.Client ")), "rabbitmq");
        assert_eq!(
            normalize_broker_kind(Some("Google.Cloud.PubSub.V1")),
            "pubsub"
        );
        assert_eq!(normalize_broker_kind(None), UNKNOWN_BROKER);
        assert_eq!(normalize_channel_kind("routing-key"), "routing_key");
        assert_eq!(
            normalize_channel_name(" '/Orders.Created/' "),
            "orders.created"
        );
        assert_eq!(
            MessagingMatchKey::new(Some("Kafka"), "topic", "Orders.Created").normalized_key,
            "messaging:kafka:topic:orders.created"
        );
    }
}
