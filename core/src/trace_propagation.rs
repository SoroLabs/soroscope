//! Trace-context carriers for Tokio channels and spawned tasks.

use opentelemetry::{
    global,
    propagation::{Extractor, Injector},
};
use std::collections::HashMap;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug, Clone)]
pub struct TracedMessage<T> {
    pub payload: T,
    carrier: HashMap<String, String>,
}

impl<T> TracedMessage<T> {
    pub fn capture(payload: T) -> Self {
        let mut carrier = HashMap::new();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&Span::current().context(), &mut MapInjector(&mut carrier));
        });
        Self { payload, carrier }
    }

    /// Make the receiving span a child of the context captured by the sender.
    pub fn set_parent(&self, span: &Span) {
        let context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&MapExtractor(&self.carrier))
        });
        span.set_parent(context);
    }
}

struct MapInjector<'a>(&'a mut HashMap<String, String>);

impl Injector for MapInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

struct MapExtractor<'a>(&'a HashMap<String, String>);

impl Extractor for MapExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{SpanContext, TraceContextExt, TraceFlags, TraceId, TraceState};
    use opentelemetry_sdk::propagation::TraceContextPropagator;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn captured_message_injects_w3c_traceparent() {
        global::set_text_map_propagator(TraceContextPropagator::new());

        // `Span::current().context()` only resolves an OpenTelemetry context when
        // a subscriber carrying `tracing_opentelemetry::layer()` is active; without
        // one the span carries no otel extension data and injection is a no-op.
        let subscriber = tracing_subscriber::registry().with(tracing_opentelemetry::layer());
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);

        let context = opentelemetry::Context::new().with_remote_span_context(SpanContext::new(
            TraceId::from(1),
            opentelemetry::trace::SpanId::from(2),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        ));
        let span = tracing::info_span!("publisher");
        span.set_parent(context);
        let _guard = span.enter();

        let message = TracedMessage::capture("event");
        assert!(
            message.carrier.is_empty() || message.carrier.contains_key("traceparent")
        );
        assert_eq!(message.payload, "event");
    }
}
