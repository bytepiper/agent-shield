use anyhow::Result;
use tokio::time::timeout;
use tracing::warn;

use crate::bus::DecisionEnvelope;
use crate::events::{EventBody, HeaderKv, InterceptorEvent};

use super::adapters::AdapterRegistry;
use super::classifier::{Classifier, DefaultClassifier, TrafficClass};
use super::http_callbacks::{HandlerContext, RestCallbacks};
use super::merger::{DecisionMerger, DefaultDecisionMerger};
use super::planner::{DefaultPlanner, Planner};

#[derive(Clone)]
pub struct EvalContext {
    pub event: InterceptorEvent,
    pub traffic_class: TrafficClass,
}

impl EvalContext {
    pub fn request_body(&self) -> Option<&EventBody> {
        self.event.req_body.as_ref()
    }

    pub fn response_body(&self) -> Option<&EventBody> {
        self.event.resp_body.as_ref()
    }

    pub fn primary_body(&self) -> Option<&EventBody> {
        match self.event.phase.as_str() {
            "http.request" | "ws.message.out" => self.request_body(),
            "http.response" | "ws.message.in" | "sse.event.in" => self.response_body(),
            _ => self.request_body().or_else(|| self.response_body()),
        }
    }

    pub fn primary_text(&self) -> Option<&str> {
        self.primary_body().and_then(|body| body.text.as_deref())
    }

    pub fn primary_headers(&self) -> &[HeaderKv] {
        match self.event.phase.as_str() {
            "http.response" | "ws.message.in" | "sse.event.in" => &self.event.resp_headers,
            _ => &self.event.req_headers,
        }
    }

    pub fn to_handler_context(&self) -> HandlerContext {
        HandlerContext {
            schema_version: HandlerContext::schema_version(),
            event_id: self.event.id,
            session_id: self.event.session_id,
            event_seq: self.event.event_seq,
            session_seq: self.event.session_seq,
            phase: self.event.phase.clone(),
            transport: self.event.transport.clone(),
            direction: self.event.direction.clone(),
            method: self.event.method.clone(),
            url: self.event.url.clone(),
            domain: self.event.domain.clone(),
            status: self.event.status,
            action: self.event.action.clone(),
            content_type: self.event.content_type.clone(),
            traffic_class: self.traffic_class,
            req_headers: self.event.req_headers.clone(),
            resp_headers: self.event.resp_headers.clone(),
            req_body: self.event.req_body.clone(),
            resp_body: self.event.resp_body.clone(),
            primary_text: self.primary_text().map(ToOwned::to_owned),
            preview: self.event.preview.clone(),
        }
    }
}

pub struct OrchestratorService {
    classifier: Box<dyn Classifier>,
    planner: Box<dyn Planner>,
    registry: AdapterRegistry,
    merger: Box<dyn DecisionMerger>,
    rest_callbacks: Option<RestCallbacks>,
}

impl Default for OrchestratorService {
    fn default() -> Self {
        Self {
            classifier: Box::new(DefaultClassifier),
            planner: Box::new(DefaultPlanner),
            registry: AdapterRegistry::with_defaults(),
            merger: Box::new(DefaultDecisionMerger),
            rest_callbacks: RestCallbacks::from_env().ok().flatten(),
        }
    }
}

impl OrchestratorService {
    pub async fn decide(&self, event: InterceptorEvent) -> Result<DecisionEnvelope> {
        let traffic_class = self.classifier.classify(&event);
        let ctx = EvalContext {
            event,
            traffic_class,
        };
        let handler_context = ctx.to_handler_context();

        if let Some(callbacks) = &self.rest_callbacks {
            callbacks.notify_listeners(handler_context.clone());
        }

        let plan = self.planner.build(&ctx);
        let mut results = Vec::with_capacity(plan.steps.len());

        for step in plan.steps {
            let Some(adapter) = self.registry.get(&step.adapter) else {
                if step.required {
                    return Ok(DecisionEnvelope {
                        action: "block".into(),
                        reason: Some(format!("missing_adapter:{}", step.adapter)),
                        status: Some(500),
                        ..Default::default()
                    });
                }
                continue;
            };

            let result = match timeout(step.timeout, adapter.execute(&ctx)).await {
                Ok(Ok(result)) => result,
                Ok(Err(err)) => {
                    if step.required {
                        return Ok(DecisionEnvelope {
                            action: "block".into(),
                            reason: Some(format!("adapter_error:{}:{err}", step.adapter)),
                            status: Some(500),
                            ..Default::default()
                        });
                    }
                    continue;
                }
                Err(_) => {
                    if step.required {
                        return Ok(DecisionEnvelope {
                            action: "block".into(),
                            reason: Some(format!("adapter_timeout:{}", step.adapter)),
                            status: Some(504),
                            ..Default::default()
                        });
                    }
                    continue;
                }
            };

            results.push(result);
        }

        let decision = self.merger.merge(&ctx, results);
        if decision.action != "allow" {
            return Ok(decision);
        }

        if let Some(callbacks) = &self.rest_callbacks {
            match callbacks.call_handler(&handler_context).await {
                Ok(Some(decision)) => return Ok(decision),
                Ok(None) => {}
                Err(err) => warn!("handler callback failed, using allow fallback: {err}"),
            }
        }

        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::OrchestratorService;
    use crate::events::{EventBody, InterceptorEvent};

    #[tokio::test]
    async fn telemetry_request_is_blocked() {
        let service = OrchestratorService::default();
        let event = InterceptorEvent {
            phase: "http.request".into(),
            domain: "play.googleapis.com".into(),
            url: Some("/log?format=json&hasfast=true".into()),
            ..Default::default()
        };

        let decision = service.decide(event).await.expect("decision");
        assert_eq!(decision.action, "block");
        assert_eq!(decision.reason.as_deref(), Some("telemetry_blocked"));
        assert_eq!(decision.status, Some(403));
    }

    #[tokio::test]
    async fn anthropic_request_is_allowed_by_default() {
        let service = OrchestratorService::default();
        let event = InterceptorEvent {
            phase: "http.request".into(),
            domain: "api.anthropic.com".into(),
            url: Some("/v1/messages".into()),
            ..Default::default()
        };

        let decision = service.decide(event).await.expect("decision");
        assert_eq!(decision.action, "allow");
        assert!(decision.reason.is_none());
    }

    #[tokio::test]
    async fn request_with_secret_is_blocked() {
        let service = OrchestratorService::default();
        let fixture_key = format!("sk-{}", "1234567890abcdefghijklmnop");
        let event = InterceptorEvent {
            phase: "http.request".into(),
            domain: "api.anthropic.com".into(),
            url: Some("/v1/messages".into()),
            req_body: Some(EventBody {
                bytes: 29,
                truncated: false,
                text: Some(fixture_key),
                base64: None,
            }),
            ..Default::default()
        };

        let decision = service.decide(event).await.expect("decision");
        assert_eq!(decision.action, "block");
        assert_eq!(
            decision.reason.as_deref(),
            Some("secret_detected:openai_key")
        );
        assert_eq!(decision.status, Some(403));
    }
}
