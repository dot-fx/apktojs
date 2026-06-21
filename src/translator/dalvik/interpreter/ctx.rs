use std::collections::HashMap;
use crate::translator::dalvik::insn::Insn;
use crate::translator::dalvik::interpreter::{JsExpr, JsStmt, RegId, TaggedStmt};
use crate::translator::resolver::pool::Pool;

pub struct LiftCtx<'a> {
    pub regs:         HashMap<u8, JsExpr>,
    pub known_constants: HashMap<u8, JsExpr>,
    pub tagged:       Vec<TaggedStmt>,
    pub warnings:     Vec<String>,
    pub result:       Option<JsExpr>,
    pub pending_call: Option<(i32, JsExpr)>,
    pub pending_new: HashMap<u8, u32>,
    pub method_name:  String,
    pub this_reg:     Option<u8>,
    pub dex_shard:    usize,
    pub current_class: String,
    pub pool: &'a Pool,
}

impl<'a> LiftCtx<'a> {
    fn reg(&self, r: u8) -> JsExpr {
        if Some(r) == self.this_reg {
            return JsExpr::This;
        }

        self.regs
            .get(&r)
            .cloned()
            .unwrap_or(JsExpr::Null)
    }

    fn set(&mut self, r: u8, expr: JsExpr, offset: i32) {
        // `r` is being overwritten with a real value here, so it no longer
        // aliases `this` -- even if it used to. Without this, `reg()`'s
        // `this_reg` check would keep firing for `r` forever, silently
        // substituting `this` for whatever actually got stored here next
        // (e.g. the result of a builder call computed a few instructions
        // earlier and reused this register number).
        if self.this_reg == Some(r) {
            self.this_reg = None;
        }

        match &expr {
            JsExpr::Int(_) | JsExpr::Null => {
                self.known_constants.insert(r, expr.clone());
            }
            _ => {
                self.known_constants.remove(&r);
            }
        }

        self.push(offset, JsStmt::Assign {
            reg: RegId { reg: r, version: 0 },
            expr: expr.clone(),
        });

        self.regs.insert(
            r,
            JsExpr::Reg(RegId {
                reg: r,
                version: 0,
            }),
        );
    }

    fn push(&mut self, offset: i32, stmt: JsStmt) {
        self.tagged.push(TaggedStmt { offset, stmt });
    }

    fn warn(&mut self, s: impl Into<String>) { self.warnings.push(s.into()); }

    pub fn flush_pending_call(&mut self, at_offset: i32) {
        if let Some((off, call)) = self.pending_call.take() {
            if self.result.is_some() {
                self.result = None;
                self.push(off, JsStmt::Expr(call));
            }
        }
    }

    fn string_ref(&self, idx: u32) -> String {
        self.pool
            .strings
            .get(&(self.dex_shard, idx))
            .cloned()
            .unwrap_or_else(|| format!("string_{}", idx))
    }

    fn type_ref(&self, idx: u32) -> String {
        self.pool
            .types
            .get(&(self.dex_shard, idx))
            .map(|s| s.trim_start_matches('[').to_string())
            .unwrap_or_else(|| format!("Type{}", idx))
    }

    fn field_ref(&self, fi: u32) -> String {
        self.pool
            .fields
            .get(&(self.dex_shard, fi))
            .map(|f| f.field_name.clone())
            .unwrap_or_else(|| format!("field{}", fi))
    }

    fn method_ref(&self, mi: u32) -> String {
        self.pool.methods
            .get(&(self.dex_shard, mi))
            .map(|m| {
                if let Some(js) = &m.js_name {
                    js.clone()
                } else {
                    format!("_meth{}_{}", self.dex_shard, mi)
                }
            })
            .unwrap_or_else(|| format!("_meth{}_{}", self.dex_shard, mi))
    }

