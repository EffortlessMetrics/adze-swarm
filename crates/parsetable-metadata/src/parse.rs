use crate::ParsetableMetadata;

#[must_use = "parsing may fail; the Result should be checked"]
pub(crate) fn parse_metadata_bytes(bytes: &[u8]) -> Result<ParsetableMetadata, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[must_use = "parsing may fail; the Result should be checked"]
pub(crate) fn parse_metadata_json(payload: &str) -> Result<ParsetableMetadata, serde_json::Error> {
    serde_json::from_str(payload)
}
