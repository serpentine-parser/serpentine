//! Event counter subscriber - counts events by type.
//!
//! This is a simple subscriber useful for debugging and testing.
//! It counts how many of each event type are emitted.

use std::collections::HashMap;

use crate::events::Event;
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

/// Subscriber that counts events by type.
pub struct EventCounterSubscriber {
    name: String,
    counts: HashMap<String, usize>,
}

impl EventCounterSubscriber {
    pub fn new(name: &str) -> Self {
        EventCounterSubscriber {
            name: name.to_string(),
            counts: HashMap::new(),
        }
    }
}

impl Subscriber for EventCounterSubscriber {
    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        let event_type = match event {
            Event::DefineName { .. } => "define_name",
            Event::EnterScope { .. } => "enter_scope",
            Event::ExitScope { .. } => "exit_scope",
            Event::UseName { .. } => "use_name",
            Event::CallExpression { .. } => "call_expression",
            Event::ImportStatement { .. } => "import_statement",
            Event::ControlBlock { .. } => "control_block",
            Event::EndControlBlock { .. } => "end_control_block",
            Event::Return { .. } => "return",
            Event::AttributeAccess { .. } => "attribute_access",
            Event::Literal { .. } => "literal",
            Event::Assignment { .. } => "assignment",
            Event::BreakStatement { .. } => "break_statement",
            Event::ContinueStatement { .. } => "continue_statement",
            Event::RaiseStatement { .. } => "raise_statement",
            Event::ElseBlock { .. } => "else_block",
            Event::Decorator { .. } => "decorator",
            Event::YieldExpression { .. } => "yield_expression",
            Event::SourceLine { .. } => "source_line",
        };

        *self.counts.entry(event_type.to_string()).or_insert(0) += 1;
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        let json_data = serde_json::json!({
            "event_counts": self.counts,
            "total": self.counts.values().sum::<usize>(),
        });

        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: json_data,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Factory for creating `EventCounterSubscriber` instances.
pub struct EventCounterSubscriberFactory {
    name: String,
}

impl EventCounterSubscriberFactory {
    pub fn new(name: &str) -> Self {
        EventCounterSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for EventCounterSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(EventCounterSubscriber::new(&self.name))
    }
}
