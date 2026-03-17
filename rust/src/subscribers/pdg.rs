//! PDG (Program Dependence Graph) subscriber — pending-exits CFG builder.
//!
//! Builds a proper control-flow graph per function by maintaining a control
//! frame stack. Events arrive in DFS order (mirroring AST nesting), so we can
//! derive the CFG structure purely from event sequencing.

use std::collections::HashMap;

use crate::events::{Event, ScopeType};
use crate::message_bus::{Subscriber, SubscriberFactory, SubscriberResult};

// ---------------------------------------------------------------------------
// Core data structures
// ---------------------------------------------------------------------------

/// A pending "half-edge": the source node and the edge type it will connect
/// to the *next* node that gets added.
#[derive(Debug, Clone)]
struct PendingExit {
    from: String,
    edge_type: String, // "flow" | "true_branch" | "false_branch" | "back_edge"
}

#[derive(Debug, Clone)]
struct PdgNode {
    id: String,
    node_type: String,
    text: String,
    line: usize,
    defines: Option<String>,
    uses: Vec<String>,
    callee_text: Option<String>,
    /// ID of the branch block this node belongs to (None = outer scope).
    branch_block_id: Option<String>,
}

#[derive(Debug, Clone)]
struct PdgEdge {
    from: String,
    to: String,
    edge_type: String,
}

#[derive(Debug)]
enum ControlFrame {
    If {
        condition_id: String,
        true_block_id: String,
        true_exits: Vec<PendingExit>,
    },
    Elif {
        condition_id: String,
        true_block_id: String,
        true_exits: Vec<PendingExit>,
        prior_false_exits: Vec<PendingExit>,
    },
    Else {
        false_block_id: String,
        exits_from_true_branches: Vec<PendingExit>,
    },
    While {
        condition_id: String,
        body_block_id: String,
        break_exits: Vec<PendingExit>,
    },
    For {
        condition_id: String,
        body_block_id: String,
        break_exits: Vec<PendingExit>,
    },
    Try {
        _try_exits: Vec<PendingExit>,
    },
    With,
    Match {
        subject_id: String,
        all_case_exits: Vec<PendingExit>,
    },
}

#[derive(Debug)]
struct FunctionPdg {
    qualname: String,
    node_counter: usize,
    nodes: Vec<PdgNode>,
    edges: Vec<PdgEdge>,
    pending_exits: Vec<PendingExit>,
    control_stack: Vec<ControlFrame>,
    /// UseName events accumulated since the last statement node.
    pending_uses: Vec<String>,
}

