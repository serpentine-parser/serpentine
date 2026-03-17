//! Subscribers that process events from the message bus.
//!
//! Each subscriber implements the `Subscriber` trait and processes events
//! to produce some output (e.g., scope trees, event counts, etc.).

mod pdg;
mod code_snippet;
mod definitions;
mod event_counter;
mod imports;
mod raw_bindings;
mod scope_tree;
mod uses;

pub use pdg::PdgSubscriberFactory;
pub use code_snippet::CodeSnippetSubscriberFactory;
pub use definitions::DefinitionsSubscriberFactory;
pub use event_counter::EventCounterSubscriberFactory;
pub use imports::ImportsSubscriberFactory;
pub use raw_bindings::RawBindingsSubscriberFactory;
pub use scope_tree::ScopeTreeSubscriberFactory;
pub use uses::UsesSubscriberFactory;
