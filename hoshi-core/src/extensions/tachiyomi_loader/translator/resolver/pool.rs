use std::collections::HashMap;
use dex::Dex;
use crate::extensions::tachiyomi_loader::translator::resolver::mappings::{from_dex_type, well_known_method};

pub struct Pool {
    pub strings: HashMap<(usize, u32), String>,
    pub methods: HashMap<(usize, u32), MethodInfo>,
    pub fields:  HashMap<(usize, u32), FieldInfo>,
    pub types:   HashMap<(usize, u32), String>,
}

pub struct MethodInfo {
    pub class_name:  String,
    pub method_name: String,
    pub js_name:     Option<String>,
}

pub struct FieldInfo {
    pub class_name: String,
    pub field_name: String,
}

impl Pool {
    pub fn build(shards: &[Dex<Vec<u8>>]) -> Self {
        let mut strings = HashMap::new();
        let mut methods = HashMap::new();
        let mut fields  = HashMap::new();
        let mut types   = HashMap::new();


        for (shard_idx, shard) in shards.iter().enumerate() {
            let mut str_idx = 0u32;
            for s in shard.strings() {
                if let Ok(s) = s {
                    strings.insert((shard_idx, str_idx), s.to_string());
                }
                str_idx += 1;
            }


            // Types
            for (idx, t) in shard.types().enumerate() {
                if let Ok(t) = t {
                    types.insert((shard_idx, idx as u32), from_dex_type(t.to_string().as_str()));
                }
            }


            // Methods
            let mut meth_idx = 0u32;
            for item in shard.method_ids() {
                if let Ok(item) = item {
                    let class_name = shard.get_type(item.class_idx() as u32)
                        .map(|t| from_dex_type(t.to_string().as_str()))
                        .unwrap_or_default();
                    let method_name = shard.get_string(item.name_idx())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let js_name = well_known_method(&class_name, &method_name);
                    methods.insert((shard_idx, meth_idx), MethodInfo { class_name, method_name, js_name });
                }
                meth_idx += 1;
            }

            // Fields
            let mut field_idx = 0u32;
            for item in shard.field_ids() {
                if let Ok(item) = item {
                    let class_name = shard.get_type(*item.class_idx() as u32)
                        .map(|t| from_dex_type(t.to_string().as_str()))
                        .unwrap_or_default();
                    let field_name = shard.get_string(*item.name_idx())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    fields.insert((shard_idx, field_idx as u32), FieldInfo { class_name, field_name });
                }
                field_idx += 1;
            }
        }


        for s in 0..shards.len() {
            let max_idx = fields.keys().filter(|(si, _)| *si == s).map(|(_, i)| *i).max();
            eprintln!("Shard {} max field idx = {:?}", s, max_idx);
        }
        Pool { strings, methods, fields, types }
    }
}