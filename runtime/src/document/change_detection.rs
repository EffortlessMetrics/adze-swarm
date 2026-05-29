use std::ops::Range;

pub(super) fn conservative_changed_ranges(old_source: &str, new_source: &str) -> Vec<Range<usize>> {
    if old_source == new_source {
        return Vec::new();
    }

    let old_bytes = old_source.as_bytes();
    let new_bytes = new_source.as_bytes();
    let common_len = old_bytes.len().min(new_bytes.len());

    let mut prefix_len = 0;
    while prefix_len < common_len && old_bytes[prefix_len] == new_bytes[prefix_len] {
        prefix_len += 1;
    }

    let mut old_suffix_start = old_bytes.len();
    let mut new_suffix_start = new_bytes.len();
    while old_suffix_start > prefix_len
        && new_suffix_start > prefix_len
        && old_bytes[old_suffix_start - 1] == new_bytes[new_suffix_start - 1]
    {
        old_suffix_start -= 1;
        new_suffix_start -= 1;
    }

    let start = previous_char_boundary(new_source, prefix_len.min(new_source.len()));
    let end = next_char_boundary(new_source, new_suffix_start.min(new_source.len()));
    std::iter::once(start..end).collect()
}

fn previous_char_boundary(source: &str, mut byte: usize) -> usize {
    while byte > 0 && !source.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

fn next_char_boundary(source: &str, mut byte: usize) -> usize {
    while byte < source.len() && !source.is_char_boundary(byte) {
        byte += 1;
    }
    byte
}