    pub fn process(&mut self, insn: &Insn, off: i32) {
        let is_move_result = matches!(
            insn,
            Insn::MoveResult(_) | Insn::MoveResultWide(_) | Insn::MoveResultObject(_)
        );
        if !is_move_result {
            self.flush_pending_call(off);
        }

        match insn {
            Insn::Nop
            | Insn::CheckCast(..)
            | Insn::Monitor(..) => {}

            // shr/ushr by 0 is a no-op
            Insn::ShrIntLit8(d, s, 0) | Insn::UshrIntLit8(d, s, 0) => {
                let e = self.reg(*s);
                if self.this_reg == Some(*d) {
                    self.this_reg = None;
                }
                self.regs.insert(*d, e);
            }

            // Moves
            Insn::Move(d, s) | Insn::MoveWide(d, s) | Insn::MoveObject(d, s) => {
                let e = self.reg(*s);
                if Some(*s) == self.this_reg {
                    self.this_reg = Some(*d);
                    self.regs.insert(*d, JsExpr::This);
                    return;
                }
                self.set(*d, e, off);
            }

            Insn::MoveResult(d) | Insn::MoveResultWide(d) | Insn::MoveResultObject(d) => {
                let e = self.result.take()
                    .unwrap_or(JsExpr::Raw("/* no-result */".into()));
                self.pending_call = None;
                self.set(*d, e, off);
            }

            Insn::MoveException(d) => { self.set(*d, JsExpr::Raw("_ex".into()), off); }

            // Constants
            Insn::Const4(d, v)   => self.set(*d, JsExpr::Int(*v as i64), off),
            Insn::Const16(d, v)  => self.set(*d, JsExpr::Int(*v as i64), off),
            Insn::Const(d, v) | Insn::ConstHigh16(d, v) => self.set(*d, JsExpr::Int(*v as i64), off),
            Insn::ConstWide16(d, v) => self.set(*d, JsExpr::Int(*v as i64), off),
            Insn::ConstWide32(d, v) => self.set(*d, JsExpr::Int(*v as i64), off),
            Insn::ConstWide(d, v) | Insn::ConstWideHigh16(d, v) => self.set(*d, JsExpr::Int(*v), off),

            Insn::ConstString(d, idx) | Insn::ConstStringJumbo(d, idx) => {
                let s = self.string_ref(*idx);
                self.set(*d, JsExpr::Str(s), off);
            }

            Insn::ConstClass(d, idx) => {
                let ty = self.type_ref(*idx);
                self.set(*d, JsExpr::Raw(ty), off);
            }

            Insn::ConstNull(d) =>
                self.set(*d, JsExpr::Null, off),

            // Returns
            Insn::ReturnVoid => self.push(off, JsStmt::Return(None)),
            Insn::Return(r) | Insn::ReturnWide(r) | Insn::ReturnObject(r) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::Return(Some(e)));
            }

            // Instance field access — use correct shard
            Insn::IGet(d, obj, fi) | Insn::IGetWide(d, obj, fi)
            | Insn::IGetObject(d, obj, fi) | Insn::IGetBoolean(d, obj, fi) => {
                let recv  = self.reg(*obj);
                let field = self.field_ref(*fi);
                self.set(*d, JsExpr::FieldGet { receiver: Box::new(recv), field }, off);
            }

            Insn::IPut(src, obj, fi) | Insn::IPutWide(src, obj, fi)
            | Insn::IPutObject(src, obj, fi) | Insn::IPutBoolean(src, obj, fi) => {
                let recv  = self.reg(*obj);
                let value = self.reg(*src);
                let field = self.field_ref(*fi);
                self.push(off, JsStmt::FieldSet { receiver: recv, field, value });
            }

            Insn::SGet(d, fi) | Insn::SGetObject(d, fi) | Insn::SGetBoolean(d, fi) => {
                if let Some(f) = self.pool.fields.get(&(self.dex_shard, *fi)) {
                    let expr = JsExpr::StaticFieldGet {
                        class: f.class_name.clone(),
                        field: f.field_name.clone(),
                    };
                    self.set(*d, expr, off);
                } else {
                    self.warn(format!("sget: unknown field #{}", fi));
                    self.set(*d, JsExpr::Raw(format!("/* unknown field #{} */", fi)), off);
                }
            }

            Insn::SPut(src, fi) | Insn::SPutObject(src, fi)
            | Insn::SPutBoolean(src, fi) | Insn::SPutByte(src, fi)
            | Insn::SPutChar(src, fi) | Insn::SPutShort(src, fi) => {
                let value = self.reg(*src);
                if let Some(f) = self.pool.fields.get(&(self.dex_shard, *fi)) {
                    self.push(off, JsStmt::StaticSet {
                        class: f.class_name.clone(),
                        field: f.field_name.clone(),
                        value,
                    });
                } else {
                    self.warn(format!("sput: unknown field #{}", fi));
                }
            }

