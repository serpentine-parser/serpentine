use crate::events::Event;

/// Trait for subscribers that process events.
///
/// Subscribers receive events one at a time via `handle_event`, then
/// produce a final result via `finalize` when all events have been processed.
pub trait Subscriber: Send {
    /// Handle a single event. Returns an error if processing fails.
    fn handle_event(&mut self, event: &Event) -> Result<(), String>;

    /// Called when all events have been published. Returns the final result.
    fn finalize(&mut self) -> Result<SubscriberResult, String>;

    /// Get the name of this subscriber (for debugging/logging).
    #[allow(dead_code)]
    fn name(&self) -> &str;
}

/// Factory trait for creating fresh subscriber instances.
///
/// Factories are registered with the message bus and used to create new
/// subscriber instances for each `publish_events` call.
pub trait SubscriberFactory: Send + Sync {
    fn create(&self) -> Box<dyn Subscriber>;
}

/// Result from a subscriber after processing all events.
#[derive(Debug, Clone)]
pub struct SubscriberResult {
    pub subscriber_name: String,
    pub data: serde_json::Value,
}

/// Message bus that fans out events to multiple subscribers sequentially.
pub struct MessageBus {
    factories: Vec<Box<dyn SubscriberFactory>>,
}

impl MessageBus {
    pub fn new() -> Self {
        MessageBus {
            factories: Vec::new(),
        }
    }

    /// Register a subscriber factory.
    pub fn register<F: SubscriberFactory + 'static>(&mut self, factory: F) {
        self.factories.push(Box::new(factory));
    }

    /// Publish events to all subscribers sequentially.
    ///
    /// Creates fresh subscriber instances from factories, processes all events
    /// through each subscriber in order, and collects results.
    /// Parallelism is handled at the file level via `open_files_bulk`.
    pub fn publish_events(&mut self, events: Vec<Event>) -> Result<Vec<SubscriberResult>, String> {
        if self.factories.is_empty() {
            return Ok(Vec::new());
        }

        let mut subscribers: Vec<_> = self.factories.iter().map(|f| f.create()).collect();

        for event in &events {
            for subscriber in &mut subscribers {
                subscriber.handle_event(event)?;
            }
        }

        subscribers.into_iter().map(|mut s| s.finalize()).collect()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}