impl FunctionPdg {
    fn new(qualname: &str) -> Self {
        FunctionPdg {
            qualname: qualname.to_string(),
            node_counter: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            pending_exits: Vec::new(),
            control_stack: Vec::new(),
            pending_uses: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Node creation
    // -----------------------------------------------------------------------

    /// Create a new node, wire all pending exits to it, then leave a single
    /// `flow` pending exit from the new node. Returns the new node's ID.
    fn add_node(
        &mut self,
        node_type: &str,
        text: &str,
        line: usize,
        defines: Option<String>,
        uses: Vec<String>,
        callee_text: Option<String>,
    ) -> String {
        let id = format!("{}:node_{}", self.qualname, self.node_counter);
        self.node_counter += 1;

        // Connect all pending exits to this node.
        for exit in self.pending_exits.drain(..) {
            self.edges.push(PdgEdge {
                from: exit.from,
                to: id.clone(),
                edge_type: exit.edge_type,
            });
        }

        let branch_block_id = self.current_branch_block_id();
        self.nodes.push(PdgNode {
            id: id.clone(),
            node_type: node_type.to_string(),
            text: text.to_string(),
            line,
            defines,
            uses,
            callee_text,
            branch_block_id,
        });

        // Leave a flow exit pointing forward.
        self.pending_exits.push(PendingExit {
            from: id.clone(),
            edge_type: "flow".to_string(),
        });

        id
    }

    /// Like `add_node`, but converts the outgoing pending exit from `flow` to
    /// `true_branch` — ready for a conditional control frame.
    fn add_condition_node(&mut self, text: &str, line: usize, uses: Vec<String>) -> String {
        let id = self.add_node("condition", text, line, None, uses, None);
        // Replace the "flow" exit with "true_branch".
        if let Some(last) = self.pending_exits.last_mut() {
            last.edge_type = "true_branch".to_string();
        }
        id
    }

    /// Emit a direct edge (used for back-edges from loop bodies to the header).
    fn emit_edge(&mut self, from: &str, to: &str, edge_type: &str) {
        self.edges.push(PdgEdge {
            from: from.to_string(),
            to: to.to_string(),
            edge_type: edge_type.to_string(),
        });
    }

    /// Return the block ID of the innermost branch block the next node belongs to.
    fn current_branch_block_id(&self) -> Option<String> {
        for frame in self.control_stack.iter().rev() {
            match frame {
                ControlFrame::If { true_block_id, .. } => return Some(true_block_id.clone()),
                ControlFrame::Elif { true_block_id, .. } => return Some(true_block_id.clone()),
                ControlFrame::Else { false_block_id, .. } => return Some(false_block_id.clone()),
                ControlFrame::While { body_block_id, .. } => return Some(body_block_id.clone()),
                ControlFrame::For { body_block_id, .. } => return Some(body_block_id.clone()),
                _ => continue,
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // Event handlers
    // -----------------------------------------------------------------------

    fn handle_assignment(&mut self, target: &str, line: usize) {
        let uses = self.pending_uses.drain(..).collect();
        self.add_node("assignment", target, line, Some(target.to_string()), uses, None);
    }

    fn handle_call(&mut self, callee: &str, line: usize, arguments: &[String]) {
        let uses = self.pending_uses.drain(..).collect();

        // Create literal predecessor nodes (data-flow sources, not in pending_exits).
        let mut literal_ids: Vec<String> = Vec::new();
        for arg in arguments {
            let trimmed = arg.trim();
            if is_pdg_literal(trimmed) {
                let lit_id = format!("{}:node_{}", self.qualname, self.node_counter);
                self.node_counter += 1;
                let branch_block_id = self.current_branch_block_id();
                self.nodes.push(PdgNode {
                    id: lit_id.clone(),
                    node_type: "literal".to_string(),
                    text: trimmed.to_string(),
                    line,
                    defines: None,
                    uses: vec![],
                    callee_text: None,
                    branch_block_id,
                });
                literal_ids.push(lit_id);
            }
        }

        // Create the call node (wires all pending_exits → call, adds flow exit).
        let call_id = self.add_node("call", callee, line, None, uses, Some(callee.to_string()));

        // Wire data edges: literal → call node.
        for lit_id in literal_ids {
            self.edges.push(PdgEdge {
                from: lit_id,
                to: call_id.clone(),
                edge_type: "data".to_string(),
            });
        }
    }

    fn handle_return(&mut self, value: &str, line: usize) {
        let uses = self.pending_uses.drain(..).collect();
        let text = if value.is_empty() {
            "return".to_string()
        } else {
            format!("return {}", value)
        };
        self.add_node("return", &text, line, None, uses, None);
        // Return is terminal — no flow forward.
        self.pending_exits.clear();
    }

    fn handle_raise(&mut self, exception: &str, line: usize) {
        let uses = self.pending_uses.drain(..).collect();
        let text = if exception.is_empty() {
            "raise".to_string()
        } else {
            format!("raise {}", exception)
        };
        self.add_node("raise", &text, line, None, uses, None);
        self.pending_exits.clear();
    }

    fn handle_break(&mut self, line: usize) {
        let uses = self.pending_uses.drain(..).collect();
        let id = self.add_node("break", "break", line, None, uses, None);
        // Move the break's exits into the nearest enclosing loop frame.
        // (pending_exits now contains [(id, "flow")])
        let exits = self.pending_exits.drain(..).collect::<Vec<_>>();
        // Re-tag the exit as "flow" going into break_exits (it's actually
        // the break node itself that's the exit).
        let break_exit = PendingExit { from: id, edge_type: "flow".to_string() };
        // Drop `exits` (the flow from break node) — breaks don't fall through.
        drop(exits);

        // Walk stack in reverse to find the nearest loop.
        for frame in self.control_stack.iter_mut().rev() {
            match frame {
                ControlFrame::While { break_exits, .. }
                | ControlFrame::For { break_exits, .. } => {
                    break_exits.push(break_exit);
                    return;
                }
                _ => continue,
            }
        }
        // No enclosing loop (malformed code) — discard.
    }

    fn handle_continue(&mut self, _line: usize) {
        self.pending_uses.clear();
        // Connect all pending exits to the loop condition via back_edge.
        let exits = self.pending_exits.drain(..).collect::<Vec<_>>();

        // Find the nearest loop condition_id.
        let mut condition_id: Option<String> = None;
        for frame in self.control_stack.iter().rev() {
            match frame {
                ControlFrame::While { condition_id: cid, .. }
                | ControlFrame::For { condition_id: cid, .. } => {
                    condition_id = Some(cid.clone());
                    break;
                }
                _ => continue,
            }
        }

        if let Some(cid) = condition_id {
            for exit in exits {
                self.emit_edge(&exit.from, &cid, "back_edge");
            }
        }
        // Continue is terminal in terms of sequential flow.
    }

    fn handle_control_block(&mut self, block_type: &str, condition: &str, line: usize) {
        match block_type {
            "if" => {
                let uses = self.pending_uses.drain(..).collect();
                let cond_id = self.add_condition_node(condition, line, uses);
                let true_block_id = format!("{}:block_{}", self.qualname, self.node_counter);
                self.node_counter += 1;
                self.control_stack.push(ControlFrame::If {
                    condition_id: cond_id,
                    true_block_id,
                    true_exits: Vec::new(),
                });
            }
            "while" => {
                let uses = self.pending_uses.drain(..).collect();
                let cond_id = self.add_condition_node(condition, line, uses);
                let body_block_id = format!("{}:block_{}", self.qualname, self.node_counter);
                self.node_counter += 1;
                self.control_stack.push(ControlFrame::While {
                    condition_id: cond_id,
                    body_block_id,
                    break_exits: Vec::new(),
                });
            }
            "for" => {
                let uses = self.pending_uses.drain(..).collect();
                let cond_id = self.add_condition_node(condition, line, uses);
                let body_block_id = format!("{}:block_{}", self.qualname, self.node_counter);
                self.node_counter += 1;
                self.control_stack.push(ControlFrame::For {
                    condition_id: cond_id,
                    body_block_id,
                    break_exits: Vec::new(),
                });
            }
            "try" => {
                self.pending_uses.clear();
                self.control_stack.push(ControlFrame::Try {
                    _try_exits: Vec::new(),
                });
            }
            "with" => {
                self.pending_uses.clear();
                self.control_stack.push(ControlFrame::With);
            }
            "match" => {
                let uses = self.pending_uses.drain(..).collect();
                let subject_id =
                    self.add_node("condition", condition, line, None, uses, None);
                // Case arms branch from the subject — clear sequential flow.
                self.pending_exits.clear();
                self.control_stack.push(ControlFrame::Match {
                    subject_id,
                    all_case_exits: Vec::new(),
                });
            }
            _ => {
                // Unknown block type — treat linearly.
                self.pending_uses.clear();
            }
        }
    }

    fn handle_else_block(&mut self, block_type: &str, condition: &str, line: usize) {
        match block_type {
            "else" => {
                // Save the true-branch exits and start the false branch.
                let top = self.control_stack.last_mut();
                match top {
                    Some(ControlFrame::If { condition_id, true_exits, .. })
                    | Some(ControlFrame::Elif { condition_id, true_exits, .. }) => {
                        let cid = condition_id.clone();
                        // Collect any remaining pending exits as true-branch exits.
                        true_exits.extend(self.pending_exits.drain(..));
                        let saved_true = true_exits.drain(..).collect::<Vec<_>>();
                        // Remove the current frame and push Else.
                        self.control_stack.pop();
                        let false_block_id = format!("{}:block_{}", self.qualname, self.node_counter);
                        self.node_counter += 1;
                        self.control_stack.push(ControlFrame::Else {
                            false_block_id,
                            exits_from_true_branches: saved_true,
                        });
                        // Start the false branch from the condition.
                        self.pending_exits.push(PendingExit {
                            from: cid,
                            edge_type: "false_branch".to_string(),
                        });
                    }
                    _ => {} // malformed
                }
            }
            "elif" => {
                // Chain: save true exits, start false branch, add new condition.
                let top = self.control_stack.last_mut();
                match top {
                    Some(ControlFrame::If { condition_id, true_exits, .. })
                    | Some(ControlFrame::Elif { condition_id, true_exits, .. }) => {
                        let cid = condition_id.clone();
                        true_exits.extend(self.pending_exits.drain(..));
                        let saved_true = true_exits.drain(..).collect::<Vec<_>>();
                        self.control_stack.pop();

                        // False branch becomes the entry for the elif condition.
                        self.pending_exits.push(PendingExit {
                            from: cid,
                            edge_type: "false_branch".to_string(),
                        });

                        let uses = self.pending_uses.drain(..).collect();
                        let new_cond_id = self.add_condition_node(condition, line, uses);
                        let true_block_id = format!("{}:block_{}", self.qualname, self.node_counter);
                        self.node_counter += 1;

                        self.control_stack.push(ControlFrame::If {
                            condition_id: new_cond_id,
                            true_block_id,
                            true_exits: saved_true,
                        });
                    }
                    _ => {}
                }
            }
            "case" => {
                // Save exits from previous case arm, start new branch from subject.
                let top = self.control_stack.last_mut();
                if let Some(ControlFrame::Match { subject_id, all_case_exits }) = top {
                    let sid = subject_id.clone();
                    all_case_exits.extend(self.pending_exits.drain(..));
                    // Start next case from subject's false branch.
                    self.pending_exits.push(PendingExit {
                        from: sid,
                        edge_type: "false_branch".to_string(),
                    });
                    // Add a condition node for the case pattern.
                    let uses = self.pending_uses.drain(..).collect();
                    self.add_condition_node(condition, line, uses);
                }
            }
            _ => {}
        }
    }

    fn handle_end_control_block(&mut self, block_type: &str) {
        match block_type {
            "if" => {
                let frame = self.control_stack.pop();
                match frame {
                    Some(ControlFrame::If { condition_id, mut true_exits, .. }) => {
                        // No else: merge true exits + condition's false branch.
                        true_exits.extend(self.pending_exits.drain(..));
                        true_exits.push(PendingExit {
                            from: condition_id,
                            edge_type: "false_branch".to_string(),
                        });
                        self.pending_exits = true_exits;
                    }
                    Some(ControlFrame::Elif { condition_id, mut true_exits, .. }) => {
                        true_exits.extend(self.pending_exits.drain(..));
                        true_exits.push(PendingExit {
                            from: condition_id,
                            edge_type: "false_branch".to_string(),
                        });
                        self.pending_exits = true_exits;
                    }
                    Some(ControlFrame::Else { exits_from_true_branches, .. }) => {
                        // Merge both-branch exits.
                        let mut all = exits_from_true_branches;
                        all.extend(self.pending_exits.drain(..));
                        self.pending_exits = all;
                    }
                    _ => {} // mismatched end
                }
            }
            "while" | "for" => {
                let frame = self.control_stack.pop();
                let (cid, break_exits) = match frame {
                    Some(ControlFrame::While { condition_id, break_exits, .. }) => {
                        (condition_id, break_exits)
                    }
                    Some(ControlFrame::For { condition_id, break_exits, .. }) => {
                        (condition_id, break_exits)
                    }
                    _ => return,
                };
                // Connect end-of-body exits as back-edges to condition.
                let body_exits: Vec<PendingExit> = self.pending_exits.drain(..).collect();
                for exit in &body_exits {
                    self.emit_edge(&exit.from, &cid, "back_edge");
                }
                // Exit via condition's false branch + break exits.
                self.pending_exits.push(PendingExit {
                    from: cid,
                    edge_type: "false_branch".to_string(),
                });
                self.pending_exits.extend(break_exits);
            }
            "try" => {
                self.control_stack.pop();
                // Linear model: pending_exits flow through unchanged.
            }
            "with" => {
                self.control_stack.pop();
                // Linear: pending_exits flow through.
            }
            "match" => {
                let frame = self.control_stack.pop();
                if let Some(ControlFrame::Match { all_case_exits, .. }) = frame {
                    let mut all = all_case_exits;
                    all.extend(self.pending_exits.drain(..));
                    self.pending_exits = all;
                }
            }
            _ => {
                self.control_stack.pop();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Serialization
    // -----------------------------------------------------------------------

    fn to_json(&self) -> serde_json::Value {
        // Serialize a PdgNode to a JSON value.
        let serialize_node = |n: &PdgNode| {
            let mut v = serde_json::json!({
                "id": n.id,
                "type": n.node_type,
                "text": n.text,
                "line": n.line,
                "uses": n.uses,
            });
            if let Some(ref d) = n.defines {
                v["defines"] = serde_json::Value::String(d.clone());
            }
            if let Some(ref c) = n.callee_text {
                v["callee_text"] = serde_json::Value::String(c.clone());
            }
            v
        };

        // Fast path: no branch blocks.
        let has_blocks = self.nodes.iter().any(|n| n.branch_block_id.is_some());
        if !has_blocks {
            let entry = self.nodes.first().map(|n| n.id.clone());
            let nodes: Vec<serde_json::Value> = self.nodes.iter().map(serialize_node).collect();
            let edges: Vec<serde_json::Value> = self.edges.iter().map(|e| {
                serde_json::json!({"from": e.from, "to": e.to, "type": e.edge_type})
            }).collect();
            return serde_json::json!({"entry": entry, "nodes": nodes, "edges": edges});
        }

        // Collect unique block IDs in order of first appearance.
        let mut block_ids: Vec<String> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for n in &self.nodes {
            if let Some(bid) = &n.branch_block_id {
                if seen.insert(bid.clone()) {
                    block_ids.push(bid.clone());
                }
            }
        }

        // Build node_id → block_id map.
        let mut node_to_block: HashMap<&str, &str> = HashMap::new();
        for n in &self.nodes {
            if let Some(ref bid) = n.branch_block_id {
                node_to_block.insert(n.id.as_str(), bid.as_str());
            }
        }

        // All inner node IDs.
        let all_inner: std::collections::HashSet<&str> =
            node_to_block.keys().copied().collect();

        // Outer nodes (no branch_block_id).
        let mut outer_nodes: Vec<serde_json::Value> = self.nodes.iter()
            .filter(|n| n.branch_block_id.is_none())
            .map(serialize_node)
            .collect();

        // Build block nodes.
        for block_id in &block_ids {
            let inner_node_ids: std::collections::HashSet<&str> = self.nodes.iter()
                .filter(|n| n.branch_block_id.as_deref() == Some(block_id))
                .map(|n| n.id.as_str())
                .collect();

            let inner_nodes: Vec<serde_json::Value> = self.nodes.iter()
                .filter(|n| inner_node_ids.contains(n.id.as_str()))
                .map(serialize_node)
                .collect();

            let inner_edges: Vec<serde_json::Value> = self.edges.iter()
                .filter(|e| {
                    inner_node_ids.contains(e.from.as_str())
                        && inner_node_ids.contains(e.to.as_str())
                })
                .map(|e| serde_json::json!({"from": e.from, "to": e.to, "type": e.edge_type}))
                .collect();

            // Determine label from entry edge type.
            let label = self.edges.iter()
                .find(|e| {
                    inner_node_ids.contains(e.to.as_str())
                        && !all_inner.contains(e.from.as_str())
                })
                .map(|e| match e.edge_type.as_str() {
                    "true_branch" => "true",
                    "false_branch" => "false",
                    _ => "block",
                })
                .unwrap_or("block");

            outer_nodes.push(serde_json::json!({
                "id": block_id,
                "type": "block",
                "text": label,
                "line": 0,
                "uses": [],
                "pdg": {"nodes": inner_nodes, "edges": inner_edges},
            }));
        }

        // Outer edges: rewire entry/exit edges; skip inner-inner edges.
        let mut outer_edges: Vec<serde_json::Value> = Vec::new();
        for edge in &self.edges {
            let from_inner = all_inner.contains(edge.from.as_str());
            let to_inner = all_inner.contains(edge.to.as_str());
            match (from_inner, to_inner) {
                (false, false) => {
                    outer_edges.push(serde_json::json!({
                        "from": edge.from, "to": edge.to, "type": edge.edge_type
                    }));
                }
                (false, true) => {
                    // Entry edge: rewire to → block_id.
                    let bid = node_to_block[edge.to.as_str()];
                    outer_edges.push(serde_json::json!({
                        "from": edge.from, "to": bid, "type": edge.edge_type
                    }));
                }
                (true, false) => {
                    // Exit edge: rewire from → block_id.
                    let bid = node_to_block[edge.from.as_str()];
                    outer_edges.push(serde_json::json!({
                        "from": bid, "to": edge.to, "type": edge.edge_type
                    }));
                }
                (true, true) => {
                    // Inner-inner: already in block.pdg.edges; skip.
                }
            }
        }

        let entry = self.nodes.iter()
            .find(|n| n.branch_block_id.is_none())
            .map(|n| n.id.clone())
            .or_else(|| self.nodes.first().map(|n| n.id.clone()));

        serde_json::json!({
            "entry": entry,
            "nodes": outer_nodes,
            "edges": outer_edges,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_pdg_literal(s: &str) -> bool {
    if s.is_empty() { return false; }
    if s.parse::<f64>().is_ok() { return true; }
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('\'') && s.ends_with('\'')))
    {
        return true;
    }
    matches!(s, "True" | "False" | "None" | "true" | "false" | "null" | "undefined")
}

// ---------------------------------------------------------------------------
// Subscriber
// ---------------------------------------------------------------------------

pub struct PdgSubscriber {
    name: String,
    /// Completed PDGs indexed by function qualname.
    completed_pdgs: HashMap<String, FunctionPdg>,
    /// Stack of functions being processed (for nested functions).
    function_stack: Vec<FunctionPdg>,
    /// Module-level PDG (captures code outside any function/class).
    module_pdg: Option<FunctionPdg>,
    /// Track class scope depth (to distinguish module-level from class-level).
    class_depth: usize,
    /// The module qualname, derived from the first scope event.
    module_qualname: Option<String>,
}

impl PdgSubscriber {
    pub fn new(name: &str) -> Self {
        PdgSubscriber {
            name: name.to_string(),
            completed_pdgs: HashMap::new(),
            function_stack: Vec::new(),
            module_pdg: None,
            class_depth: 0,
            module_qualname: None,
        }
    }

    fn is_module_level(&self) -> bool {
        self.function_stack.is_empty() && self.class_depth == 0
    }

    fn ensure_module_pdg(&mut self) {
        if self.module_pdg.is_none() {
            let qualname = self
                .module_qualname
                .clone()
                .unwrap_or_else(|| "<module>".to_string());
            self.module_pdg = Some(FunctionPdg::new(&qualname));
        }
    }

    fn derive_module_qualname(&mut self, qualname: &str) {
        if self.module_qualname.is_none() {
            if let Some(pos) = qualname.rfind('.') {
                self.module_qualname = Some(qualname[..pos].to_string());
            }
        }
    }

    fn active_pdg_mut(&mut self) -> Option<&mut FunctionPdg> {
        if !self.function_stack.is_empty() {
            self.function_stack.last_mut()
        } else if self.is_module_level() {
            self.module_pdg.as_mut()
        } else {
            None
        }
    }

    fn handle_exit_function(&mut self) {
        if let Some(pdg) = self.function_stack.pop() {
            self.completed_pdgs.insert(pdg.qualname.clone(), pdg);
        }
    }
}

impl Subscriber for PdgSubscriber {
    fn handle_event(&mut self, event: &Event) -> Result<(), String> {
        match event {
            // ------------------------------------------------------------------
            // Scope tracking
            // ------------------------------------------------------------------
            Event::EnterScope {
                scope_type: ScopeType::Function,
                qualname,
                parameters,
                ..
            } => {
                self.derive_module_qualname(qualname);
                let mut pdg = FunctionPdg::new(qualname);
                // Create parameter nodes.
                for param in parameters {
                    pdg.add_node("parameter", param, 0, Some(param.clone()), Vec::new(), None);
                }
                self.function_stack.push(pdg);
            }

            Event::ExitScope {
                scope_type: ScopeType::Function,
                ..
            } => {
                self.handle_exit_function();
            }

            Event::EnterScope {
                scope_type: ScopeType::Class,
                qualname,
                ..
            } => {
                self.derive_module_qualname(qualname);
                self.class_depth += 1;
            }

            Event::ExitScope {
                scope_type: ScopeType::Class,
                ..
            } => {
                if self.class_depth > 0 {
                    self.class_depth -= 1;
                }
            }

            // ------------------------------------------------------------------
            // Data flow: accumulate uses for the next statement
            // ------------------------------------------------------------------
            Event::UseName { name, .. } => {
                if let Some(pdg) = self.active_pdg_mut() {
                    pdg.pending_uses.push(name.clone());
                }
            }

            // ------------------------------------------------------------------
            // Statement nodes
            // ------------------------------------------------------------------
            Event::Assignment {
                target,
                target_qualname,
                line,
                ..
            } => {
                if self.is_module_level() {
                    self.derive_module_qualname(target_qualname);
                    self.ensure_module_pdg();
                }
                let line = *line;
                let target = target.clone();
                if let Some(pdg) = self.active_pdg_mut() {
                    pdg.handle_assignment(&target, line);
                }
            }

            Event::CallExpression { callee, arguments, line, .. } => {
                if self.is_module_level() {
                    self.ensure_module_pdg();
                }
                let line = *line;
                let callee = callee.clone();
                let arguments = arguments.clone();
                if let Some(pdg) = self.active_pdg_mut() {
                    pdg.handle_call(&callee, line, &arguments);
                }
            }

            Event::Return { value, line, .. } => {
                let line = *line;
                let value = value.clone();
                if let Some(pdg) = self.function_stack.last_mut() {
                    pdg.handle_return(&value, line);
                }
            }

            Event::RaiseStatement { exception, line, .. } => {
                let line = *line;
                let exception = exception.clone();
                if let Some(pdg) = self.function_stack.last_mut() {
                    pdg.handle_raise(&exception, line);
                }
            }

            Event::BreakStatement { line, .. } => {
                let line = *line;
                if let Some(pdg) = self.function_stack.last_mut() {
                    pdg.handle_break(line);
                }
            }

            Event::ContinueStatement { line, .. } => {
                let line = *line;
                if let Some(pdg) = self.function_stack.last_mut() {
                    pdg.handle_continue(line);
                }
            }

            // ------------------------------------------------------------------
            // Control flow
            // ------------------------------------------------------------------
            Event::ControlBlock {
                block_type,
                condition,
                line,
                ..
            } => {
                if self.is_module_level() {
                    self.ensure_module_pdg();
                }
                let block_type = block_type.clone();
                let condition = condition.clone();
                let line = *line;
                if let Some(pdg) = self.active_pdg_mut() {
                    pdg.handle_control_block(&block_type, &condition, line);
                }
            }

            Event::ElseBlock {
                block_type,
                condition,
                line,
                ..
            } => {
                let block_type = block_type.clone();
                let condition = condition.clone();
                let line = *line;
                if let Some(pdg) = self.active_pdg_mut() {
                    pdg.handle_else_block(&block_type, &condition, line);
                }
            }

            Event::EndControlBlock { block_type, .. } => {
                let block_type = block_type.clone();
                if let Some(pdg) = self.active_pdg_mut() {
                    pdg.handle_end_control_block(&block_type);
                }
            }

            _ => {}
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<SubscriberResult, String> {
        // Finalize any lingering functions (shouldn't happen with well-formed code).
        while !self.function_stack.is_empty() {
            self.handle_exit_function();
        }

        if let Some(module_pdg) = self.module_pdg.take() {
            self.completed_pdgs
                .insert(module_pdg.qualname.clone(), module_pdg);
        }

        let pdgs: serde_json::Map<String, serde_json::Value> = self
            .completed_pdgs
            .iter()
            .map(|(qualname, pdg)| (qualname.clone(), pdg.to_json()))
            .collect();

        Ok(SubscriberResult {
            subscriber_name: self.name.clone(),
            data: serde_json::json!({ "pdgs": pdgs }),
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

pub struct PdgSubscriberFactory {
    name: String,
}

impl PdgSubscriberFactory {
    pub fn new(name: &str) -> Self {
        PdgSubscriberFactory {
            name: name.to_string(),
        }
    }
}

impl SubscriberFactory for PdgSubscriberFactory {
    fn create(&self) -> Box<dyn Subscriber> {
        Box::new(PdgSubscriber::new(&self.name))
    }
}
