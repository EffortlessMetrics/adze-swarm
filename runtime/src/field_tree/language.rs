use crate::ffi::TSSymbol;

/// Language definition with field name tables
pub struct TSLanguage {
    /// Field names table (sorted lexicographically)
    pub field_names: Vec<&'static str>,
    /// Symbol names table
    pub symbol_names: Vec<&'static str>,
    /// Production field mappings: production_id -> child_index -> field_id
    pub production_field_map: Vec<Vec<Option<u16>>>,
    // ... other language data
}

impl TSLanguage {
    /// Look up a field ID by name (binary search since names are sorted)
    pub fn field_id_for_name(&self, name: &str) -> Option<u16> {
        self.field_names
            .binary_search_by_key(&name, |&n| n)
            .ok()
            .map(|idx| idx as u16)
    }

    /// Get field name by ID
    pub fn field_name(&self, id: u16) -> Option<&'static str> {
        self.field_names.get(id as usize).copied()
    }

    /// Get symbol name by ID
    pub fn symbol_name(&self, symbol: TSSymbol) -> &'static str {
        self.symbol_names
            .get(symbol as usize)
            .copied()
            .unwrap_or("ERROR")
    }

    /// Get field mappings for a production
    pub fn production_fields(&self, production_id: u16) -> &[Option<u16>] {
        self.production_field_map
            .get(production_id as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
