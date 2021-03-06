use opentelemetry::api::{self, HttpTextFormat, Provider};
use opentelemetry::sdk;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::prelude::*;

struct TonicMetadataMapCarrier<'a>(&'a tonic::metadata::MetadataMap);
impl<'a> api::Carrier for TonicMetadataMapCarrier<'a> {
    fn get(&self, key: &'static str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn set(&mut self, _key: &'static str, _value: String) {
        unimplemented!()
    }
}

struct TonicMetadataMapCarrierMut<'a>(&'a mut tonic::metadata::MetadataMap);
impl<'a> api::Carrier for TonicMetadataMapCarrierMut<'a> {
    fn get(&self, key: &'static str) -> Option<&str> {
        self.0.get(key).and_then(|metadata| metadata.to_str().ok())
    }

    fn set(&mut self, key: &'static str, value: String) {
        if let Ok(key) = tonic::metadata::MetadataKey::from_bytes(key.to_lowercase().as_bytes()) {
            self.0.insert(
                key,
                tonic::metadata::MetadataValue::from_str(&value).unwrap(),
            );
        }
    }
}

pub struct TracedRequest {}

impl TracedRequest {
    pub fn new<T>(msg: T) -> tonic::Request<T> {
        let mut req = tonic::Request::new(msg);
        inject_trace(req.metadata_mut(), &tracing::Span::current());
        req
    }
}

pub fn trace_response<T: std::fmt::Debug>(
    response: Result<tonic::Response<T>, tonic::Status>,
) -> Result<T, tonic::Status> {
    match response {
        Ok(msg) => {
            tracing::info!("{:?}", msg);
            Ok(msg.into_inner())
        }
        Err(e) => {
            tracing::error!("Error received: {:?}", e);
            Err(e)
        }
    }
}

pub fn inject_trace(headers: &mut tonic::metadata::MetadataMap, span: &tracing::Span) {
    let propagator = api::TraceContextPropagator::new();
    propagator.inject_context(&span.context(), &mut TonicMetadataMapCarrierMut(headers));
}

pub fn propagate_trace(metadata: &tonic::metadata::MetadataMap) {
    let propagator = api::TraceContextPropagator::new();
    let span = tracing::Span::current();
    let parent_cx = propagator.extract(&TonicMetadataMapCarrier(metadata));
    span.set_parent(&parent_cx);
}

pub fn init_tracer(
    jaeger_url: &str,
    service_name: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let exporter = opentelemetry_jaeger::Exporter::builder()
        .with_agent_endpoint(jaeger_url.parse()?)
        .with_process(opentelemetry_jaeger::Process {
            service_name: String::from(service_name),
            tags: Vec::new(),
        })
        .init()?;
    let provider = sdk::Provider::builder()
        .with_simple_exporter(exporter)
        .with_config(sdk::Config {
            default_sampler: Box::new(sdk::Sampler::Always),
            ..Default::default()
        })
        .build();
    let tracer = provider.get_tracer(service_name);

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let filter = tracing_subscriber::EnvFilter::from_default_env();
    tracing_subscriber::registry()
        .with(opentelemetry)
        .with(filter)
        .try_init()?;

    Ok(())
}