            // Calls
            Insn::InvokeVirtual { args, method_idx }
            | Insn::InvokeInterface { args, method_idx } => {
                let call = self.make_virtual_call(args, *method_idx);
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeSuper { args, method_idx } => {
                let arg_exprs = args.iter().skip(1).map(|r| self.reg(*r)).collect();
                let is_static = self.pool.methods
                    .get(&(self.dex_shard, *method_idx))
                    .map(|m| m.is_static)
                    .unwrap_or(false);
                let call = JsExpr::MethodCall {
                    receiver: Box::new(JsExpr::Raw("super".into())),
                    method:   self.method_ref(*method_idx),
                    args:     arg_exprs,
                    is_static
                };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeDirect { args, method_idx } => {
                if let Some(&recv_reg) = args.first() {
                    if let Some(type_idx) = self.pending_new.remove(&recv_reg) {
                        let mut ctor_args: Vec<JsExpr> = args.iter()
                            .skip(1)
                            .map(|r| self.reg(*r))
                            .collect();

                        if ctor_args.len() >= 2 {
                            let resolve = |expr: &JsExpr| -> Option<JsExpr> {
                                match expr {
                                    JsExpr::Reg(id) => self.known_constants.get(&id.reg).cloned(),
                                    other => Some(other.clone()),
                                }
                            };

                            let marker_val = resolve(&ctor_args[ctor_args.len() - 1]);
                            let mask_val   = resolve(&ctor_args[ctor_args.len() - 2]);

                            let is_marker = matches!(&marker_val, Some(JsExpr::Int(0)) | Some(JsExpr::Null) | None);
                            let is_mask   = matches!(&mask_val,   Some(JsExpr::Int(n)) if *n > 0);

                            if is_marker && is_mask {
                                if let Some(JsExpr::Int(mask)) = mask_val {
                                    ctor_args.truncate(ctor_args.len() - 2);
                                    for i in 0..ctor_args.len() {
                                        if (mask >> i) & 1 == 1 && i > 0 {
                                            ctor_args[i] = ctor_args[0].clone();
                                        }
                                    }
                                }
                            }
                        }

                        self.set(recv_reg, JsExpr::New {
                            class: self.type_ref(type_idx),
                            args: ctor_args,
                        }, off);

                        self.pending_call = None;
                        return;
                    }

                    let recv_expr = self.reg(recv_reg);

                    if matches!(recv_expr, JsExpr::This) || Some(recv_reg) == self.this_reg {
                        if let Some(m) =
                            self.pool.methods.get(&(self.dex_shard, *method_idx))
                        {

                            if m.method_name == "<init>" {

                                let ctor_args: Vec<JsExpr> = args.iter()
                                    .skip(1)
                                    .map(|r| self.reg(*r))
                                    .collect();

                                let call = if m.class_name == self.current_class
                                {
                                    JsExpr::ThisCtorCall { args: ctor_args }
                                } else {
                                    JsExpr::SuperCall { args: ctor_args }
                                };

                                self.result = Some(call.clone());
                                self.pending_call = Some((off, call));
                                return;
                            }
                        }
                    }
                }

                let call = self.make_virtual_call(args, *method_idx);

                self.result = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeStatic { args, method_idx } => {
                let arg_exprs: Vec<_> = args.iter().map(|r| self.reg(*r)).collect();

                let info = self.pool.methods.get(&(self.dex_shard, *method_idx));

                let class = info
                    .map(|m| m.class_name.clone())
                    .unwrap_or_else(|| "UnknownClass".into());

                let method = self.method_ref(*method_idx);

                let is_default_ctor = info
                    .map(|m| m.method_name.contains("$default") && m.method_name.contains("<init>"))
                    .unwrap_or(false);

                if is_default_ctor && arg_exprs.len() >= 2 {
                    let recv_reg = args[0];
                    if let Some(type_idx) = self.pending_new.remove(&recv_reg) {
                        let real_args_end = arg_exprs.len() - 2;
                        let mut ctor_args: Vec<JsExpr> = arg_exprs[1..real_args_end].to_vec();

                        let mask_expr = match &arg_exprs[arg_exprs.len() - 2] {
                            JsExpr::Reg(id) => self.known_constants.get(&id.reg).cloned(),
                            other => Some(other.clone()),
                        };

                        if let Some(JsExpr::Int(mask)) = mask_expr {
                            let mask = mask;
                            for i in 0..ctor_args.len() {
                                if (mask >> i) & 1 == 1 {
                                    ctor_args[i] = ctor_args[0].clone();
                                }
                            }
                        }

                        self.set(recv_reg, JsExpr::New {
                            class: self.type_ref(type_idx),
                            args: ctor_args,
                        }, off);

                        self.pending_call = None;
                        return;
                    }
                }

                let call = JsExpr::StaticCall { class, method, args: arg_exprs };
                self.result = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeVirtualRange { first, count, method_idx } => {
                let args: Vec<JsExpr> =
                    (*first..*first + *count)
                        .map(|r| self.reg(r))
                        .collect();

                let recv =
                    args.first()
                        .cloned()
                        .unwrap_or(JsExpr::This);

                let call_args =
                    if args.len() > 1 {
                        args[1..].to_vec()
                    } else {
                        vec![]
                    };

                let is_static = self.pool.methods
                    .get(&(self.dex_shard, *method_idx))
                    .map(|m| m.is_static)
                    .unwrap_or(false);

                let call = JsExpr::MethodCall {
                    receiver: Box::new(recv),
                    method: self.method_ref(*method_idx),
                    args: call_args,
                    is_static
                };

                self.result = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeStaticRange { first, count, method_idx } => {
                let args: Vec<JsExpr> =
                    (*first..*first + *count)
                        .map(|r| self.reg(r))
                        .collect();

                let info =
                    self.pool.methods.get(&(self.dex_shard, *method_idx));

                let class = info
                    .map(|m| m.class_name.clone())
                    .unwrap_or_else(|| "UnknownClass".into());

                let method = self.method_ref(*method_idx);

                let call = JsExpr::StaticCall {
                    class,
                    method,
                    args,
                };

                self.result = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeDirectRange { first, count, method_idx } => {
                let regs: Vec<u8> = (*first..*first + *count).collect();

                if let Some(&recv_reg) = regs.first() {

                    if let Some(type_idx) = self.pending_new.remove(&recv_reg) {
                        let mut ctor_args: Vec<JsExpr> = regs.iter()
                            .skip(1)
                            .map(|r| self.reg(*r))
                            .collect();

                        if ctor_args.len() >= 2 {
                            let resolve = |expr: &JsExpr| -> Option<JsExpr> {
                                match expr {
                                    JsExpr::Reg(id) => self.regs.get(&id.reg).cloned(),
                                    other => Some(other.clone()),
                                }
                            };

                            let marker_val = resolve(&ctor_args[ctor_args.len() - 1]);
                            let mask_val   = resolve(&ctor_args[ctor_args.len() - 2]);

                            let is_marker = matches!(&marker_val, Some(JsExpr::Int(0)) | Some(JsExpr::Null) | None);
                            let is_mask   = matches!(&mask_val,   Some(JsExpr::Int(n)) if *n > 0);

                            if is_marker && is_mask {
                                if let Some(JsExpr::Int(mask)) = mask_val {
                                    ctor_args.truncate(ctor_args.len() - 2);
                                    for i in 0..ctor_args.len() {
                                        if (mask >> i) & 1 == 1 && i > 0 {
                                            ctor_args[i] = ctor_args[0].clone();
                                        }
                                    }
                                }
                            }
                        }

                        let expr = JsExpr::New {
                            class: self.type_ref(type_idx),
                            args: ctor_args,
                        };

                        self.set(recv_reg, expr, off);
                        self.pending_call = None;
                        return;
                    }

                    // this(...) / super(...)
                    let recv_expr = self.reg(recv_reg);

                    if matches!(recv_expr, JsExpr::This) || Some(recv_reg) == self.this_reg {
                        if let Some(m) =
                            self.pool.methods.get(&(self.dex_shard, *method_idx))
                        {
                            if m.method_name == "<init>" {

                                let ctor_args: Vec<JsExpr> = regs.iter()
                                    .skip(1)
                                    .map(|r| self.reg(*r))
                                    .collect();

                                let call = if m.class_name == self.current_class
                                {
                                    JsExpr::ThisCtorCall { args: ctor_args }
                                } else {
                                    JsExpr::SuperCall { args: ctor_args }
                                };

                                self.result = Some(call.clone());
                                self.pending_call = Some((off, call));
                                return;
                            }
                        }
                    }
                }

                let args: Vec<JsExpr> = regs.iter()
                    .map(|r| self.reg(*r))
                    .collect();

                let recv = args.first().cloned().unwrap_or(JsExpr::This);

                let call_args =
                    if args.len() > 1 { args[1..].to_vec() }
                    else { vec![] };

                let is_static = self.pool.methods
                    .get(&(self.dex_shard, *method_idx))
                    .map(|m| m.is_static)
                    .unwrap_or(false);

                let call = JsExpr::MethodCall {
                    receiver: Box::new(recv),
                    method: self.method_ref(*method_idx),
                    args: call_args,
                    is_static,
                };

                self.result = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::NewInstance(d, type_idx) => {
                self.pending_new.insert(*d, *type_idx);
            }

            Insn::NewArray(d, len_reg, _type_idx) => {
                let len = self.reg(*len_reg);
                self.set(*d, JsExpr::New {
                    class: "Array".into(),
                    args: vec![len],
                }, off);
            }

            Insn::FilledNewArray { args, .. } => {
                let exprs: Vec<JsExpr> =
                    args.iter()
                        .map(|r| self.reg(*r))
                        .collect();

                self.result = Some(JsExpr::ArrayLiteral(exprs));
            }

            Insn::FilledNewArrayRange { first, count, .. } => {
                let exprs: Vec<JsExpr> =
                    (*first..*first + *count as u16)
                        .map(|r| self.reg(r as u8))
                        .collect();

                self.result = Some(JsExpr::ArrayLiteral(exprs));
            }

            Insn::AGet(d, arr, idx) | Insn::AGetObject(d, arr, idx) => {
                let ae = self.reg(*arr);
                let ie = self.reg(*idx);
                self.set(*d, JsExpr::Index { arr: Box::new(ae), idx: Box::new(ie) }, off);
            }

            Insn::APut(src, arr, idx) | Insn::APutObject(src, arr, idx) => {
                let value = self.reg(*src);
                let ae    = self.reg(*arr);
                let ie    = self.reg(*idx);
                self.push(off, JsStmt::ArraySet { arr: ae, idx: ie, value });
            }

            Insn::ArrayLength(d, arr) => {
                let ae = self.reg(*arr);
                self.set(*d, JsExpr::FieldGet {
                    receiver: Box::new(ae), field: "length".into()
                }, off);
            }

            Insn::InstanceOf(d, obj, type_idx) => {
                let oe = self.reg(*obj);
                let ty = self.type_ref(*type_idx);
                self.set(*d, JsExpr::BinOp {
                    op: "instanceof",
                    left: Box::new(oe),
                    right: Box::new(JsExpr::Raw(ty)),
                }, off);
            }

            // Branches
            Insn::Goto(rel)   => self.push(off, JsStmt::Goto(off + *rel as i32)),
            Insn::Goto16(rel) => self.push(off, JsStmt::Goto(off + *rel as i32)),
            Insn::Goto32(rel) => self.push(off, JsStmt::Goto(off + *rel)),

            Insn::IfEqz(r, rel) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp {
                        op: "===",
                        left: Box::new(e),
                        right: Box::new(JsExpr::Int(0)),
                    },
                    target: off + *rel as i32,
                });
            }
            Insn::IfNez(r, rel) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp {
                        op: "!==",
                        left: Box::new(e),
                        right: Box::new(JsExpr::Int(0)),
                    },
                    target: off + *rel as i32,
                });
            }
            Insn::IfEq(a, b, rel) => {
                let ea = self.reg(*a); let eb = self.reg(*b);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp { op: "===", left: Box::new(ea), right: Box::new(eb) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfNe(a, b, rel) => {
                let ea = self.reg(*a); let eb = self.reg(*b);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp { op: "!==", left: Box::new(ea), right: Box::new(eb) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfLt(a, b, rel) => {
                let ea = self.reg(*a); let eb = self.reg(*b);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp { op: "<", left: Box::new(ea), right: Box::new(eb) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfGt(a, b, rel) => {
                let ea = self.reg(*a); let eb = self.reg(*b);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp { op: ">", left: Box::new(ea), right: Box::new(eb) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfLe(a, b, rel) => {
                let ea = self.reg(*a); let eb = self.reg(*b);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp { op: "<=", left: Box::new(ea), right: Box::new(eb) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfGe(a, b, rel) => {
                let ea = self.reg(*a); let eb = self.reg(*b);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp { op: ">=", left: Box::new(ea), right: Box::new(eb) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfLtz(r, rel) => self.cmp_zero(*r, "<",  off, *rel as i32),
            Insn::IfGtz(r, rel) => self.cmp_zero(*r, ">",  off, *rel as i32),
            Insn::IfLez(r, rel) => self.cmp_zero(*r, "<=", off, *rel as i32),
            Insn::IfGez(r, rel) => self.cmp_zero(*r, ">=", off, *rel as i32),

            Insn::PackedSwitch { reg, first_key, targets } => {
                let e = self.reg(*reg);

                let cases: Vec<(i32, Vec<JsStmt>)> = targets.iter()
                    .enumerate()
                    .map(|(i, &abs_target)| {
                        (*first_key + i as i32, vec![JsStmt::Goto(abs_target)])
                    })
                    .collect();

                self.push(off, JsStmt::Switch {
                    expr: e,
                    cases,
                    default: None,
                });
            }

            Insn::SparseSwitch { reg, keys, targets } => {
                let e = self.reg(*reg);
                let cases: Vec<(i32, Vec<JsStmt>)> = keys.iter().zip(targets.iter())
                    .map(|(&key, &abs_target)| (key, vec![JsStmt::Goto(abs_target)]))
                    .collect();
                self.push(off, JsStmt::Switch {
                    expr: e,
                    cases,
                    default: None,
                });
            }

            // Arithmetic / logic
            Insn::NegInt(d, s) => {
                let e = self.reg(*s);
                self.set(*d, JsExpr::UnaryOp { op: "-", expr: Box::new(e) }, off);
            }
            Insn::NotInt(d, s) => {
                let e = self.reg(*s);
                self.set(*d, JsExpr::UnaryOp { op: "~", expr: Box::new(e) }, off);
            }

            // Numeric conversions transparent in JS
            Insn::IntToLong(d, s) | Insn::IntToFloat(d, s) | Insn::IntToDouble(d, s)
            | Insn::LongToInt(d, s) | Insn::LongToFloat(d, s) | Insn::LongToDouble(d, s)
            | Insn::FloatToInt(d, s) | Insn::FloatToDouble(d, s) | Insn::DoubleToInt(d, s) => {
                let e = self.reg(*s);
                self.set(*d, e, off);
            }

            // Use dedicated BitMask variant instead of abusing BinOp with empty rhs
            Insn::IntToByte(d, s) => {
                let e = self.reg(*s);
                self.set(*d, JsExpr::BitMask { expr: Box::new(e), mask: "& 0xFF" }, off);
            }
            Insn::IntToShort(d, s) => {
                let e = self.reg(*s);
                self.set(*d, JsExpr::BitMask { expr: Box::new(e), mask: "| 0" }, off);
            }
            Insn::IntToChar(d, s) => {
                let e = self.reg(*s);
                self.set(*d, JsExpr::StaticCall {
                    class: "String".into(),
                    method: "fromCharCode".into(),
                    args: vec![e]
                }, off);
            }

            Insn::AddInt(d,a,b)|Insn::AddLong(d,a,b)|Insn::AddFloat(d,a,b)|Insn::AddDouble(d,a,b)
            => self.binop(*d,*a,*b,"+",off),
            Insn::SubInt(d,a,b)|Insn::SubLong(d,a,b)|Insn::SubFloat(d,a,b)|Insn::SubDouble(d,a,b)
            => self.binop(*d,*a,*b,"-",off),
            Insn::MulInt(d,a,b)|Insn::MulLong(d,a,b)|Insn::MulFloat(d,a,b)|Insn::MulDouble(d,a,b)
            => self.binop(*d,*a,*b,"*",off),
            Insn::DivInt(d,a,b)|Insn::DivLong(d,a,b)|Insn::DivFloat(d,a,b)|Insn::DivDouble(d,a,b)
            => self.binop(*d,*a,*b,"/",off),
            Insn::RemInt(d,a,b)  => self.binop(*d,*a,*b,"%",off),
            Insn::AndInt(d,a,b)  => self.binop(*d,*a,*b,"&",off),
            Insn::OrInt(d,a,b)   => self.binop(*d,*a,*b,"|",off),
            Insn::XorInt(d,a,b)  => self.binop(*d,*a,*b,"^",off),
            Insn::ShlInt(d,a,b)  => self.binop(*d,*a,*b,"<<",off),
            Insn::ShrInt(d,a,b)  => self.binop(*d,*a,*b,">>",off),
            Insn::UshrInt(d,a,b) => self.binop(*d,*a,*b,">>>",off),
            Insn::RemIntLit16(d, s, l) => self.binop_lit(*d, *s, *l as i64, "%", off),

            Insn::AddInt2Addr(d,s)|Insn::AddLong2Addr(d,s) => self.binop2addr(*d,*s,"+",off),
            Insn::SubInt2Addr(d,s)|Insn::SubLong2Addr(d,s) => self.binop2addr(*d,*s,"-",off),
            Insn::MulInt2Addr(d,s)|Insn::MulLong2Addr(d,s) => self.binop2addr(*d,*s,"*",off),
            Insn::DivInt2Addr(d,s)|Insn::DivLong2Addr(d,s) => self.binop2addr(*d,*s,"/",off),
            Insn::RemInt2Addr(d,s) => self.binop2addr(*d,*s,"%",off),
            Insn::AndInt2Addr(d,s) => self.binop2addr(*d,*s,"&",off),
            Insn::OrInt2Addr(d,s)  => self.binop2addr(*d,*s,"|",off),
            Insn::XorInt2Addr(d,s)  => self.binop2addr(*d,*s,"^",off),
            Insn::ShlInt2Addr(d,s)  => self.binop2addr(*d,*s,"<<",off),
            Insn::ShrInt2Addr(d,s)  => self.binop2addr(*d,*s,">>",off),
            Insn::UshrInt2Addr(d,s) => self.binop2addr(*d,*s,">>>",off),

            Insn::AddIntLit16(d,s,l) => self.binop_lit(*d,*s,*l as i64,"+",off),
            Insn::MulIntLit16(d,s,l) => self.binop_lit(*d,*s,*l as i64,"*",off),
            Insn::AndIntLit16(d,s,l) => self.binop_lit(*d,*s,*l as i64,"&",off),
            Insn::OrIntLit16(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"|",off),

            Insn::AddIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,"+",off),
            Insn::MulIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,"*",off),
            Insn::DivIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,"/",off),
            Insn::RemIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,"%",off),
            Insn::AndIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,"&",off),
            Insn::OrIntLit8(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"|",off),
            Insn::XorIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,"^",off),
            Insn::ShlIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,"<<",off),
            Insn::ShrIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,">>",off),
            Insn::UshrIntLit8(d,s,l) => self.binop_lit(*d,*s,*l as i64,">>>",off),
            Insn::RsubIntLit8(d,s,l) => {
                let se = self.reg(*s);
                self.set(*d, JsExpr::BinOp {
                    op: "-",
                    left:  Box::new(JsExpr::Int(*l as i64)),
                    right: Box::new(se),
                }, off);
            }

            Insn::CmpLong(d,a,b)|Insn::CmplFloat(d,a,b)|Insn::CmpgFloat(d,a,b)
            |Insn::CmplDouble(d,a,b)|Insn::CmpgDouble(d,a,b) => {
                let ae = self.reg(*a); let be = self.reg(*b);
                self.set(*d, JsExpr::StaticCall {
                    class: "Math".into(),
                    method: "sign".into(),
                    args: vec![JsExpr::BinOp { op: "-", left: Box::new(ae), right: Box::new(be) }]
                }, off);
            }
            // Replace the old Insn::Throw in ctx.rs with this:
            Insn::Throw(r) => {
                let e = self.reg(*r);
                // Use UnaryOp so SSA can traverse and rename the register
                self.push(off, JsStmt::Expr(JsExpr::UnaryOp { op: "throw ", expr: Box::new(e) }));
            }

            Insn::FillArrayData(arr, _) => {
                let ae = self.reg(*arr);
                self.push(off, JsStmt::Comment(
                    format!("// fill_array_data({}) /* TODO */", render_expr(&ae))
                ));
            }

            Insn::Unknown(word) => {
                let msg = format!("// UNSUPPORTED opcode 0x{:02X}", word & 0xFF);
                self.warn(format!("method {}: {}", self.method_name, msg));
                self.push(off, JsStmt::Comment(msg));
            }

            #[allow(unreachable_patterns)]
            _ => {
                self.push(off, JsStmt::Comment("// unhandled insn".into()));
            }
        }
    }

    fn make_virtual_call(&mut self, args: &[u8], method_idx: u32) -> JsExpr {
        let receiver =
            if args.is_empty() {
                JsExpr::This
            } else {
                self.reg(args[0])
            };

        let mut call_args: Vec<JsExpr> =
            args.iter()
                .skip(1)
                .map(|r| self.reg(*r))
                .collect();

        for arg in &mut call_args {
            if let JsExpr::ArrayLiteral(items) = arg {
                *arg = JsExpr::StringConcat(items.clone());
            }
        }

        let is_static = self.pool.methods
            .get(&(self.dex_shard, method_idx))
            .map(|m| m.is_static)
            .unwrap_or(false);

        JsExpr::MethodCall {
            receiver: Box::new(receiver),
            method: self.method_ref(method_idx),
            args: call_args,
            is_static,
        }
    }
    fn binop(&mut self, dst: u8, a: u8, b: u8, op: &'static str, off: i32) {
        let ea = self.reg(a); let eb = self.reg(b);
        self.set(dst, JsExpr::BinOp { op, left: Box::new(ea), right: Box::new(eb) }, off);
    }
    fn binop2addr(&mut self, dst: u8, src: u8, op: &'static str, off: i32) {
        let ed = self.reg(dst); let es = self.reg(src);
        self.set(dst, JsExpr::BinOp { op, left: Box::new(ed), right: Box::new(es) }, off);
    }
    fn binop_lit(&mut self, dst: u8, src: u8, lit: i64, op: &'static str, off: i32) {
        let e = self.reg(src);
        let folded = if let JsExpr::Int(n) = &e {
            match op {
                "+" => Some(JsExpr::Int(n + lit)),
                "-" => Some(JsExpr::Int(n - lit)),
                "*" => Some(JsExpr::Int(n * lit)),
                "/" if lit != 0 => Some(JsExpr::Int(n / lit)),
                "%" if lit != 0 => Some(JsExpr::Int(n % lit)),
                _ => None,
            }
        } else { None };
        self.set(dst, folded.unwrap_or_else(|| JsExpr::BinOp {
            op, left: Box::new(e), right: Box::new(JsExpr::Int(lit)),
        }), off);
    }
    fn cmp_zero(&mut self, r: u8, op: &'static str, off: i32, rel: i32) {
        let e = self.reg(r);
        self.push(off, JsStmt::CondGoto {
            cond:   JsExpr::BinOp { op, left: Box::new(e), right: Box::new(JsExpr::Int(0)) },
            target: off + rel,
        });
    }
}

fn render_expr(expr: &JsExpr) -> String {
    match expr {
        JsExpr::Null => "null".into(),
        JsExpr::Bool(b) => b.to_string(),
        JsExpr::Int(n) => n.to_string(),
        JsExpr::Float(f) => f.to_string(),
        JsExpr::Str(s) => format!("\"{}\"", s),
        JsExpr::Reg(r) => format!("v{}_{}", r.reg, r.version),
        JsExpr::This => "this".into(),
        JsExpr::Raw(s) => s.clone(),
        JsExpr::BitMask { expr, mask } => format!("({} {})", render_expr(expr), mask),
        JsExpr::BinOp { op, left, right } => format!("({} {} {})", render_expr(left), op, render_expr(right)),
        JsExpr::UnaryOp { op, expr } => format!("{}{}", op, render_expr(expr)),
        JsExpr::Index { arr, idx } => format!("{}[{}]", render_expr(arr), render_expr(idx)),
        JsExpr::FieldGet { receiver, field } => format!("{}.{}", render_expr(receiver), field),
        JsExpr::ArrayLiteral(items) => format!(
            "[{}]",
            items.iter().map(render_expr).collect::<Vec<_>>().join(", ")
        ),
        JsExpr::StringConcat(items) => items
            .iter()
            .map(|e| format!("String({})", render_expr(e)))
            .collect::<Vec<_>>()
            .join(" + "),
        _ => "/* ... */".into(),
    }
}