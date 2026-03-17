use super::{GraphBuilder, ObjectType};

impl GraphBuilder {
    /// Resolve `callee_text` → `references` for every call node in every PDG.
    ///
    /// The PDG subscriber emits call nodes with a `callee_text` field containing
    /// the raw source text of the callee (e.g. `"math.sqrt"`, `"Car.drive"`).
    /// This pass uses the same LEGB resolution as the dependency graph to turn
    /// that text into a fully-qualified node ID (`references`).  The frontend
    /// uses `references` to inline the callee's PDG when the user expands a
    /// call node.
    ///
    /// Must run after `load_raw_bindings` and `load_uses` so `import_bindings`
    /// is fully populated.
    pub fn enrich_pdgs(&mut self) {
        // Collect qualnames first to avoid borrow conflicts with resolve_uses_target.
        let qualnames: Vec<String> = self
            .definitions
            .iter()
            .filter(|(_, n)| n.pdg.is_some())
            .map(|(q, _)| q.clone())
            .collect();

        for qualname in qualnames {
            let pdg_json = match self.definitions.get(&qualname).and_then(|n| n.pdg.clone()) {
                Some(p) => p,
                None => continue,
            };

            let enriched = self.resolve_call_references(&qualname, pdg_json);

            if let Some(node) = self.definitions.get_mut(&qualname) {
                node.pdg = Some(enriched);
            }
        }
    }

    /// Resolve `callee_text` → `references` for call nodes in a single PDG.
    fn resolve_call_references(
        &self,
        qualname: &str,
        mut pdg: serde_json::Value,
    ) -> serde_json::Value {
        let Some(nodes) = pdg.get_mut("nodes").and_then(|n| n.as_array_mut()) else {
            return pdg;
        };

        for pdg_node in nodes {
            let node_type = pdg_node
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            // Recurse into block containers so calls inside if/while/for bodies get resolved.
            if node_type == "block" {
                if let Some(inner_pdg) = pdg_node.get_mut("pdg") {
                    let inner = std::mem::replace(inner_pdg, serde_json::Value::Null);
                    *inner_pdg = self.resolve_call_references(qualname, inner);
                }
                continue;
            }

            if node_type != "call" {
                continue;
            }

            let callee_text = match pdg_node.get("callee_text").and_then(|c| c.as_str()) {
                Some(t) => t.to_string(),
                None => continue,
            };
            if let Some(resolved) = self.resolve_callee(qualname, &callee_text) {
                // Constructor call → substitute __init__ so the PDG expands to show internals
                let final_resolved = if let Some(node) = self.definitions.get(&resolved) {
                    if node.object_type == ObjectType::Class {
                        let init_qualname = format!("{}.__init__", resolved);
                        if self.definitions.contains_key(&init_qualname) {
                            init_qualname
                        } else {
                            resolved
                        }
                    } else {
                        resolved
                    }
                } else {
                    resolved
                };
                pdg_node["references"] = serde_json::Value::String(final_resolved);
            }
        }

        pdg
    }
}
