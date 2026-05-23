use crate::translator::resolver::pool::{FieldInfo, MethodInfo, Pool};

pub fn lookup_string(pool: &Pool, idx: u32) -> Option<&String> {
    (0..16).find_map(|s| pool.strings.get(&(s, idx)))
}

pub fn lookup_method(pool: &Pool, idx: u32) -> Option<&MethodInfo> {
    (0..16).find_map(|s| pool.methods.get(&(s, idx)))
}

pub fn lookup_field(pool: &Pool, idx: u32) -> Option<&FieldInfo> {
    for s in 0..16 {
        if let Some(f) = pool.fields.get(&(s, idx)) {
            return Some(f);
        }
    }
    None
}

pub fn lookup_type(pool: &Pool, idx: u32) -> Option<&String> {
    (0..16).find_map(|s| pool.types.get(&(s, idx)))
}