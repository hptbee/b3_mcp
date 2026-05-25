use super::*;

pub(super) fn collect_web_pubsub(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !web_pubsub_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if has_import_or_require(trimmed, "@google-cloud/pubsub") {
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub import",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "google_pubsub",
                    "Publisher",
                    "unknown",
                    "GooglePubSubImport",
                ),
            ));
        }
        if has_import_or_require(trimmed, "pubsub-js") {
            output.push(messaging_symbol(
                input,
                line_number,
                "Generic Pub/Sub import",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "pubsub",
                    "Unknown",
                    "unknown",
                    "GenericPubSubImport",
                ),
            ));
        }
        if trimmed.contains("new PubSub(") {
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub client",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "google_pubsub",
                    "Publisher",
                    "bidirectional",
                    "GooglePubSubClient",
                ),
            ));
        }
        if trimmed.contains(".topic(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "google_pubsub",
                "Topic",
                "unknown",
                "GooglePubSubTopic",
            );
            metadata.topic = call_literal_argument(trimmed, ".topic(");
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub topic",
                metadata,
            ));
        }
        if trimmed.contains(".publishMessage(") || trimmed.contains(".publish(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "google_pubsub",
                "Publisher",
                "outbound",
                "GooglePubSubTopicPublish",
            );
            metadata.topic = call_literal_argument(trimmed, ".topic(")
                .or_else(|| object_literal_string(trimmed, "topic"));
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub publish",
                metadata,
            ));
        }
        if trimmed.contains(".subscription(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "google_pubsub",
                "Subscriber",
                "inbound",
                "GooglePubSubSubscription",
            );
            metadata.queue = call_literal_argument(trimmed, ".subscription(");
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub subscription",
                metadata,
            ));
        }
        if trimmed.contains(".on(\"message\"") || trimmed.contains(".on('message'") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "google_pubsub",
                "Subscriber",
                "inbound",
                "GooglePubSubSubscriptionHandler",
            );
            metadata.pattern = Some("message".to_string());
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub message handler",
                metadata,
            ));
        }
    }
    output
}

pub(super) fn collect_csharp_pubsub(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !csharp_pubsub_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed == "using Google.Cloud.PubSub.V1;" {
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub using",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "google_pubsub",
                    "Publisher",
                    "unknown",
                    "GooglePubSubUsing",
                ),
            ));
        }
        if trimmed.contains("PublisherClient.CreateAsync(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "google_pubsub",
                "Publisher",
                "outbound",
                "GooglePubSubPublisherClient",
            );
            metadata.topic = literal_string_argument(trimmed);
            metadata.confidence = 8_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub publisher",
                metadata,
            ));
        }
        if trimmed.contains(".PublishAsync(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "google_pubsub",
                "Publisher",
                "outbound",
                "GooglePubSubPublishAsync",
            );
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub publish",
                metadata,
            ));
        }
        if trimmed.contains("SubscriberClient.CreateAsync(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "google_pubsub",
                "Subscriber",
                "inbound",
                "GooglePubSubSubscriberClient",
            );
            metadata.queue = literal_string_argument(trimmed);
            metadata.confidence = 8_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub subscriber",
                metadata,
            ));
        }
        if trimmed.contains(".StartAsync(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "google_pubsub",
                "Subscriber",
                "inbound",
                "GooglePubSubSubscriberStart",
            );
            metadata.confidence = 8_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "Google Pub/Sub subscriber start",
                metadata,
            ));
        }
    }
    output
}

fn web_pubsub_context(source: &str) -> bool {
    source.contains("@google-cloud/pubsub")
        || source.contains("new PubSub(")
        || source.contains("pubsub-js")
}

fn csharp_pubsub_context(source: &str) -> bool {
    source.contains("Google.Cloud.PubSub.V1")
        || source.contains("PublisherClient")
        || source.contains("SubscriberClient")
}
