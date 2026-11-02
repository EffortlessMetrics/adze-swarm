//! Admission for a completed static runner's existing JSON projections.
//!
//! The runtime's `AdzeDocument::to_json_value` and document/json_projection.rs
//! remain the serialization authority. These checks require their emitted
//! fields before CLI consumers render or forward them; they infer no document
//! outcome, syntax, aliases, or diagnostics. Borrowed RawValue views retain
//! original UTF-8 bytes without constructing a recursive semantic model.
//! Process capture, deadlines and private protocol versioning are separate:
//! validating completed bytes does not bound their capture or processing cost.

use anyhow::{Context, Result, ensure};
use serde_json::{Value, value::RawValue};
use std::collections::BTreeMap;

type Object<'a> = BTreeMap<String, &'a RawValue>;

pub(crate) struct RunnerOutput {
    text: String,
}

impl RunnerOutput {
    pub(crate) fn parse(bytes: &[u8], projection: &str) -> Result<Self> {
        let text = std::str::from_utf8(bytes)
            .context("static runner output is not UTF-8")?
            .to_owned();
        // serde_json 1.0.149 validates RawValue with its iterative ignore_value
        // implementation. No recursion protection is disabled. Shallow borrowed
        // views below check fields without recursively materializing the tree.
        let raw: &RawValue = serde_json::from_str(&text)
            .context("static runner output is not one complete JSON payload")?;
        validate(raw, projection).context("static runner output has invalid projection fields")?;
        Ok(Self { text })
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    // Only visual renderers need Value materialization. Keep their existing
    // serde_json recursion protection; JSON/projection forwarding never uses it.
    pub(crate) fn json(&self) -> Result<Value> {
        serde_json::from_str(&self.text)
            .context("static runner output exceeds visual JSON decoding")
    }
}

fn object(raw: &RawValue) -> Result<Object<'_>> {
    serde_json::from_str(raw.get()).context("expected an object")
}

fn array(raw: &RawValue) -> Result<Vec<&RawValue>> {
    serde_json::from_str(raw.get()).context("expected an array")
}

fn member<'a>(value: &Object<'a>, key: &str) -> Result<&'a RawValue> {
    value
        .get(key)
        .copied()
        .with_context(|| format!("missing `{key}`"))
}

fn require(value: &Object<'_>, key: &str, check: fn(&RawValue) -> bool) -> Result<()> {
    ensure!(check(member(value, key)?), "invalid `{key}` field");
    Ok(())
}

fn unsigned(value: &RawValue) -> bool {
    serde_json::from_str::<u64>(value.get()).is_ok()
}

fn signed(value: &RawValue) -> bool {
    serde_json::from_str::<i64>(value.get()).is_ok()
}

fn boolean(value: &RawValue) -> bool {
    serde_json::from_str::<bool>(value.get()).is_ok()
}

fn string(value: &RawValue) -> bool {
    serde_json::from_str::<String>(value.get()).is_ok()
}

fn nullable_unsigned(value: &RawValue) -> bool {
    serde_json::from_str::<Option<u64>>(value.get()).is_ok()
}

fn nullable_string(value: &RawValue) -> bool {
    serde_json::from_str::<Option<String>>(value.get()).is_ok()
}

fn string_value(value: &RawValue) -> Result<String> {
    serde_json::from_str(value.get()).context("expected a string")
}

fn point(raw: &RawValue) -> Result<()> {
    let value = object(raw)?;
    require(&value, "row", unsigned)?;
    require(&value, "column", unsigned)
}

fn span(raw: &RawValue) -> Result<()> {
    let value = object(raw)?;
    let start: u64 = serde_json::from_str(member(&value, "start_byte")?.get())?;
    let end: u64 = serde_json::from_str(member(&value, "end_byte")?.get())?;
    ensure!(start <= end, "reversed byte range");
    Ok(())
}

fn language(raw: &RawValue) -> Result<()> {
    require(&object(raw)?, "name", string)
}

