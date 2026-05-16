use super::AbiLanguageBuilder;
use adze_ir::Rule;
use proc_macro2::TokenStream;
use quote::quote;

impl<'a> AbiLanguageBuilder<'a> {
    /// Generate field names with lexicographic ordering
    pub(super) fn generate_field_names(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let mut names = Vec::new();
        let mut name_idents = Vec::new();

        // Fields must be in lexicographic order
        let mut fields: Vec<_> = self.grammar.fields.iter().collect();
        fields.sort_by_key(|(_, name)| name.as_str());

        for (i, (_id, name)) in fields.iter().enumerate() {
            let ident = quote::format_ident!("FIELD_NAME_{}", i);
            let name_bytes = format!("{}\0", name).into_bytes();
            names.push(quote! {
                static #ident: &[u8] = &[#(#name_bytes),*];
            });
            name_idents.push(ident);
        }

        let ptrs = name_idents
            .iter()
            .map(|ident| {
                quote! { SyncPtr::new(#ident.as_ptr()) }
            })
            .collect();

        (names, ptrs)
    }
    pub(super) fn field_name_indices_by_field_id(&self) -> std::collections::BTreeMap<u16, u16> {
        let mut fields: Vec<_> = self.grammar.fields.iter().collect();
        fields.sort_by_key(|(_, name)| name.as_str());
        fields
            .into_iter()
            .enumerate()
            .map(|(index, (field_id, _))| (field_id.0, index as u16))
            .collect()
    }
    /// Generate field maps
    pub(super) fn generate_field_maps(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let production_id_count = self.calculate_counts().production_id_count as usize;
        let mut field_map_slices = vec![quote! { 0u16 }; production_id_count * 2];
        let mut field_map_entries = Vec::new();
        let field_name_indices = self.field_name_indices_by_field_id();

        // Group rules by production ID
        let mut rules_by_production: std::collections::BTreeMap<u16, Vec<&Rule>> =
            std::collections::BTreeMap::new();
        for (_, rules) in &self.grammar.rules {
            for rule in rules {
                rules_by_production
                    .entry(rule.production_id.0)
                    .or_default()
                    .push(rule);
            }
        }

        // Build field map entries for each production
        for (production_id, rules) in rules_by_production {
            let start_index = (field_map_entries.len() / 2) as u16;
            let mut entry_count = 0u16;

            // Process each rule with this production ID
            for rule in rules {
                // Add entries for each field in this rule
                for (field_id, position) in &rule.fields {
                    let field_id_val = field_name_indices
                        .get(&field_id.0)
                        .copied()
                        .unwrap_or(field_id.0);
                    let child_index = *position as u8;
                    let inherited = 0u8; // false - TODO: implement inheritance detection

                    // Pack TSFieldMapEntry: field_id (16 bits) | child_index (8 bits) | inherited (8 bits)
                    let packed_entry = (field_id_val as u32)
                        | ((child_index as u32) << 16)
                        | ((inherited as u32) << 24);
                    field_map_entries.push(quote! { #packed_entry as u16 });
                    field_map_entries.push(quote! { (#packed_entry >> 16) as u16 });
                    entry_count += 1;
                }
            }

            // Add slice for this production ID if it has fields
            if entry_count > 0 {
                let slice_offset = production_id as usize * 2;
                if slice_offset + 1 < field_map_slices.len() {
                    field_map_slices[slice_offset] = quote! { #start_index };
                    field_map_slices[slice_offset + 1] = quote! { #entry_count };
                }
            }
        }
        if field_map_entries.is_empty() {
            field_map_entries.push(quote! { 0u16 });
        }

        (field_map_slices, field_map_entries)
    }
}
