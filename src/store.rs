use crate::events::InterceptorEvent;
use std::sync::Mutex;

#[derive(Default)]
pub struct EventStore {
    events: Mutex<Vec<InterceptorEvent>>,
}

impl EventStore {
    pub fn push(&self, event: InterceptorEvent) {
        let mut events = self.events.lock().unwrap();
        events.push(event);
        let len = events.len();
        if len > 1000 {
            events.drain(..len - 1000);
        }
    }

    pub fn snapshot(&self) -> Vec<InterceptorEvent> {
        self.events.lock().unwrap().clone()
    }
}
