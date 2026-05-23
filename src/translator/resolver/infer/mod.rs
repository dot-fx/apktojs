mod candidates;
mod helpers;

use std::collections::HashMap;
use candidates::CANDIDATES;
use helpers::{is_resolved_method, obfuscated_call_key};
use crate::translator::dalvik::interpreter::{JsExpr, JsStmt, RegId};
use crate::translator::resolver::infer::candidates::score_candidate;
use crate::translator::resolver::infer::helpers::parse_meth_token;
use crate::translator::resolver::pool::Pool;

const HTTP_SOURCE_IDENTITY_METHODS: &[&str] = &[
    "popularMangaRequest", "popularMangaParse",
    "searchMangaRequest",  "searchMangaParse",
    "latestUpdatesRequest","latestUpdatesParse",
    "mangaDetailsParse",   "chapterListParse",
    "pageListParse",       "imageUrlParse",
    "chapterListRequest",  "pageListRequest",
    "baseUrl", "client", "headers", "headersBuilder",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymKey {
    Method(usize, u32),
    Field(usize, u32),
}

#[derive(Debug, Clone)]
pub struct Evidence {
    pub kind:   EvidenceKind,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub enum EvidenceKind {
    FlowsIntoSetter { setter_class: String, setter_method: String },
    ReceiverOf { method: String },
    PassedToKnown { method_js_name: String, param_index: u8 },
    StoredFrom { expr_type: String },
    CtorArgType { class: String, arg_index: u8 },
    NullChecked,
    Iterated,
    GetterShaped,
    ComparedToStringLiteral { literal: String },
    ComparedToIntLiteral,
    UsedAsLoopCondition,
    ResultPushedToList,
    CalledOnIteratorNext,
    AppearsBeforeStringLiteral(String),
    AppearsAfterStringLiteral(String),
}

#[derive(Debug, Default)]
pub struct EvidenceSet {
    pub class_name: Option<String>,
    pub entries: Vec<Evidence>,
}

impl EvidenceSet {
    pub fn push(&mut self, kind: EvidenceKind, weight: f32) {
        self.entries.push(Evidence { kind, weight });
    }

    pub fn total_weight(&self) -> f32 {
        self.entries.iter().map(|e| e.weight).sum()
    }
}

pub struct InferCtx {
    pub evidence: HashMap<SymKey, EvidenceSet>,
}

impl InferCtx {

    pub fn best_name(&self, key: &SymKey) -> Option<(&'static str, f32)> {
        let ev = self.evidence.get(key)?;

        let mut best: Option<(&'static str, f32)> = None;

        for candidate in CANDIDATES {
            let score = score_candidate(candidate, ev);
            if score > 0.0 {
                if best.map(|(_, s)| score > s).unwrap_or(true) {
                    best = Some((candidate.name, score));
                }
            }
        }

        best.filter(|(_, s)| *s >= 0.8)
    }

    pub fn scan_stmts(&mut self, stmts: &[JsStmt], pool: &Pool, shard: usize) {
        let mut reg_calls: HashMap<RegId, JsExpr> = HashMap::new();

        for stmt in stmts {
            match stmt {
                JsStmt::Assign { reg, expr } => {
                    let resolved = self.resolve_expr(expr, &reg_calls);
                    self.scan_resolved_expr(&resolved, pool, shard, &reg_calls);

                    if matches!(resolved, JsExpr::MethodCall { .. }) {
                        reg_calls.insert(reg.clone(), resolved);
                    } else {
                        reg_calls.remove(reg);
                    }
                }

                JsStmt::FieldSet { receiver, field, value } => {
                    let resolved_val = self.resolve_expr(value, &reg_calls);
                    if let Some(key) = self.call_key(&resolved_val, pool, shard) {
                        self.evidence.entry(key).or_default()
                            .push(EvidenceKind::FlowsIntoSetter {
                                setter_class: "this".to_string(),
                                setter_method: field.clone(),
                            }, 1.0);
                    }
                }

                JsStmt::Expr(expr) => {
                    let resolved = self.resolve_expr(expr, &reg_calls);
                    self.scan_resolved_expr(&resolved, pool, shard, &reg_calls);
                }

                JsStmt::Return(Some(expr)) => {
                    let resolved = self.resolve_expr(expr, &reg_calls);
                    self.scan_resolved_expr(&resolved, pool, shard, &reg_calls);
                }

                JsStmt::While { cond, body } => {
                    let resolved_cond = self.resolve_expr(cond, &reg_calls);
                    if let Some(key) = self.call_key(&resolved_cond, pool, shard) {
                        self.evidence.entry(key).or_default()
                            .push(EvidenceKind::UsedAsLoopCondition, 1.0);
                    }
                    self.scan_stmts(body, pool, shard);
                }

                JsStmt::DoWhile { body, cond } => {
                    let resolved_cond = self.resolve_expr(cond, &reg_calls);
                    if let Some(key) = self.call_key(&resolved_cond, pool, shard) {
                        self.evidence.entry(key).or_default()
                            .push(EvidenceKind::UsedAsLoopCondition, 1.0);
                    }
                    self.scan_stmts(body, pool, shard);
                }

                JsStmt::If { cond, then_body, else_body } => {
                    let resolved_cond = self.resolve_expr(cond, &reg_calls);
                    // null check: if (!reg) or if (reg == null)
                    self.check_null_pattern(&resolved_cond, pool, shard, &reg_calls);
                    self.scan_stmts(then_body, pool, shard);
                    self.scan_stmts(else_body, pool, shard);
                }

                JsStmt::Loop { body } => {
                    if let Some(JsStmt::If { cond, then_body, else_body }) = body.first() {
                        if else_body.is_empty()
                            && then_body.len() == 1
                            && matches!(then_body[0], JsStmt::Break)
                        {
                            let inner_cond = match cond {
                                JsExpr::UnaryOp { op: "!", expr } => expr.as_ref(),
                                other => other,
                            };
                            let resolved = self.resolve_expr(inner_cond, &reg_calls);
                            if let Some(key) = self.call_key(&resolved, pool, shard) {
                                self.evidence.entry(key).or_default()
                                    .push(EvidenceKind::UsedAsLoopCondition, 1.0);
                            }
                        }
                    }
                    self.scan_stmts(body, pool, shard);
                },

                _ => {}
            }
        }

        self.scan_stringbuilder_appends(stmts, pool, shard, &reg_calls);
    }

    fn resolve_expr<'a>(
        &self,
        expr: &'a JsExpr,
        reg_calls: &'a HashMap<RegId, JsExpr>,
    ) -> JsExpr {
        match expr {
            JsExpr::Reg(r) => reg_calls.get(r).cloned().unwrap_or_else(|| expr.clone()),
            _ => expr.clone(),
        }
    }

    fn call_key(&self, expr: &JsExpr, pool: &Pool, shard: usize) -> Option<SymKey> {
        obfuscated_call_key(expr, pool, shard).map(|(key, _)| key)
    }

    fn check_null_pattern(&mut self, cond: &JsExpr, pool: &Pool, shard: usize, reg_calls: &HashMap<RegId, JsExpr>) {
        match cond {
            JsExpr::UnaryOp { op: "!", expr } => {
                let inner = self.resolve_expr(expr, reg_calls);
                if let Some(key) = self.call_key(&inner, pool, shard) {
                    self.evidence.entry(key).or_default()
                        .push(EvidenceKind::NullChecked, 0.5);
                }
            }
            JsExpr::BinOp { op, left, right }
            if (*op == "==" || *op == "!=") && matches!(**right, JsExpr::Null) =>
                {
                    let inner = self.resolve_expr(left, reg_calls);
                    if let Some(key) = self.call_key(&inner, pool, shard) {
                        self.evidence.entry(key).or_default()
                            .push(EvidenceKind::NullChecked, 0.5);
                    }
                }
            _ => {}
        }
    }

    fn scan_resolved_expr(&mut self, expr: &JsExpr, pool: &Pool, shard: usize, reg_calls: &HashMap<RegId, JsExpr>) {
        match expr {
            JsExpr::MethodCall { receiver, method, args, .. } => {
                let real_method = if let Some((token_shard, idx)) = parse_meth_token(method) {
                    pool.methods.get(&(token_shard, idx))
                        .map(|m| m.js_name.as_deref().unwrap_or(&m.method_name).to_string())
                        .unwrap_or_else(|| method.clone())
                } else {
                    method.clone()
                };
                let resolved_recv = self.resolve_expr(receiver, reg_calls);
                let resolved_args: Vec<JsExpr> = args.iter()
                    .map(|a| self.resolve_expr(a, reg_calls))
                    .collect();

                if args.is_empty() {
                    if let Some(key) = self.call_key(expr, pool, shard) {
                        self.evidence.entry(key).or_default()
                            .push(EvidenceKind::GetterShaped, 0.2);
                    }
                }

                if is_resolved_method(&*real_method, pool, shard) {
                    if let Some(key) = self.call_key(&resolved_recv, pool, shard) {
                        self.evidence.entry(key).or_default()
                            .push(EvidenceKind::ReceiverOf { method: real_method.clone() }, 0.7);
                    }
                }

                if real_method == "iterator" || real_method == "forEach" {
                    if let Some(key) = self.call_key(&resolved_recv, pool, shard) {
                        self.evidence.entry(key).or_default()
                            .push(EvidenceKind::Iterated, 0.8);
                    }
                }

                if is_resolved_method(method, pool, shard) {
                    for (i, arg) in resolved_args.iter().enumerate() {
                        if let Some(key) = self.call_key(arg, pool, shard) {
                            self.evidence.entry(key).or_default()
                                .push(EvidenceKind::PassedToKnown {
                                    method_js_name: method.clone(),
                                    param_index: i as u8,
                                }, 0.6);
                        }
                    }
                }

                if method == "push" {
                    if let Some(arg0) = resolved_args.get(0) {
                        if let Some(key) = self.call_key(arg0, pool, shard) {
                            self.evidence.entry(key).or_default()
                                .push(EvidenceKind::ResultPushedToList, 1.0);
                        }
                    }
                }

                if let JsExpr::MethodCall { method: recv_method, .. } = &resolved_recv {
                    if recv_method == "next" {
                        if let Some(key) = self.call_key(expr, pool, shard) {
                            self.evidence.entry(key).or_default()
                                .push(EvidenceKind::CalledOnIteratorNext, 0.8);
                        }
                    }
                }
            }

            JsExpr::StaticCall { class, method, args } => {
                let resolved_args: Vec<JsExpr> = args.iter()
                    .map(|a| self.resolve_expr(a, reg_calls))
                    .collect();

                let real_method = if let Some((token_shard, idx)) = parse_meth_token(method) {
                    pool.methods.get(&(token_shard, idx))
                        .map(|m| m.js_name.as_deref().unwrap_or(&m.method_name).to_string())
                        .unwrap_or_else(|| method.clone())
                } else {
                    method.clone()
                };

                if is_resolved_method(&real_method, pool, shard) {
                    for (i, arg) in resolved_args.iter().enumerate() {
                        if let Some((key, class)) = obfuscated_call_key(arg, pool, shard) {
                            let ev = self.evidence.entry(key).or_default();
                            ev.class_name = ev.class_name.clone().or(Some(class));
                            ev.push(EvidenceKind::PassedToKnown {
                                method_js_name: real_method.clone(),
                                param_index: i as u8,
                            }, 1.0);
                        }
                    }
                }

                if real_method == "areEqual" {
                    if let (Some(target), Some(JsExpr::Str(lit))) =
                        (resolved_args.get(0), resolved_args.get(1))
                    {
                        if let Some((key, class)) = obfuscated_call_key(target, pool, shard) {
                            let ev = self.evidence.entry(key).or_default();
                            ev.class_name = ev.class_name.clone().or(Some(class));
                            ev.push(EvidenceKind::ComparedToStringLiteral {
                                literal: lit.clone(),
                            }, 0.9);
                        }
                    }
                }
            }

            JsExpr::BinOp { op, left, right }
            if matches!(*op, "==" | "!=" | "<" | ">" | "<=" | ">=") =>
                {
                    let resolved_left  = self.resolve_expr(left, reg_calls);
                    let resolved_right = self.resolve_expr(right, reg_calls);

                    if let Some(key) = self.call_key(&resolved_left, pool, shard) {
                        if matches!(resolved_right, JsExpr::Int(_)) {
                            self.evidence.entry(key).or_default()
                                .push(EvidenceKind::ComparedToIntLiteral, 0.6);
                        }
                        if matches!(resolved_right, JsExpr::Null) {
                            self.evidence.entry(key).or_default()
                                .push(EvidenceKind::NullChecked, 0.5);
                        }
                    }
                }

            _ => {}
        }
    }

    pub fn apply(&self, pool: &mut Pool) {
        for (key, _) in &self.evidence {
            let Some((name, _score)) = self.best_name(key) else { continue };

            match key {
                SymKey::Method(s, idx) => {
                    let m = match pool.methods.get(&(*s, *idx)) {
                        Some(m) => m,
                        None => continue,
                    };
                    if m.js_name.is_some() { continue; }

                    let is_framework_class = m.class_name.contains('.')
                        && !m.class_name.starts_with("eu.kanade.tachiyomi.extension");
                    if is_framework_class { continue; }

                    if let Some(m) = pool.methods.get_mut(&(*s, *idx)) {
                        m.js_name = Some(name.to_string());
                    }
                }
                SymKey::Field(s, idx) => {
                    let field_name = match pool.fields.get(&(*s, *idx)) {
                        Some(f) => f.field_name.clone(),
                        None => continue,
                    };

                    let looks_obfuscated = field_name.len() <= 3
                        && field_name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
                    if !looks_obfuscated { continue; }

                    if let Some(f) = pool.fields.get_mut(&(*s, *idx)) {
                        f.field_name = name.to_string();
                    }
                }
            }
        }
    }

    fn scan_stringbuilder_appends(
        &mut self,
        stmts: &[JsStmt],
        pool: &Pool,
        shard: usize,
        reg_calls: &HashMap<RegId, JsExpr>,
    ) {
        let mut local_regs = reg_calls.clone();
        let mut seq: Vec<JsExpr> = Vec::new();

        for stmt in stmts {
            match stmt {
                JsStmt::Assign { reg, expr } => {
                    let resolved = self.resolve_expr(expr, &local_regs);
                    match &resolved {
                        JsExpr::MethodCall { .. } => { local_regs.insert(reg.clone(), resolved); }
                        JsExpr::Str(_)            => { local_regs.insert(reg.clone(), resolved); }
                        _                         => { local_regs.remove(reg); }
                    }
                }
                JsStmt::Expr(JsExpr::MethodCall { method, args, .. }) => {
                    let real_method = if let Some((ts, idx)) = parse_meth_token(method) {
                        pool.methods.get(&(ts, idx))
                            .map(|m| m.js_name.as_deref().unwrap_or(&m.method_name).to_string())
                            .unwrap_or_else(|| method.clone())
                    } else {
                        method.clone()
                    };

                    if real_method == "append" {
                        if let Some(arg) = args.get(0) {
                            seq.push(self.resolve_expr(arg, &local_regs));
                        }
                    }
                }
                _ => {}
            }
        }

        for i in 0..seq.len() {
            let Some((key, _)) = obfuscated_call_key(&seq[i], pool, shard) else { continue };
            if i > 0 {
                if let JsExpr::Str(lit) = &seq[i - 1] {
                    self.evidence.entry(key).or_default()
                        .push(EvidenceKind::AppearsAfterStringLiteral(lit.clone()), 0.9);
                }
            }
            if i + 1 < seq.len() {
                if let JsExpr::Str(lit) = &seq[i + 1] {
                    self.evidence.entry(key).or_default()
                        .push(EvidenceKind::AppearsBeforeStringLiteral(lit.clone()), 0.9);
                }
            }
        }
    }
}


impl Default for InferCtx {
    fn default() -> Self {
        Self { evidence: HashMap::new() }
    }
}

pub fn rename_source_classes(pool: &mut Pool, source_name: &str) -> HashMap<String, String> {
    let mut hits: HashMap<String, u32> = HashMap::new();
    for m in pool.methods.values() {
        let js = match m.js_name.as_deref() {
            Some(n) => n,
            None => continue,
        };
        if HTTP_SOURCE_IDENTITY_METHODS.contains(&js) {
            *hits.entry(m.class_name.clone()).or_default() += 1;
        }
    }

    for (raw_name, ti) in &pool.type_info {
        if let Some(ref sc) = ti.superclass {
            let sc_simple = pool.type_info.get(sc)
                .map(|t| t.simple_name.as_str())
                .unwrap_or(sc.as_str());
            if matches!(sc_simple, "HttpSource" | "ParsedHttpSource") {
                *hits.entry(raw_name.clone()).or_default() += 2;
            }
        }
    }

    let to_rename: Vec<String> = hits.into_iter()
        .filter(|(_, count)| *count >= 1)
        .map(|(name, _)| name)
        .collect();

    let mut renames = HashMap::new();

    for old_name in &to_rename {
        let new_name = pool.type_info.get(old_name)
            .map(|ti| ti.simple_name.clone())
            .filter(|n| n != old_name && !n.is_empty())
            .unwrap_or_else(|| source_name.to_string());

        if new_name == *old_name { continue; }
        renames.insert(old_name.clone(), new_name.clone());

        for m in pool.methods.values_mut() {
            if m.class_name == *old_name {
                m.class_name = new_name.clone();
            }
        }
        for f in pool.fields.values_mut() {
            if f.class_name == *old_name {
                f.class_name = new_name.clone();
            }
        }

        if let Some(ti) = pool.type_info.remove(old_name) {
            pool.type_info.insert(new_name.clone(), ti);
        }
    }
    renames
}