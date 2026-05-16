//! `.parsetable` byte-format loading.
//!
//! This module owns byte-level validation and deserialization so the parser
//! facade does not also have to understand the on-disk table format.

use adze_parsetable_metadata::{FORMAT_VERSION, MAGIC_NUMBER, ParsetableMetadata};

use crate::error::ParseError;

use super::Parser;

const HEADER_LEN: usize = 44;
const METADATA_LEN_OFFSET: usize = 40;
const U32_LEN: usize = 4;

struct LoadedParsetable {
    table: &'static adze_glr_core::ParseTable,
    metadata: Option<ParsetableMetadata>,
}

impl Parser {
    /// Load GLR parse table from .parsetable file bytes.
    ///
    /// This is the primary method for loading pre-generated parse tables in production.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the file header is invalid, a section is truncated,
    /// metadata cannot be decoded, or the parse table payload cannot be deserialized.
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "pure-rust", feature = "serialization")))
    )]
    pub fn load_glr_table_from_bytes(&mut self, bytes: &[u8]) -> Result<(), ParseError> {
        let loaded = parse_parsetable_bytes(bytes)?;
        self.set_glr_table(loaded.table)?;
        self.parsetable_metadata = loaded.metadata;
        Ok(())
    }
}

fn parse_parsetable_bytes(bytes: &[u8]) -> Result<LoadedParsetable, ParseError> {
    validate_header(bytes)?;

    // Skip grammar hash (bytes 8-40) for now.
    // TODO Phase 3.3: Verify hash matches expected grammar.
    let metadata_len = read_u32_at(bytes, METADATA_LEN_OFFSET)? as usize;
    let metadata_start = HEADER_LEN;
    let metadata_end = checked_section_end(metadata_start, metadata_len, "metadata")?;
    ensure_available(bytes, metadata_end, "truncated metadata")?;

    let metadata = decode_metadata(&bytes[metadata_start..metadata_end])?;

    let table_len_offset = metadata_end;
    ensure_available(bytes, table_len_offset + U32_LEN, "missing table length")?;
    let table_len = read_u32_at(bytes, table_len_offset)? as usize;
    let table_start = table_len_offset + U32_LEN;
    let table_end = checked_section_end(table_start, table_len, "table data")?;
    ensure_available(bytes, table_end, "truncated table data")?;

    let table = adze_glr_core::ParseTable::from_bytes(&bytes[table_start..table_end])
        .map_err(|e| ParseError::with_msg(&format!("Failed to deserialize ParseTable: {e}")))?;

    // Parse tables are immutable and intentionally retained for the process lifetime.
    let table = Box::leak(Box::new(table));

    Ok(LoadedParsetable { table, metadata })
}

fn validate_header(bytes: &[u8]) -> Result<(), ParseError> {
    if bytes.len() < HEADER_LEN {
        return Err(ParseError::with_msg(&format!(
            "Invalid .parsetable file: too short ({} bytes, need at least {HEADER_LEN})",
            bytes.len()
        )));
    }

    let magic = &bytes[0..4];
    if magic != MAGIC_NUMBER {
        return Err(ParseError::with_msg(&format!(
            "Invalid .parsetable file: bad magic number {:?} (expected 'RSPT')",
            magic
        )));
    }

    let version = read_u32_at(bytes, 4)?;
    if version != FORMAT_VERSION {
        return Err(ParseError::with_msg(&format!(
            "Unsupported .parsetable format version {version} (expected {FORMAT_VERSION})"
        )));
    }

    Ok(())
}

fn decode_metadata(bytes: &[u8]) -> Result<Option<ParsetableMetadata>, ParseError> {
    if bytes.is_empty() {
        return Ok(None);
    }

    ParsetableMetadata::from_bytes(bytes)
        .map(Some)
        .map_err(|e| ParseError::with_msg(&format!("Invalid .parsetable metadata: {e}")))
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Result<u32, ParseError> {
    let end = offset + U32_LEN;
    ensure_available(bytes, end, "missing u32 field")?;
    Ok(u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

fn checked_section_end(start: usize, len: usize, section: &str) -> Result<usize, ParseError> {
    start.checked_add(len).ok_or_else(|| {
        ParseError::with_msg(&format!(
            "Invalid .parsetable file: {section} length overflows usize"
        ))
    })
}

fn ensure_available(bytes: &[u8], needed: usize, reason: &str) -> Result<(), ParseError> {
    if bytes.len() < needed {
        return Err(ParseError::with_msg(&format!(
            "Invalid .parsetable file: {reason} (need {needed} bytes, have {})",
            bytes.len()
        )));
    }

    Ok(())
}
