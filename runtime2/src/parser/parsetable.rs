use super::Parser;
use crate::error::ParseError;
use adze_parsetable_metadata::{FORMAT_VERSION, MAGIC_NUMBER, ParsetableMetadata};

const HEADER_LEN: usize = 44;
const VERSION_RANGE: std::ops::Range<usize> = 4..8;
const METADATA_LEN_RANGE: std::ops::Range<usize> = 40..44;

struct ParsetableSections<'a> {
    metadata: Option<ParsetableMetadata>,
    table_bytes: &'a [u8],
}

impl Parser {
    /// Load GLR parse table from .parsetable file bytes
    ///
    /// This is the primary method for loading pre-generated parse tables in production.
    /// The .parsetable file format is a binary format that includes:
    /// - **Magic number**: "RSPT" (Adze Parse Table)
    /// - **Format version**: u32 version number (currently 1)
    /// - **Grammar hash**: SHA-256 hash for verification
    /// - **Metadata**: JSON metadata with grammar info, statistics, and feature flags
    /// - **ParseTable**: Postcard-serialized parse table with GLR multi-action cells
    ///
    /// # File Format Layout
    ///
    /// ```text
    /// ┌────────────────────────────────┐
    /// │ "RSPT" (4 bytes)              │ Magic number
    /// ├────────────────────────────────┤
    /// │ Version: 1 (u32 LE)           │ Format version
    /// ├────────────────────────────────┤
    /// │ Grammar Hash (32 bytes)       │ SHA-256
    /// ├────────────────────────────────┤
    /// │ Metadata Length (u32 LE)      │
    /// ├────────────────────────────────┤
    /// │ Metadata JSON (variable)      │ Grammar metadata
    /// ├────────────────────────────────┤
    /// │ Table Length (u32 LE)         │
    /// ├────────────────────────────────┤
    /// │ ParseTable (postcard)         │ Serialized parse table
    /// └────────────────────────────────┘
    /// ```
    ///
    /// # Contract
    ///
    /// - Must validate magic number and format version
    /// - Must deserialize ParseTable without data loss
    /// - Must preserve GLR multi-action cells
    /// - Must leak ParseTable for 'static lifetime (safe, immutable)
    ///
    /// # Usage Flow
    ///
    /// 1. Load .parsetable file with this method
    /// 2. Set symbol metadata with `set_symbol_metadata()`
    /// 3. Set token patterns with `set_token_patterns()`
    /// 4. Parse input with `parse()`
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if:
    /// - File is too short (< 44 bytes header)
    /// - Magic number is not "RSPT"
    /// - Format version is unsupported (not 1)
    /// - Metadata section is truncated
    /// - Table data section is truncated
    /// - ParseTable deserialization fails (corrupted postcard payload)
    ///
    /// # Specification
    ///
    /// See [`docs/specs/PARSETABLE_FILE_FORMAT_SPEC.md`](https://github.com/EffortlessMetrics/adze/blob/main/docs/specs/PARSETABLE_FILE_FORMAT_SPEC.md) for complete file format specification.
    ///
    /// See [`docs/GLR_PARSETABLE_QUICKSTART.md`](https://github.com/EffortlessMetrics/adze/blob/main/docs/GLR_PARSETABLE_QUICKSTART.md) for usage guide.
    ///
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "pure-rust", feature = "serialization")))
    )]
    pub fn load_glr_table_from_bytes(&mut self, bytes: &[u8]) -> Result<(), ParseError> {
        let sections = parse_parsetable_sections(bytes)?;
        let table = deserialize_parse_table(sections.table_bytes)?;
        let table_static: &'static adze_glr_core::ParseTable = Box::leak(Box::new(table));

        self.set_glr_table(table_static)?;
        self.parsetable_metadata = sections.metadata;

        Ok(())
    }

    /// Return metadata loaded from the last `.parsetable` file.
    ///
    /// Returns `None` when no `.parsetable` has been loaded in this parser
    /// instance or when the method is called without the required features.
    pub fn parsetable_metadata(&self) -> Option<&ParsetableMetadata> {
        self.parsetable_metadata.as_ref()
    }
}

fn parse_parsetable_sections(bytes: &[u8]) -> Result<ParsetableSections<'_>, ParseError> {
    validate_header(bytes)?;

    let metadata_len = read_le_u32(bytes, METADATA_LEN_RANGE) as usize;
    let metadata_start = HEADER_LEN;
    let metadata_end = checked_end(metadata_start, metadata_len, "metadata")?;
    ensure_available(bytes, metadata_end, "truncated metadata")?;

    let metadata = parse_metadata(&bytes[metadata_start..metadata_end])?;
    let table_len_start = metadata_end;
    let table_len_end = checked_end(table_len_start, 4, "table length")?;
    ensure_available(bytes, table_len_end, "missing table length")?;

    let table_len = read_le_u32(bytes, table_len_start..table_len_end) as usize;
    let table_start = table_len_end;
    let table_end = checked_end(table_start, table_len, "table data")?;
    ensure_available(bytes, table_end, "truncated table data")?;

    Ok(ParsetableSections {
        metadata,
        table_bytes: &bytes[table_start..table_end],
    })
}

fn validate_header(bytes: &[u8]) -> Result<(), ParseError> {
    if bytes.len() < HEADER_LEN {
        return Err(ParseError::with_msg(&format!(
            "Invalid .parsetable file: too short ({} bytes, need at least {})",
            bytes.len(),
            HEADER_LEN
        )));
    }

    let magic = &bytes[0..4];
    if magic != MAGIC_NUMBER {
        return Err(ParseError::with_msg(&format!(
            "Invalid .parsetable file: bad magic number {:?} (expected 'RSPT')",
            magic
        )));
    }

    let version = read_le_u32(bytes, VERSION_RANGE);
    if version != FORMAT_VERSION {
        return Err(ParseError::with_msg(&format!(
            "Unsupported .parsetable format version {} (expected {})",
            version, FORMAT_VERSION
        )));
    }

    Ok(())
}

fn parse_metadata(bytes: &[u8]) -> Result<Option<ParsetableMetadata>, ParseError> {
    if bytes.is_empty() {
        Ok(None)
    } else {
        ParsetableMetadata::from_bytes(bytes)
            .map(Some)
            .map_err(|e| ParseError::with_msg(&format!("Invalid .parsetable metadata: {}", e)))
    }
}

fn deserialize_parse_table(bytes: &[u8]) -> Result<adze_glr_core::ParseTable, ParseError> {
    adze_glr_core::ParseTable::from_bytes(bytes)
        .map_err(|e| ParseError::with_msg(&format!("Failed to deserialize ParseTable: {}", e)))
}

fn read_le_u32(bytes: &[u8], range: std::ops::Range<usize>) -> u32 {
    u32::from_le_bytes([
        bytes[range.start],
        bytes[range.start + 1],
        bytes[range.start + 2],
        bytes[range.start + 3],
    ])
}

fn checked_end(start: usize, len: usize, section: &str) -> Result<usize, ParseError> {
    start.checked_add(len).ok_or_else(|| {
        ParseError::with_msg(&format!(
            "Invalid .parsetable file: {} length overflows usize",
            section
        ))
    })
}

fn ensure_available(bytes: &[u8], end: usize, reason: &str) -> Result<(), ParseError> {
    if bytes.len() < end {
        Err(ParseError::with_msg(&format!(
            "Invalid .parsetable file: {} (need {} bytes, have {})",
            reason,
            end,
            bytes.len()
        )))
    } else {
        Ok(())
    }
}
