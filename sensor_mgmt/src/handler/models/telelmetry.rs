use opentelemetry::{
    global,
    propagation::{Extractor, Injector},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use ulid::Ulid;

/// Serializable datastructure to hold the opentelemetry propagation context.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PropagationContext(HashMap<String, String>);

impl PropagationContext {
    fn empty() -> Self {
        Self(HashMap::new())
    }

    pub fn inject(context: &opentelemetry::Context) -> Self {
        global::get_text_map_propagator(|propagator| {
            let mut propagation_context = PropagationContext::empty();
            propagator.inject_context(context, &mut propagation_context);
            propagation_context
        })
    }

    pub fn extract(&self) -> opentelemetry::Context {
        global::get_text_map_propagator(|propagator| propagator.extract(self))
    }
}

impl Injector for PropagationContext {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }
}

impl Extractor for PropagationContext {
    fn get(&self, key: &str) -> Option<&str> {
        let key = key.to_owned();
        self.0.get(&key).map(|v| v.as_ref())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_ref()).collect()
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OTelData {
    // As long as the span stuff doesnt work reliably we use this
    // pub correlation_id: Ulid, NOTE its part of the context now
    pub context: PropagationContext,
}
impl OTelData {
    pub fn generate() -> OTelData {
        let otel_ctx = Span::current().context();

        Span::current().record("correlation_id", format!("{}", Ulid::new()));

        OTelData {
            context: PropagationContext::inject(&otel_ctx),
        }
    }
}
