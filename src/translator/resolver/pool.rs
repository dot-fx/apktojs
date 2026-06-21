use std::collections::HashMap;

use dex::Dex;
use dex::method::AccessFlags;
use crate::translator::resolver::mappings::{
    from_dex_type,
};

#[derive(Clone)]
pub struct Pool {
    pub strings: HashMap<(usize, u32), String>,
    pub methods: HashMap<(usize, u32), MethodInfo>,
    pub fields: HashMap<(usize, u32), FieldInfo>,
    pub types: HashMap<(usize, u32), String>,
    pub type_info: HashMap<String, TypeInfo>,
}

#[derive(Clone)]
pub struct MethodInfo {
    pub class_name: String,
    pub method_name: String,
    pub js_name: Option<String>,
    pub is_static: bool,
}

#[derive(Clone)]
pub struct FieldInfo {
    pub class_name: String,
    pub field_name: String,
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub full_name: String,
    pub simple_name: String,

    pub superclass: Option<String>,
    pub interfaces: Vec<String>,

    pub methods: Vec<String>,
}

impl Pool {

    pub fn field(&self, shard: usize, fi: u32) -> Option<&FieldInfo> {
        self.fields.get(&(shard, fi))
    }

    pub fn build(shards: &[Dex<Vec<u8>>]) -> Self {
        let mut strings = HashMap::new();
        let mut methods = HashMap::new();
        let mut fields = HashMap::new();
        let mut types = HashMap::new();
        let mut type_info = HashMap::new();

        let mut desc_to_shard: HashMap<String, usize> = HashMap::new();
        for (shard_idx, shard) in shards.iter().enumerate() {
            for class in shard.classes() {
                if let Ok(class) = class {
                    let desc = class.jtype().to_string();
                    desc_to_shard.entry(desc).or_insert(shard_idx);
                }
            }
        }

        for (shard_idx, shard) in shards.iter().enumerate() {

            for (idx, s) in shard.strings().enumerate() {
                if let Ok(s) = s {
                    strings.insert((shard_idx, idx as u32), s.to_string());
                }
            }

            for (idx, t) in shard.types().enumerate() {
                if let Ok(t) = t {
                    let name = from_dex_type(t.to_string().as_str());

                    types.insert((shard_idx, idx as u32), name.clone());

                    let simple_name = name
                        .split('.')
                        .last()
                        .unwrap_or(&name)
                        .to_string();

                    type_info.entry(name.clone()).or_insert(TypeInfo {
                        full_name: name,
                        simple_name,

                        superclass: None,
                        interfaces: vec![],

                        methods: vec![],
                    });
                }
            }

            for (meth_idx, item) in shard.method_ids().enumerate() {
                if let Ok(item) = item {

                    let class_name = shard
                        .get_type(item.class_idx() as u32)
                        .map(|t| from_dex_type(t.to_string().as_str()))
                        .unwrap_or_default();

                    let method_name = shard
                        .get_string(item.name_idx())
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    methods.insert(
                        (shard_idx, meth_idx as u32),
                        MethodInfo {
                            class_name: class_name.clone(),
                            method_name: method_name.clone(),
                            js_name: None,
                            is_static: false
                        },
                    );

                    if let Some(info) = type_info.get_mut(&class_name) {
                        info.methods.push(method_name);
                    }
                }
            }

            for (field_idx, item) in shard.field_ids().enumerate() {
                if let Ok(item) = item {

                    let class_name = shard
                        .get_type(*item.class_idx() as u32)
                        .map(|t| from_dex_type(t.to_string().as_str()))
                        .unwrap_or_default();

                    let field_name = shard
                        .get_string(*item.name_idx())
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    fields.insert(
                        (shard_idx, field_idx as u32),
                        FieldInfo {
                            class_name,
                            field_name,
                        },
                    );
                }
            }

            for class in shard.classes() {
                if let Ok(class) = class {

                    let class_name = from_dex_type(class.jtype().to_string().as_str());

                    for m in class.methods() {
                        let method_name = m.name().to_string();
                        let is_static = m.access_flags().contains(AccessFlags::STATIC);
                        let is_ctor = method_name == "<init>";

                        for info in methods.values_mut() {
                            if info.class_name == class_name && info.method_name == method_name {
                                info.is_static = is_static && !is_ctor;
                            }
                        }
                    }

                    if let Some(info) = type_info.get_mut(&class_name) {

                        // superclass
                        info.superclass = class.super_class().and_then(|id| {
                            let desc = shard.get_type(id).ok()?.to_string();
                            if desc.is_empty() { return None; }

                            Some(from_dex_type(&desc))
                        });
                        
                        // interfaces
                        info.interfaces = class
                            .interfaces()
                            .iter()
                            .map(|t| from_dex_type(t.to_string().as_str()))
                            .collect();

                        // methods
                        info.methods.extend(
                            class.methods().map(|m| {
                                m.name().to_string()
                            })
                        );
                    }
                }
            }
        }

        Pool {
            strings,
            methods,
            fields,
            types,
            type_info,
        }
    }
}