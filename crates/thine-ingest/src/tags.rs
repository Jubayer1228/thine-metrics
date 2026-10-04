use thine_common::{normalize_tags, tag_or, Tags};

use crate::{OtlpAnyValue, OtlpKeyValue};

pub fn kvs_to_tags(attrs: &[OtlpKeyValue]) -> Tags {
    let mut tags = Tags::new();
    for kv in attrs {
        let value = kv
            .value
            .as_ref()
            .and_then(|v| any_to_string(v))
            .unwrap_or_default();
        if !kv.key.is_empty() {
            tags.insert(kv.key.clone(), value);
        }
    }
    normalize_tags(tags)
}

pub fn any_to_string(v: &OtlpAnyValue) -> Option<String> {
    v.string_value
        .clone()
        .or_else(|| v.int_value.clone())
        .or_else(|| v.double_value.map(|d| d.to_string()))
        .or_else(|| v.bool_value.map(|b| b.to_string()))
}

pub fn service_from_tags(tags: &Tags) -> String {
    tag_or(tags, "service", "unknown").to_string()
}
