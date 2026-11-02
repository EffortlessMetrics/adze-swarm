//! Admission for a completed static runner's existing JSON projections.
//!
//! The runtime's `AdzeDocument::to_json_value` and document/json_projection.rs
//! remain the serialization authority. These checks require their emitted
//! fields before CLI consumers render or forward them; they infer no document
//! outcome, syntax, aliases, or diagnostics. Public JSON keeps its original
//! UTF-8 bytes. Process capture, deadlines and private protocol versioning are
//! separate work: validating completed bytes does not bound their capture.

use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub(crate) struct RunnerOutput {
    text: String,
    json: Value,
}

impl RunnerOutput {
    pub(crate) fn parse(bytes: &[u8], projection: &str) -> Result<Self> {
        let text = std::str::from_utf8(bytes)
            .context("static runner output is not UTF-8")?
            .to_owned();
        let json: Value = serde_json::from_str(&text)
            .context("static runner output is not one complete JSON payload")?;
        validate(&json, projection)
            .context("static runner output has invalid projection fields")?;
        Ok(Self { text, json })
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn json(&self) -> &Value {
        &self.json
    }
}

fn member<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .as_object()
        .context("expected an object")?
        .get(key)
        .with_context(|| format!("missing `{key}`"))
}

fn require(value: &Value, key: &str, check: fn(&Value) -> bool) -> Result<()> {
    ensure!(check(member(value, key)?), "invalid `{key}` field");
    Ok(())
}

fn unsigned(value: &Value) -> bool {
    value.as_u64().is_some()
}

fn nullable_unsigned(value: &Value) -> bool {
    value.is_null() || unsigned(value)
}

fn nullable_string(value: &Value) -> bool {
    value.is_null() || value.is_string()
}

fn point(value: &Value) -> Result<()> {
    require(value, "row", unsigned)?;
    require(value, "column", unsigned)
}

fn span(value: &Value) -> Result<()> {
    require(value, "start_byte", unsigned)?;
    require(value, "end_byte", unsigned)?;
    ensure!(
        member(value, "start_byte")?.as_u64() <= member(value, "end_byte")?.as_u64(),
        "reversed byte range"
    );
    Ok(())
}

fn language(value: &Value) -> Result<()> {
    require(value, "name", Value::is_string)
}

fn tree(value: &Value) -> Result<()> {
    let mut pending = vec![member(value, "root")?];
    while let Some(node) = pending.pop() {
        for key in ["id", "kind_id", "grammar_id"] {
            require(node, key, unsigned)?;
        }
        for key in ["kind", "grammar_kind"] {
            require(node, key, Value::is_string)?;
        }
        require(node, "alias_symbol_id", nullable_unsigned)?;
        require(node, "has_alias", Value::is_boolean)?;
        require(node, "text", nullable_string)?;
        let range = member(node, "range")?;
        span(range)?;
        point(member(range, "start_point")?)?;
        point(member(range, "end_point")?)?;
        let flags = member(node, "flags")?;
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
            require(flags, key, Value::is_boolean)?;
        }
        let children = member(node, "children")?
            .as_array()
            .context("invalid `children` array")?;
        for edge in children {
            require(edge, "child_index", unsigned)?;
            require(edge, "field_name", nullable_string)?;
            require(edge, "field_id", nullable_unsigned)?;
            pending.push(member(edge, "node")?);
        }
    }
    Ok(())
}

fn diagnostics(value: &Value) -> Result<()> {
    for diagnostic in value.as_array().context("invalid `diagnostics` array")? {
        span(diagnostic)?;
        let points = member(diagnostic, "point_range")?;
        point(member(points, "start")?)?;
        point(member(points, "end")?)?;
        require(diagnostic, "found", nullable_string)?;
        require(diagnostic, "message", Value::is_string)?;
        for expected in member(diagnostic, "expected")?
            .as_array()
            .context("invalid `expected` array")?
        {
            ensure!(expected.is_string(), "invalid expected token name");
        }
        for related in member(diagnostic, "related_nodes")?
            .as_array()
            .context("invalid `related_nodes` array")?
        {
            ensure!(unsigned(related), "invalid related node id");
        }
    }
    Ok(())
}

fn ambiguities(value: &Value) -> Result<()> {
    for ambiguity in value.as_array().context("invalid `ambiguities` array")? {
        span(member(ambiguity, "span")?)?;
        require(ambiguity, "selected", nullable_unsigned)?;
        require(ambiguity, "selection_reason", Value::is_string)?;
        let alternatives = member(ambiguity, "alternatives")?
            .as_array()
            .context("invalid `alternatives` array")?;
        for alternative in alternatives {
            for key in ["index", "root_symbol", "cost", "node_count"] {
                require(alternative, key, unsigned)?;
            }
            require(alternative, "dynamic_precedence", |value| {
                value.as_i64().is_some()
            })?;
            require(alternative, "in_error", Value::is_boolean)?;
            span(member(alternative, "span")?)?;
        }
    }
    Ok(())
}

fn validate(json: &Value, projection: &str) -> Result<()> {
    let schema = match projection {
        "document-json" => "adze.document.v1",
        "tree-json" => "adze.tree.v1",
        "diagnostics-json" => "adze.diagnostics.v1",
        "ambiguity-json" => "adze.ambiguity.v1",
        _ => anyhow::bail!("unknown static runner projection `{projection}`"),
    };
    ensure!(
        member(json, "schema")?.as_str() == Some(schema),
        "expected schema `{schema}`"
    );
    language(member(json, "language")?)?;
    if projection != "document-json" {
        ensure!(
            member(json, "document_schema")?.as_str() == Some("adze.document.v1"),
            "expected source document schema `adze.document.v1`"
        );
    }
    match projection {
        "document-json" => {
            require(member(json, "source")?, "byte_len", unsigned)?;
            let metadata = member(json, "metadata")?;
            ensure!(
                matches!(
                    member(metadata, "outcome")?.as_str(),
                    Some("complete" | "recovered" | "failed")
                ),
                "unknown document outcome"
            );
            require(metadata, "error_count", unsigned)?;
            require(metadata, "incremental_requested", Value::is_boolean)?;
            require(metadata, "incremental_used", Value::is_boolean)?;
            require(metadata, "fallback_reason", nullable_string)?;
            tree(member(json, "tree")?)?;
            diagnostics(member(json, "diagnostics")?)?;
            ambiguities(member(json, "ambiguities")?)
        }
        "tree-json" => tree(member(json, "tree")?),
        "diagnostics-json" => diagnostics(member(json, "diagnostics")?),
        "ambiguity-json" => ambiguities(member(json, "ambiguities")?),
        _ => anyhow::bail!("unknown static runner projection `{projection}`"),
    }
}
