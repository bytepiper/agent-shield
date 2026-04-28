use crate::bus::DecisionEnvelope;

use super::adapters::{AdapterAction, AdapterResult};
use super::service::EvalContext;

pub trait DecisionMerger: Send + Sync {
    fn merge(&self, ctx: &EvalContext, results: Vec<AdapterResult>) -> DecisionEnvelope;
}

#[derive(Clone, Default)]
pub struct DefaultDecisionMerger;

impl DecisionMerger for DefaultDecisionMerger {
    fn merge(&self, _ctx: &EvalContext, results: Vec<AdapterResult>) -> DecisionEnvelope {
        for result in results {
            match result.action {
                AdapterAction::Neutral => continue,
                AdapterAction::Block => {
                    return DecisionEnvelope {
                        action: "block".into(),
                        reason: result.reason,
                        status: result.status.or(Some(403)),
                        headers: result.headers,
                        text: result.text,
                        base64: result.base64,
                        route_target: result.route_target,
                        meta: result.meta,
                    };
                }
                AdapterAction::Modify => {
                    return DecisionEnvelope {
                        action: "modify".into(),
                        reason: result.reason,
                        status: result.status,
                        headers: result.headers,
                        text: result.text,
                        base64: result.base64,
                        route_target: result.route_target,
                        meta: result.meta,
                    };
                }
                AdapterAction::Replace => {
                    return DecisionEnvelope {
                        action: "replace".into(),
                        reason: result.reason,
                        status: result.status,
                        headers: result.headers,
                        text: result.text,
                        base64: result.base64,
                        route_target: result.route_target,
                        meta: result.meta,
                    };
                }
                AdapterAction::Route => {
                    return DecisionEnvelope {
                        action: "route".into(),
                        reason: result.reason,
                        status: result.status,
                        headers: result.headers,
                        text: result.text,
                        base64: result.base64,
                        route_target: result.route_target,
                        meta: result.meta,
                    };
                }
            }
        }

        DecisionEnvelope {
            action: "allow".into(),
            ..Default::default()
        }
    }
}
