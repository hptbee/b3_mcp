use super::*;

pub(super) fn collect_nestjs_messaging(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !nestjs_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if has_import_or_require(trimmed, "@nestjs/microservices") {
            output.push(messaging_symbol(
                input,
                line_number,
                "NestJS messaging import",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "nestjs_messaging",
                    "Handler",
                    "inbound",
                    "NestMessagingImport",
                ),
            ));
        }
        if trimmed.contains("@MessagePattern(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "nestjs_messaging",
                "Handler",
                "inbound",
                "NestMessagePattern",
            );
            metadata.pattern = decorator_pattern(trimmed, "@MessagePattern(");
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "NestJS MessagePattern",
                metadata,
            ));
        }
        if trimmed.contains("@EventPattern(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "nestjs_messaging",
                "Handler",
                "inbound",
                "NestEventPattern",
            );
            metadata.pattern = decorator_pattern(trimmed, "@EventPattern(");
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "NestJS EventPattern",
                metadata,
            ));
        }
        if trimmed.contains(".emit(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "nestjs_messaging",
                "Producer",
                "outbound",
                "NestClientProxyEmit",
            );
            metadata.pattern = call_literal_argument(trimmed, ".emit(");
            metadata.confidence = 8_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "NestJS ClientProxy emit",
                metadata,
            ));
        }
        if trimmed.contains(".send(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "nestjs_messaging",
                "Producer",
                "outbound",
                "NestClientProxySend",
            );
            metadata.pattern = call_literal_argument(trimmed, ".send(");
            metadata.confidence = 8_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "NestJS ClientProxy send",
                metadata,
            ));
        }
    }
    output
}

fn nestjs_context(source: &str) -> bool {
    source.contains("@nestjs/microservices")
        || source.contains("@MessagePattern(")
        || source.contains("@EventPattern(")
}

fn decorator_pattern(line: &str, marker: &str) -> Option<String> {
    call_literal_argument(line, marker).or_else(|| object_literal_string(line, "cmd"))
}
