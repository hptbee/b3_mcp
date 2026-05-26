use super::*;

pub(crate) async fn events(
    State(state): State<ControlState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|event| match event {
        Ok(event) => Some(Ok(Event::default()
            .event(event.event_type)
            .data(event.payload.to_string()))),
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
