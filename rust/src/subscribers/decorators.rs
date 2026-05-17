//! Decorators subscriber - collects decorator applications for edge generation.

use crate::events::Event;
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

pub struct DecoratorsSubscriber {
    name: String,
    decorators: Vec<serde_json::Value>,
}

impl DecoratorsSubscriber {
    pub fn new(name: &str) -> Self {
        DecoratorsSubscriber {
            name: name.to_string(),
            decorators: Vec::new(),
        }
    }
}

impl Subscriber for DecoratorsSubscriber {
    fn name(&self) -> &str {
        &self.name
    }

    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        if let Event::Decorator {
            node_id,
            name,
            root,
            is_attribute,
            is_call,
            ..
        } = event
        {
            if !node_id.is_empty() {
                self.decorators.push(serde_json::json!({
                    "decorated_fn": node_id,
                    "name": name,
                    "root": root,
                    "is_attribute": is_attribute,
                    "is_call": is_call,
                }));
            }
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: serde_json::json!({ "decorators": self.decorators }),
        })
    }
}

pub struct DecoratorsSubscriberFactory {
    name: String,
}

impl DecoratorsSubscriberFactory {
    pub fn new(name: &str) -> Self {
        DecoratorsSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for DecoratorsSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(DecoratorsSubscriber::new(&self.name))
    }
}
