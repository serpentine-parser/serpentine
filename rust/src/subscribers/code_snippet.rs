//! Code snippet subscriber - captures source lines and scope ranges.
//!
//! This subscriber listens to `SourceLine` events to collect the raw source text
//! of each file, and `EnterScope`/`ExitScope`/`DefineName` events to track the
//! line range of each definition. The graph builder uses this data to populate
//! the `code_block` field on each node.

use std::collections::HashMap;

use crate::events::Event;
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

/// Line range for a definition (file, start_line, end_line — all 1-indexed).
#[derive(Debug, Clone)]
struct ScopeRange {
    file: String,
    start_line: usize,
    end_line: usize,
}

/// Subscriber that captures source lines and maps definitions to their line ranges.
pub struct CodeSnippetSubscriber {
    name: String,
    /// Source lines per file (0-indexed vec, lines are 1-indexed)
    source_lines: HashMap<String, Vec<String>>,
    /// Line range per qualname
    scope_ranges: HashMap<String, ScopeRange>,
    /// Stack of open scopes: (qualname, file, start_line)
    scope_stack: Vec<(String, String, usize)>,
    /// Docstrings per qualname (from EnterScope.docstring)
    docstrings: HashMap<String, String>,
}

impl CodeSnippetSubscriber {
    pub fn new(name: &str) -> Self {
        CodeSnippetSubscriber {
            name: name.to_string(),
            source_lines: HashMap::new(),
            scope_ranges: HashMap::new(),
            scope_stack: Vec::new(),
            docstrings: HashMap::new(),
        }
    }
}

impl Subscriber for CodeSnippetSubscriber {
    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        match event {
            Event::SourceLine { file, text, .. } => {
                self.source_lines
                    .entry(file.clone())
                    .or_default()
                    .push(text.clone());
            }

            Event::EnterScope {
                qualname,
                file,
                line,
                docstring,
                ..
            } => {
                self.scope_stack
                    .push((qualname.clone(), file.clone(), *line));
                self.scope_ranges.insert(
                    qualname.clone(),
                    ScopeRange {
                        file: file.clone(),
                        start_line: *line,
                        end_line: *line, // will be updated on ExitScope
                    },
                );
                if let Some(ds) = docstring {
                    self.docstrings.insert(qualname.clone(), ds.clone());
                }
            }

            Event::ExitScope { qualname, line, .. } => {
                // Update the end_line for this scope
                if let Some(range) = self.scope_ranges.get_mut(qualname) {
                    range.end_line = *line;
                }
                self.scope_stack.pop();
            }

            Event::DefineName {
                qualname,
                node_type,
                file,
                line,
                end_line,
                ..
            } => {
                // Functions and classes are handled by EnterScope/ExitScope
                if node_type != "function" && node_type != "class" {
                    self.scope_ranges.insert(
                        qualname.clone(),
                        ScopeRange {
                            file: file.clone(),
                            start_line: *line,
                            end_line: *end_line,
                        },
                    );
                }
            }

            _ => {}
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        // Build source_lines JSON: { "file_path": ["line1", "line2", ...] }
        let source_lines_json: serde_json::Map<String, serde_json::Value> = self
            .source_lines
            .iter()
            .map(|(file, lines)| {
                let lines_json: serde_json::Value = lines
                    .iter()
                    .map(|l| serde_json::Value::String(l.clone()))
                    .collect::<Vec<_>>()
                    .into();
                (file.clone(), lines_json)
            })
            .collect();

        // Build scope_ranges JSON: { "qualname": { "file": "...", "start_line": N, "end_line": M } }
        let scope_ranges_json: serde_json::Map<String, serde_json::Value> = self
            .scope_ranges
            .iter()
            .map(|(qualname, range)| {
                (
                    qualname.clone(),
                    serde_json::json!({
                        "file": range.file,
                        "start_line": range.start_line,
                        "end_line": range.end_line,
                    }),
                )
            })
            .collect();

        // Build docstrings JSON: { "qualname": "docstring text" }
        let docstrings_json: serde_json::Map<String, serde_json::Value> = self
            .docstrings
            .iter()
            .map(|(qualname, text)| (qualname.clone(), serde_json::Value::String(text.clone())))
            .collect();

        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: serde_json::json!({
                "source_lines": source_lines_json,
                "scope_ranges": scope_ranges_json,
                "docstrings": docstrings_json,
            }),
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Factory for creating `CodeSnippetSubscriber` instances.
pub struct CodeSnippetSubscriberFactory {
    name: String,
}

impl CodeSnippetSubscriberFactory {
    pub fn new(name: &str) -> Self {
        CodeSnippetSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for CodeSnippetSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(CodeSnippetSubscriber::new(&self.name))
    }
}
