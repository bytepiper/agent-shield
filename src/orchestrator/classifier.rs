use crate::events::InterceptorEvent;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficClass {
    ControlPlane,
    ModelHttp,
    ModelWs,
    Telemetry,
    #[default]
    Unknown,
}

pub trait Classifier: Send + Sync {
    fn classify(&self, event: &InterceptorEvent) -> TrafficClass;
}

#[derive(Clone, Default)]
pub struct DefaultClassifier;

impl Classifier for DefaultClassifier {
    fn classify(&self, event: &InterceptorEvent) -> TrafficClass {
        let host = event.domain.as_str();
        let path = event.url.as_deref().unwrap_or_default();
        let phase = event.phase.as_str();

        if host == "play.googleapis.com"
            || path.starts_with("/otlp/")
            || path.contains("/api/event_logging/")
            || path.starts_with("/log?")
        {
            return TrafficClass::Telemetry;
        }

        if host == "oauth2.googleapis.com"
            || host == "accounts.google.com"
            || path == "/token"
            || path.contains("/oauth2/")
        {
            return TrafficClass::ControlPlane;
        }

        if phase.starts_with("ws.") || path.contains("/backend-api/codex/responses") {
            return TrafficClass::ModelWs;
        }

        if host == "api.anthropic.com"
            || host == "api.openai.com"
            || host == "chatgpt.com"
            || host == "ab.chatgpt.com"
            || host == "cloudcode-pa.googleapis.com"
            || phase == "sse.event.in"
        {
            return TrafficClass::ModelHttp;
        }

        TrafficClass::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::{Classifier, DefaultClassifier, TrafficClass};
    use crate::events::InterceptorEvent;

    #[test]
    fn classifies_google_log_as_telemetry() {
        let event = InterceptorEvent {
            phase: "http.request".into(),
            domain: "play.googleapis.com".into(),
            url: Some("/log?format=json&hasfast=true".into()),
            ..Default::default()
        };

        assert_eq!(DefaultClassifier.classify(&event), TrafficClass::Telemetry);
    }

    #[test]
    fn classifies_codex_ws_as_model_ws() {
        let event = InterceptorEvent {
            phase: "ws.message.in".into(),
            domain: "chatgpt.com".into(),
            url: Some("/backend-api/codex/responses".into()),
            ..Default::default()
        };

        assert_eq!(DefaultClassifier.classify(&event), TrafficClass::ModelWs);
    }
}