fn tree(raw: &RawValue) -> Result<()> {
    let value = object(raw)?;
    let mut pending = vec![member(&value, "root")?];
    while let Some(raw) = pending.pop() {
        let node = object(raw)?;
        for key in ["id", "kind_id", "grammar_id"] {
            require(&node, key, unsigned)?;
        }
        for key in ["kind", "grammar_kind"] {
            require(&node, key, string)?;
        }
        require(&node, "alias_symbol_id", nullable_unsigned)?;
        require(&node, "has_alias", boolean)?;
        require(&node, "text", nullable_string)?;
        let raw_range = member(&node, "range")?;
        span(raw_range)?;
        let range = object(raw_range)?;
        point(member(&range, "start_point")?)?;
        point(member(&range, "end_point")?)?;
        let flags = object(member(&node, "flags")?)?;
        for key in [
            "named",
            "visible",
            "extra",
            "terminal",
            "supertype",
            "error",
            "missing",
            "has_error",
        ] {
            require(&flags, key, boolean)?;
        }
        for raw in array(member(&node, "children")?)? {
            let edge = object(raw)?;
            require(&edge, "child_index", unsigned)?;
            require(&edge, "field_name", nullable_string)?;
            require(&edge, "field_id", nullable_unsigned)?;
            pending.push(member(&edge, "node")?);
        }
    }
    Ok(())
}

fn diagnostics(raw: &RawValue) -> Result<()> {
    for raw in array(raw)? {
        span(raw)?;
        let diagnostic = object(raw)?;
        let points = object(member(&diagnostic, "point_range")?)?;
        point(member(&points, "start")?)?;
        point(member(&points, "end")?)?;
        require(&diagnostic, "found", nullable_string)?;
        require(&diagnostic, "message", string)?;
        for expected in array(member(&diagnostic, "expected")?)? {
            ensure!(string(expected), "invalid expected token name");
        }
        for related in array(member(&diagnostic, "related_nodes")?)? {
            ensure!(unsigned(related), "invalid related node id");
        }
    }
    Ok(())
}

fn ambiguities(raw: &RawValue) -> Result<()> {
    for raw in array(raw)? {
        let ambiguity = object(raw)?;
        span(member(&ambiguity, "span")?)?;
        require(&ambiguity, "selected", nullable_unsigned)?;
        require(&ambiguity, "selection_reason", string)?;
        for raw in array(member(&ambiguity, "alternatives")?)? {
            let alternative = object(raw)?;
            for key in ["index", "root_symbol", "cost", "node_count"] {
                require(&alternative, key, unsigned)?;
            }
            require(&alternative, "dynamic_precedence", signed)?;
            require(&alternative, "in_error", boolean)?;
            span(member(&alternative, "span")?)?;
        }
    }
    Ok(())
}

fn validate(raw: &RawValue, projection: &str) -> Result<()> {
    let json = object(raw)?;
    let schema = match projection {
        "document-json" => "adze.document.v1",
        "tree-json" => "adze.tree.v1",
        "diagnostics-json" => "adze.diagnostics.v1",
        "ambiguity-json" => "adze.ambiguity.v1",
        _ => anyhow::bail!("unknown static runner projection `{projection}`"),
    };
    ensure!(
        string_value(member(&json, "schema")?)? == schema,
        "expected schema `{schema}`"
    );
    language(member(&json, "language")?)?;
    if projection != "document-json" {
        ensure!(
            string_value(member(&json, "document_schema")?)? == "adze.document.v1",
            "expected source document schema `adze.document.v1`"
        );
    }
    match projection {
        "document-json" => {
            require(&object(member(&json, "source")?)?, "byte_len", unsigned)?;
            let metadata = object(member(&json, "metadata")?)?;
            let outcome = string_value(member(&metadata, "outcome")?)?;
            ensure!(
                matches!(outcome.as_str(), "complete" | "recovered" | "failed"),
                "unknown document outcome"
            );
            require(&metadata, "error_count", unsigned)?;
            require(&metadata, "incremental_requested", boolean)?;
            require(&metadata, "incremental_used", boolean)?;
            require(&metadata, "fallback_reason", nullable_string)?;
            tree(member(&json, "tree")?)?;
            diagnostics(member(&json, "diagnostics")?)?;
            ambiguities(member(&json, "ambiguities")?)
        }
        "tree-json" => tree(member(&json, "tree")?),
        "diagnostics-json" => diagnostics(member(&json, "diagnostics")?),
        "ambiguity-json" => ambiguities(member(&json, "ambiguities")?),
        _ => anyhow::bail!("unknown static runner projection `{projection}`"),
    }
}
