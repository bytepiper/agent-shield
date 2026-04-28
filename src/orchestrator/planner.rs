use std::time::Duration;

use super::classifier::TrafficClass;
use super::service::EvalContext;

#[derive(Clone, Debug)]
pub struct PlanStep {
    pub adapter: String,
    pub timeout: Duration,
    pub required: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ExecutionPlan {
    pub steps: Vec<PlanStep>,
}

pub trait Planner: Send + Sync {
    fn build(&self, ctx: &EvalContext) -> ExecutionPlan;
}

#[derive(Clone, Default)]
pub struct DefaultPlanner;

impl Planner for DefaultPlanner {
    fn build(&self, ctx: &EvalContext) -> ExecutionPlan {
        let mut steps = Vec::new();

        if matches!(
            ctx.event.phase.as_str(),
            "http.request" | "ws.message.out" | "http.response" | "ws.message.in" | "sse.event.in"
        ) {
            steps.push(PlanStep {
                adapter: "secret_scanner".into(),
                timeout: Duration::from_millis(50),
                required: true,
            });
        }

        if ctx.traffic_class == TrafficClass::Telemetry {
            steps.push(PlanStep {
                adapter: "telemetry_blocker".into(),
                timeout: Duration::from_millis(50),
                required: true,
            });
        }

        ExecutionPlan { steps }
    }
}
