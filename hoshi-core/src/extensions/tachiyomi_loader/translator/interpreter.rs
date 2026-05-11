use std::collections::{HashMap, HashSet};

use crate::extensions::tachiyomi_loader::translator::dalvik::Insn;

#[derive(Debug, Clone)]
pub enum JsExpr {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Reg(u8),
    This,
    MethodCall { receiver: Box<JsExpr>, method: String, args: Vec<JsExpr> },
    StaticCall  { class: String, method: String, args: Vec<JsExpr> },
    New         { class: String, args: Vec<JsExpr> },
    FieldGet    { receiver: Box<JsExpr>, field: String },
    BinOp       { op: &'static str, left: Box<JsExpr>, right: Box<JsExpr> },
    UnaryOp     { op: &'static str, expr: Box<JsExpr> },
    Index       { arr: Box<JsExpr>, idx: Box<JsExpr> },
    Raw(String),
}

#[derive(Debug, Clone)]
pub struct TaggedStmt {
    pub offset: i32,
    pub stmt:   JsStmt,
}

#[derive(Debug, Clone)]
pub enum JsStmt {
    /// `let vN = expr;`  or  `vN = expr;`  (resolved by emit layer)
    Assign { reg: u8, expr: JsExpr },
    /// `lhs.field = rhs;`
    FieldSet { receiver: JsExpr, field: String, value: JsExpr },
    /// `arr[idx] = value;`
    ArraySet { arr: JsExpr, idx: JsExpr, value: JsExpr },
    /// side-effectful call whose result is unused
    Expr(JsExpr),
    /// `return expr;` or `return;`
    Return(Option<JsExpr>),
    /// Structured: `if (cond) { <body> }`
    If { cond: JsExpr, body: Vec<JsStmt> },
    /// Structured: `while (true) { <body> }`  (condition tested inside via break)
    Loop { body: Vec<JsStmt> },
    /// `switch (expr) { case N: ... }`
    Switch { expr: JsExpr, cases: Vec<(i32, Vec<JsStmt>)> },
    /// `break;`
    Break,
    /// `continue;`
    Continue,
    /// `// text`
    Comment(String),
    CondGoto { cond: JsExpr, target: i32 },
    Goto(i32),
}

pub fn lift(
    insns: &[Insn],
    method_name: &str,
    registers_size: u16,
    params_size: u16,
    is_static: bool,
) -> (Vec<JsStmt>, Vec<String>) {
    let this_reg = if is_static {
        None
    } else {
        Some((registers_size - params_size) as u8)
    };

    let first_param = (registers_size - params_size) as u8;

    let mut ctx = LiftCtx {
        regs:         HashMap::new(),
        tagged:       Vec::new(),
        warnings:     Vec::new(),
        result:       None,
        pending_call: None,
        pending_new:  HashMap::new(),
        method_name:  method_name.to_string(),
        this_reg,
    };

    let mut offset = 0i32;
    let mut arg_idx = 0usize;
    for i in 0..params_size {
        let r = first_param + i as u8;
        if Some(r) == ctx.this_reg { continue; }
        ctx.regs.insert(r, JsExpr::Reg(r));
        ctx.tagged.push(TaggedStmt {
            offset: -1,
            stmt: JsStmt::Assign { reg: r, expr: JsExpr::Raw(format!("arguments[{}]", arg_idx)) },
        });
        arg_idx += 1;
    }

    // Phase 1: linear decode → TaggedStmt list
    let mut offset = 0i32;
    for insn in insns {
        ctx.process(insn, offset);
        offset += insn.length_in_units() as i32;
    }
    ctx.flush_pending_call(offset);

    // Phase 2: CFG structuring
    let tagged = ctx.tagged;
    let stmts  = structure_cfg(tagged);
    (stmts, ctx.warnings)
}

struct LiftCtx {
    regs:         HashMap<u8, JsExpr>,
    tagged:       Vec<TaggedStmt>,
    warnings:     Vec<String>,
    result:       Option<JsExpr>,
    /// A call that was just emitted but not yet consumed by move-result.
    /// We defer pushing it so that if move-result follows we suppress the
    /// bare Expr and only emit the Assign.
    pending_call: Option<(i32, JsExpr)>,
    pending_new:  HashMap<u8, String>,
    method_name:  String,
    this_reg:     Option<u8>,
}

impl LiftCtx {
    fn reg(&self, r: u8) -> JsExpr {
        if Some(r) == self.this_reg { return JsExpr::This; }
        self.regs.get(&r).cloned().unwrap_or(JsExpr::Reg(r))
    }

    fn set(&mut self, r: u8, expr: JsExpr, offset: i32) {
        if let JsExpr::Reg(s) = &expr {
            if *s == r { return; }
        }

        self.regs.insert(r, JsExpr::Reg(r));
        self.push(offset, JsStmt::Assign { reg: r, expr });
    }

    fn push(&mut self, offset: i32, stmt: JsStmt) {
        self.tagged.push(TaggedStmt { offset, stmt });
    }

    fn warn(&mut self, s: impl Into<String>) { self.warnings.push(s.into()); }

    /// Flush a pending call as a bare Expr (result was not consumed).
    fn flush_pending_call(&mut self, at_offset: i32) {
        if let Some((off, call)) = self.pending_call.take() {
            self.push(off, JsStmt::Expr(call));  // uses `off`, ignores `at_offset`
        }
    }

    fn process(&mut self, insn: &Insn, off: i32) {
        self.push(i32::MIN, JsStmt::Comment(String::new()));

        // Flush previous call unless this instruction consumes its result.
        let is_move_result = matches!(
            insn,
            Insn::MoveResult(_) | Insn::MoveResultWide(_) | Insn::MoveResultObject(_)
        );
        if !is_move_result {
            self.flush_pending_call(off);
        }

        match insn {
            Insn::ShrIntLit8(d, s, l) if *l == 0 => {
                let e = self.reg(*s);
                self.regs.insert(*d, e);
                return;
            }
            Insn::Nop => {}

            Insn::Move(d, s) | Insn::MoveWide(d, s) | Insn::MoveObject(d, s) => {
                let e = self.reg(*s); self.set(*d, e, off);
            }

            Insn::MoveResult(d) | Insn::MoveResultWide(d) | Insn::MoveResultObject(d) => {
                // pending_call was NOT flushed (see above); consume the result.
                self.pending_call = None;
                if let Some(e) = self.result.take() {
                    self.set(*d, e, off);
                }
            }

            Insn::MoveException(d) => { self.set(*d, JsExpr::Raw("_ex".into()), off); }

            Insn::Const4(d, v) => {
                let e = if *v == 0 { JsExpr::Null } else { JsExpr::Int(*v as i64) };
                self.set(*d, e, off);
            }
            Insn::Const16(d, v)                          => { self.set(*d, JsExpr::Int(*v as i64), off); }
            Insn::Const(d, v) | Insn::ConstHigh16(d, v) => { self.set(*d, JsExpr::Int(*v as i64), off); }
            Insn::ConstWide16(d, v)                      => { self.set(*d, JsExpr::Int(*v as i64), off); }
            Insn::ConstWide32(d, v)                      => { self.set(*d, JsExpr::Int(*v as i64), off); }
            Insn::ConstWide(d, v) | Insn::ConstWideHigh16(d, v) => { self.set(*d, JsExpr::Int(*v), off); }

            Insn::ConstString(d, idx) | Insn::ConstStringJumbo(d, idx) => {
                self.set(*d, JsExpr::Raw(format!("/* string#{} */", idx)), off);
            }
            Insn::ConstClass(d, idx) => { self.set(*d, JsExpr::Raw(format!("/* class#{} */", idx)), off); }
            Insn::ConstNull(d)       => { self.set(*d, JsExpr::Null, off); }

            Insn::Return(r) if *r == 0 => { self.push(off, JsStmt::Return(None)); }
            Insn::Return(r) | Insn::ReturnWide(r) | Insn::ReturnObject(r) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::Return(Some(e)));
            }

            Insn::IGet(d, obj, fi) | Insn::IGetWide(d, obj, fi)
            | Insn::IGetObject(d, obj, fi) | Insn::IGetBoolean(d, obj, fi) => {
                let recv = self.reg(*obj);
                self.set(*d, JsExpr::FieldGet { receiver: Box::new(recv), field: format!("_field{}", fi) }, off);
            }

            Insn::IPut(src, obj, fi) | Insn::IPutWide(src, obj, fi)
            | Insn::IPutObject(src, obj, fi) | Insn::IPutBoolean(src, obj, fi) => {
                let recv  = self.reg(*obj);
                let value = self.reg(*src);
                self.push(off, JsStmt::FieldSet { receiver: recv, field: format!("_field{}", fi), value });
            }

            Insn::SGet(d, fi) | Insn::SGetObject(d, fi) | Insn::SGetBoolean(d, fi) => {
                self.set(*d, JsExpr::Raw(format!("/* static_field#{} */", fi)), off);
            }

            Insn::SPut(src, fi) | Insn::SPutObject(src, fi) => {
                let value = self.reg(*src);
                self.push(off, JsStmt::Comment(format!("/* sput #{}: {} */", fi, expr_to_js(&value))));
            }

            Insn::InvokeVirtual { args, method_idx }
            | Insn::InvokeInterface { args, method_idx } => {
                let call = self.make_virtual_call(args, *method_idx);
                self.result      = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeSuper { args, method_idx } => {
                let arg_exprs = args.iter().skip(1).map(|r| self.reg(*r)).collect();
                let call = JsExpr::MethodCall {
                    receiver: Box::new(JsExpr::Raw("super".into())),
                    method:   format!("_meth{}", method_idx),
                    args:     arg_exprs,
                };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeDirect { args, method_idx } => {
                if let Some(&recv_reg) = args.first() {
                    if let Some(type_ph) = self.pending_new.remove(&recv_reg) {
                        // Constructor fusion — emit new Type(args)
                        let ctor_args: Vec<JsExpr> = args.iter().skip(1).map(|r| self.reg(*r)).collect();
                        let args_js = ctor_args.iter().map(|e| expr_to_js(e)).collect::<Vec<_>>().join(", ");
                        let expr = JsExpr::Raw(format!("new {}({})", type_ph, args_js));
                        self.result = Some(expr.clone());
                        self.set(recv_reg, expr, off);
                        return;
                    }
                }
                let call = self.make_virtual_call(args, *method_idx);
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeStatic { args, method_idx } => {
                let arg_exprs: Vec<_> = args.iter().map(|r| self.reg(*r)).collect();
                let args_js = arg_exprs.iter().map(expr_to_js).collect::<Vec<_>>().join(", ");
                let call = JsExpr::Raw(format!("/* static_meth{} */({})", method_idx, args_js));
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeVirtualRange { first, count, method_idx } => {
                let args: Vec<JsExpr> = (*first..*first + *count).map(|r| self.reg(r)).collect();
                let recv      = args.first().cloned().unwrap_or(JsExpr::This);
                let call_args = if args.len() > 1 { args[1..].to_vec() } else { vec![] };
                let call = JsExpr::MethodCall { receiver: Box::new(recv), method: format!("_meth{}", method_idx), args: call_args };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeStaticRange { first, count, method_idx } => {
                let args: Vec<JsExpr> = (*first..*first + *count).map(|r| self.reg(r)).collect();
                let call = JsExpr::StaticCall { class: "/* class */".into(), method: format!("_static{}", method_idx), args };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeDirectRange { first, count, method_idx } => {
                let args: Vec<JsExpr> = (*first..*first + *count).map(|r| self.reg(r)).collect();
                let recv      = args.first().cloned().unwrap_or(JsExpr::This);
                let call_args = if args.len() > 1 { args[1..].to_vec() } else { vec![] };
                let call = JsExpr::MethodCall { receiver: Box::new(recv), method: format!("_meth{}", method_idx), args: call_args };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::NewInstance(d, type_idx) => {
                // Defer — will be fused with the following invoke-direct <init>.
                self.pending_new.insert(*d, format!("/* type#{} */", type_idx));
            }

            Insn::NewArray(d, len_reg, type_idx) => {
                let len = self.reg(*len_reg);
                self.set(*d, JsExpr::Raw(format!("new Array({}) /* type#{} */", expr_to_js(&len), type_idx)), off);
            }

            Insn::FilledNewArray { args, type_idx } => {
                let exprs: Vec<String> = args.iter().map(|r| expr_to_js(&self.reg(*r))).collect();
                self.result = Some(JsExpr::Raw(format!("[{}] /* type#{} */", exprs.join(", "), type_idx)));
            }

            Insn::AGet(d, arr, idx) | Insn::AGetObject(d, arr, idx) => {
                let ae = self.reg(*arr); let ie = self.reg(*idx);
                self.set(*d, JsExpr::Index { arr: Box::new(ae), idx: Box::new(ie) }, off);
            }

            Insn::APut(src, arr, idx) | Insn::APutObject(src, arr, idx) => {
                let value = self.reg(*src); let ae = self.reg(*arr); let ie = self.reg(*idx);
                self.push(off, JsStmt::ArraySet { arr: ae, idx: ie, value });
            }

            Insn::ArrayLength(d, arr) => {
                let ae = self.reg(*arr);
                self.set(*d, JsExpr::FieldGet { receiver: Box::new(ae), field: "length".into() }, off);
            }

            Insn::CheckCast(..) | Insn::Monitor(..) | Insn::Nop => {
                self.push(off, JsStmt::Comment(String::new()));
            }

            Insn::InstanceOf(d, obj, type_idx) => {
                let oe = self.reg(*obj);
                self.set(*d, JsExpr::Raw(format!("({} instanceof /* type#{} */)", expr_to_js(&oe), type_idx)), off);
            }

            Insn::Goto(rel)   => { self.push(off, JsStmt::Goto(off + *rel as i32)); }
            Insn::Goto16(rel) => { self.push(off, JsStmt::Goto(off + *rel as i32)); }
            Insn::Goto32(rel) => { self.push(off, JsStmt::Goto(off + *rel)); }

            Insn::IfEqz(r, rel) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::UnaryOp { op: "!", expr: Box::new(e) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfNez(r, rel) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::CondGoto {
                    cond: e,
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
            Insn::IfLt(a, b, rel) => { let ea = self.reg(*a); let eb = self.reg(*b); self.push(off, JsStmt::CondGoto { cond: JsExpr::BinOp { op: "<",  left: Box::new(ea), right: Box::new(eb) }, target: off + *rel as i32 }); }
            Insn::IfGt(a, b, rel) => { let ea = self.reg(*a); let eb = self.reg(*b); self.push(off, JsStmt::CondGoto { cond: JsExpr::BinOp { op: ">",  left: Box::new(ea), right: Box::new(eb) }, target: off + *rel as i32 }); }
            Insn::IfLe(a, b, rel) => { let ea = self.reg(*a); let eb = self.reg(*b); self.push(off, JsStmt::CondGoto { cond: JsExpr::BinOp { op: "<=", left: Box::new(ea), right: Box::new(eb) }, target: off + *rel as i32 }); }
            Insn::IfGe(a, b, rel) => { let ea = self.reg(*a); let eb = self.reg(*b); self.push(off, JsStmt::CondGoto { cond: JsExpr::BinOp { op: ">=", left: Box::new(ea), right: Box::new(eb) }, target: off + *rel as i32 }); }

            Insn::IfLtz(r, rel) => self.cmp_zero(*r, "<",  off, *rel as i32),
            Insn::IfGtz(r, rel) => self.cmp_zero(*r, ">",  off, *rel as i32),
            Insn::IfLez(r, rel) => self.cmp_zero(*r, "<=", off, *rel as i32),
            Insn::IfGez(r, rel) => self.cmp_zero(*r, ">=", off, *rel as i32),

            Insn::PackedSwitch { reg, first_key, targets } => {
                let e = self.reg(*reg);
                let cases: Vec<(i32, Vec<JsStmt>)> = targets
                    .iter()
                    .enumerate()
                    .map(|(i, &abs_target)| (*first_key + i as i32, vec![JsStmt::Goto(abs_target)]))
                    .collect();
                self.push(off, JsStmt::Switch { expr: e, cases });
            }

            Insn::SparseSwitch { reg, keys, targets } => {
                let e = self.reg(*reg);
                let cases: Vec<(i32, Vec<JsStmt>)> = keys
                    .iter()
                    .zip(targets.iter())
                    .map(|(&key, &abs_target)| (key, vec![JsStmt::Goto(abs_target)]))
                    .collect();
                self.push(off, JsStmt::Switch { expr: e, cases });
            }

            Insn::NegInt(d, s) => { let e = self.reg(*s); self.set(*d, JsExpr::UnaryOp { op: "-", expr: Box::new(e) }, off); }
            Insn::NotInt(d, s) => { let e = self.reg(*s); self.set(*d, JsExpr::UnaryOp { op: "~", expr: Box::new(e) }, off); }

            Insn::IntToLong(d, s) | Insn::IntToFloat(d, s) | Insn::IntToDouble(d, s)
            | Insn::LongToInt(d, s) | Insn::LongToFloat(d, s) | Insn::LongToDouble(d, s)
            | Insn::FloatToInt(d, s) | Insn::FloatToDouble(d, s) | Insn::DoubleToInt(d, s) => {
                let e = self.reg(*s); self.set(*d, e, off);
            }
            Insn::IntToByte(d, s) => {
                let e = self.reg(*s);
                self.set(*d, JsExpr::BinOp { op: "& 0xFF", left: Box::new(e), right: Box::new(JsExpr::Raw("".into())) }, off);
            }
            Insn::IntToChar(d, s) => {
                let e = self.reg(*s);
                self.set(*d, JsExpr::Raw(format!("String.fromCharCode({})", expr_to_js(&e))), off);
            }
            Insn::IntToShort(d, s) => {
                let e = self.reg(*s);
                self.set(*d, JsExpr::BinOp { op: "| 0", left: Box::new(e), right: Box::new(JsExpr::Raw("".into())) }, off);
            }

            Insn::AddInt(d,a,b)|Insn::AddLong(d,a,b)|Insn::AddFloat(d,a,b)|Insn::AddDouble(d,a,b) => self.binop(*d,*a,*b,"+",off),
            Insn::SubInt(d,a,b)|Insn::SubLong(d,a,b)|Insn::SubFloat(d,a,b)|Insn::SubDouble(d,a,b) => self.binop(*d,*a,*b,"-",off),
            Insn::MulInt(d,a,b)|Insn::MulLong(d,a,b)|Insn::MulFloat(d,a,b)|Insn::MulDouble(d,a,b) => self.binop(*d,*a,*b,"*",off),
            Insn::DivInt(d,a,b)|Insn::DivLong(d,a,b)|Insn::DivFloat(d,a,b)|Insn::DivDouble(d,a,b) => self.binop(*d,*a,*b,"/",off),
            Insn::RemInt(d,a,b)  => self.binop(*d,*a,*b,"%",off),
            Insn::AndInt(d,a,b)  => self.binop(*d,*a,*b,"&",off),
            Insn::OrInt(d,a,b)   => self.binop(*d,*a,*b,"|",off),
            Insn::XorInt(d,a,b)  => self.binop(*d,*a,*b,"^",off),
            Insn::ShlInt(d,a,b)  => self.binop(*d,*a,*b,"<<",off),
            Insn::ShrInt(d,a,b)  => self.binop(*d,*a,*b,">>",off),
            Insn::UshrInt(d,a,b) => self.binop(*d,*a,*b,">>>",off),

            Insn::AddInt2Addr(d,s)|Insn::AddLong2Addr(d,s) => self.binop2addr(*d,*s,"+",off),
            Insn::SubInt2Addr(d,s)|Insn::SubLong2Addr(d,s) => self.binop2addr(*d,*s,"-",off),
            Insn::MulInt2Addr(d,s)|Insn::MulLong2Addr(d,s) => self.binop2addr(*d,*s,"*",off),
            Insn::DivInt2Addr(d,s)|Insn::DivLong2Addr(d,s) => self.binop2addr(*d,*s,"/",off),
            Insn::RemInt2Addr(d,s) => self.binop2addr(*d,*s,"%",off),
            Insn::AndInt2Addr(d,s) => self.binop2addr(*d,*s,"&",off),
            Insn::OrInt2Addr(d,s)  => self.binop2addr(*d,*s,"|",off),

            Insn::AddIntLit16(d,s,l) => self.binop_lit(*d,*s,*l as i64,"+",off),
            Insn::MulIntLit16(d,s,l) => self.binop_lit(*d,*s,*l as i64,"*",off),
            Insn::AndIntLit16(d,s,l) => self.binop_lit(*d,*s,*l as i64,"&",off),
            Insn::OrIntLit16(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"|",off),

            Insn::AddIntLit8(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"+",off),
            Insn::MulIntLit8(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"*",off),
            Insn::DivIntLit8(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"/",off),
            Insn::RemIntLit8(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"%",off),
            Insn::AndIntLit8(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"&",off),
            Insn::OrIntLit8(d,s,l)   => self.binop_lit(*d,*s,*l as i64,"|",off),
            Insn::XorIntLit8(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"^",off),
            Insn::ShlIntLit8(d,s,l)  => self.binop_lit(*d,*s,*l as i64,"<<",off),
            Insn::ShrIntLit8(d,s,l)  => {
                if *l == 0 {
                    let e = self.reg(*s);
                    self.regs.insert(*d, e);
                } else {
                    self.binop_lit(*d,*s,*l as i64,">>",off)
                }
            }
            Insn::UshrIntLit8(d,s,l) => {
                if *l == 0 {
                    let e = self.reg(*s);
                    self.regs.insert(*d, e);
                } else {
                    self.binop_lit(*d,*s,*l as i64,">>>",off)
                }
            }
            Insn::RsubIntLit8(d,s,l) => {
                let se = self.reg(*s);
                self.set(*d, JsExpr::BinOp { op: "-", left: Box::new(JsExpr::Int(*l as i64)), right: Box::new(se) }, off);
            }

            Insn::CmpLong(d,a,b)|Insn::CmplFloat(d,a,b)|Insn::CmpgFloat(d,a,b)
            |Insn::CmplDouble(d,a,b)|Insn::CmpgDouble(d,a,b) => {
                let ae = self.reg(*a); let be = self.reg(*b);
                self.set(*d, JsExpr::Raw(format!("Math.sign({} - {})", expr_to_js(&ae), expr_to_js(&be))), off);
            }

            Insn::Throw(r) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::Expr(JsExpr::Raw(format!("throw {}", expr_to_js(&e)))));
            }

            Insn::Monitor(..) => {}

            Insn::FillArrayData(arr, _) => {
                let ae = self.reg(*arr);
                self.push(off, JsStmt::Comment(format!("// fill_array_data({}) /* TODO */", expr_to_js(&ae))));
            }

            Insn::Unknown(word) => {
                let msg = format!("// UNSUPPORTED opcode 0x{:02X}", word & 0xFF);
                self.warn(format!("method {}: {}", self.method_name, msg));
                self.push(off, JsStmt::Comment(msg));
            }

            #[allow(unreachable_patterns)]
            _ => {
                eprintln!("UNHANDLED: {:?} at offset {}", insn, off);
                self.push(off, JsStmt::Comment("// unhandled insn".into()));
            }
        }
    }

    fn make_virtual_call(&mut self, args: &[u8], method_idx: u32) -> JsExpr {
        let receiver  = if args.is_empty() { JsExpr::This } else { self.reg(args[0]) };
        let call_args = args.iter().skip(1).map(|r| self.reg(*r)).collect();
        JsExpr::MethodCall { receiver: Box::new(receiver), method: format!("_meth{}", method_idx), args: call_args }
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
            let result = match op {
                "+" => Some(n + lit),
                "-" => Some(n - lit),
                "*" => Some(n * lit),
                "/" if lit != 0 => Some(n / lit),
                _ => None,
            };
            result.map(JsExpr::Int)
        } else { None };
        self.set(dst, folded.unwrap_or_else(|| JsExpr::BinOp {
            op, left: Box::new(e), right: Box::new(JsExpr::Int(lit))
        }), off);
    }
    fn cmp_zero(&mut self, r: u8, op: &'static str, off: i32, rel: i32) {
        let e = self.reg(r);
        self.push(off, JsStmt::CondGoto {
            cond: JsExpr::BinOp { op, left: Box::new(e), right: Box::new(JsExpr::Int(0)) },
            target: off + rel,
        });
    }
}

// The approach:
//
//  Pass 1 — reachability from index 0 via normal (non-exception) forward flow.
//            Only offsets reachable from the entry are kept; everything else
//            is dead code (exception handler bodies, padding) and stripped.
//
//  Pass 2 — identify *real* loop headers: an offset H is a loop header only if
//            there exists a reachable back-edge Goto/CondGoto → H, AND H itself
//            is reachable from index 0.  This prevents exception-handler gotos
//            from spawning phantom loops.
//
//  Pass 3 — structure_emit: recursive descent that converts the flat list into
//            nested If / Loop / Break / Continue.
//
// Key invariant: every Goto/CondGoto stores an *absolute* DEX code-unit offset.

pub fn structure_cfg(tagged: Vec<TaggedStmt>) -> Vec<JsStmt> {
    static CALL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call_n = CALL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let tagged: Vec<TaggedStmt> = tagged
        .into_iter()
        .filter(|ts| !(ts.offset == i32::MIN && matches!(&ts.stmt, JsStmt::Comment(c) if c.is_empty())))
        .collect();

    if tagged.is_empty() { return vec![]; }

    let (prologue, tagged): (Vec<_>, Vec<_>) = tagged
        .into_iter()
        .partition(|ts| ts.offset < 0);

    if tagged.is_empty() {
        return prologue.into_iter().map(|ts| ts.stmt).collect();
    }

    let o2i: HashMap<i32, usize> = tagged.iter().enumerate()
        .fold(HashMap::new(), |mut map, (i, ts)| {
            map.insert(ts.offset, i);
            map
        });

    let loop_headers: HashSet<i32> = {
        let mut headers = HashSet::new();
        for ts in tagged.iter() {
            let src = ts.offset;
            let target = match &ts.stmt {
                JsStmt::Goto(t)                    => Some(*t),
                JsStmt::CondGoto { target: t, .. } => Some(*t),
                _ => None,
            };
            if let Some(t) = target {
                // In loop_headers building:
                if t < src {
                    // Find the statement whose offset is <= t and closest to t
                    let header_off = o2i.keys()
                        .filter(|&&k| k >= t)
                        .min()
                        .copied()
                        .or_else(|| o2i.keys().filter(|&&k| k <= t).max().copied());

                    if let Some(h) = header_off {
                        headers.insert(h);
                    }
                }
            }
        }
        headers
    };

    let body_flat: Vec<(i32, JsStmt)> = tagged.into_iter()
        .map(|ts| (ts.offset, ts.stmt))
        .collect();
    
    let mut flat: Vec<(i32, JsStmt)> = prologue.into_iter()
        .map(|ts| (ts.offset, ts.stmt))
        .collect();
    flat.extend(body_flat);

    let mut out = Vec::new();
    let mut pos = 0usize;

    let flat_o2i: HashMap<i32, usize> = flat.iter().enumerate()
        .map(|(i, (off, _))| (*off, i))
        .collect();


    structure_emit(&flat, &flat_o2i, &loop_headers, &mut pos, i32::MAX, None, &mut out, 0, None);

    out
}

fn switch_single_assign_dest(stmt: &JsStmt) -> Option<u8> {
    if let JsStmt::Switch { cases, .. } = stmt {
        let mut dest = None;
        for (_, body) in cases {
            let assign = body.iter().find(|s| matches!(s, JsStmt::Assign { .. }))?;
            if let JsStmt::Assign { reg, .. } = assign {
                match dest {
                    None => dest = Some(*reg),
                    Some(d) if d == *reg => {}
                    _ => return None,
                }
            }
        }
        dest
    } else {
        None
    }
}

fn try_merge_switches(out: &mut Vec<JsStmt>) {
    if out.len() < 2 { return; }
    let last = out.len() - 1;
    let second_last = last - 1;
    
    let sw1_dest = switch_single_assign_dest(&out[second_last]);
    let sw2_dest = switch_single_assign_dest(&out[last]);

    match (sw1_dest, sw2_dest) {
        (Some(d1), Some(d2)) if d1 == d2 => { /* same reg, merging makes sense */ }
        _ => return, // different registers or complex bodies — leave them separate
    }
    if out.len() < 2 { return; }
    let last = out.len() - 1;
    let second_last = out.len() - 2;

    let (sw1_expr, sw1_cases) = match &out[second_last] {
        JsStmt::Switch { expr, cases } => (expr.clone(), cases.clone()),
        _ => return,
    };
    let (sw2_expr, sw2_cases) = match &out[last] {
        JsStmt::Switch { expr, cases } => (expr.clone(), cases.clone()),
        _ => return,
    };

    // Only merge if both switch on the same expression
    if format!("{:?}", sw1_expr) != format!("{:?}", sw2_expr) { return; }
    
    let mut merged: Vec<(i32, Vec<JsStmt>)> = Vec::new();
    for (key, body1) in &sw1_cases {
        let body2 = sw2_cases.iter()
            .find(|(k, _)| k == key)
            .map(|(_, b)| b.clone())
            .unwrap_or_default();

        let mut combined = body1.clone();
        // Remove trailing break from body1 before combining
        if matches!(combined.last(), Some(JsStmt::Break)) {
            combined.pop();
        }
        combined.extend(body2);
        merged.push((*key, combined));
    }

    out.pop();
    out.pop();
    out.push(JsStmt::Switch { expr: sw1_expr, cases: merged });
}

const MAX_NEST: usize = 48;

fn structure_emit(
    stmts:          &[(i32, JsStmt)],
    o2i:            &HashMap<i32, usize>,
    loop_headers:   &HashSet<i32>,
    pos:            &mut usize,
    until:          i32,
    loop_exit:      Option<i32>,
    out:            &mut Vec<JsStmt>,
    depth:          usize,
    current_header: Option<i32>,
) {
    if depth > MAX_NEST {
        while *pos < stmts.len() {
            let (off, stmt) = &stmts[*pos];
            if *off >= until { break; }
            out.push(stmt.clone());
            *pos += 1;
        }
        return;
    }

    while *pos < stmts.len() {
        let (off, _) = &stmts[*pos];
        if *off >= until { break; }
        let off = *off;
        if loop_headers.contains(&off) && Some(off) != current_header {
            let header = off;
            let loop_end_off = find_loop_end(stmts, *pos, header);
            let mut body = Vec::new();
            structure_emit(stmts, o2i, loop_headers, pos,
                           loop_end_off, Some(loop_end_off), &mut body, depth + 1, Some(header));

            if *pos < stmts.len() {
                let (boff, bstmt) = &stmts[*pos];
                let is_edge = match bstmt {
                    JsStmt::Goto(t)                    => *t == header,
                    JsStmt::CondGoto { target: t, .. } => *t == header,
                    _ => false,
                };
                if is_edge { *pos += 1; }
            }
            out.push(JsStmt::Loop { body });
            continue;
        }

        *pos += 1;
        let stmt = &stmts[*pos - 1].1;

        match stmt {
            JsStmt::Goto(target) => {
                let t = *target;
                if t <= off {
                    // Back-edge inside a loop
                    out.push(JsStmt::Continue);
                } else if loop_exit.map_or(false, |exit| t >= exit) {
                    out.push(JsStmt::Break);
                } else {
                    out.push(JsStmt::Comment(format!("// goto → {}", t)));
                }
            }

            JsStmt::CondGoto { cond, target } => {
                let t = *target;

                if t <= off {
                    // Do-while back-edge: if (!cond) break; continue;
                    out.push(JsStmt::If {
                        cond: negate(cond.clone()),
                        body: vec![JsStmt::Break],
                    });
                    out.push(JsStmt::Continue);
                } else if loop_exit.map_or(false, |exit| t >= exit) {
                    let target_idx = o2i.get(&t).copied().unwrap_or(stmts.len());
                    let body_until = stmts.get(target_idx).map(|(o, _)| *o).unwrap_or(i32::MAX);
                    let mut body = Vec::new();
                    structure_emit(stmts, o2i, loop_headers, pos,
                                   body_until, loop_exit, &mut body, depth + 1, current_header);
                    if !body.is_empty() {
                        out.push(JsStmt::If { cond: negate(cond.clone()), body });  // ← negate here
                    }
                    out.push(JsStmt::Break);
                } else {
                    // Forward branch: if (!cond) { <body until t> }
                    let target_idx = o2i.get(&t).copied().unwrap_or(stmts.len());
                    let body_until = stmts.get(target_idx)
                        .map(|(o, _)| *o)
                        .unwrap_or(i32::MAX);

                    let inv = negate(cond.clone());
                    let mut body = Vec::new();
                    structure_emit(stmts, o2i, loop_headers, pos,
                                   body_until, loop_exit, &mut body, depth + 1, current_header);

                    // Check if body ends with a Goto that jumps *past* body_until
                    // (else-branch pattern) — if so, wrap as if/else.
                    let else_branch = body.last().and_then(|s| {
                        if let JsStmt::Comment(c) = s {
                            c.strip_prefix("// goto → ")
                                .and_then(|n| n.parse::<i32>().ok())
                                .filter(|&n| n > body_until)
                        } else { None }
                    });

                    if !body.is_empty() {
                        if else_branch.is_some() {
                            // Remove the trailing goto comment from body
                            let mut if_body = body;
                            if_body.pop();
                            let else_target = else_branch.unwrap();
                            let else_idx = o2i.get(&else_target).copied().unwrap_or(stmts.len());
                            let else_until = stmts.get(else_idx)
                                .map(|(o, _)| *o)
                                .unwrap_or(i32::MAX);
                            let mut else_body = Vec::new();
                            structure_emit(stmts, o2i, loop_headers, pos,
                                           else_until, loop_exit, &mut else_body, depth + 1, current_header);
                            // Emit as if/else via two separate ifs for now
                            out.push(JsStmt::If { cond: inv.clone(), body: if_body });
                            if !else_body.is_empty() {
                                out.push(JsStmt::If {
                                    cond: negate(inv),
                                    body: else_body,
                                });
                            }
                        } else {
                            out.push(JsStmt::If { cond: inv, body });
                        }
                    }
                    // pos is now at t; outer while picks up from there
                }
            }

            JsStmt::Switch { expr, cases } => {
                let mut target_to_keys: std::collections::BTreeMap<i32, Vec<i32>> =
                    std::collections::BTreeMap::new();
                let mut key_order: Vec<i32> = Vec::new();

                for (key, body) in cases.iter() {
                    if let Some(JsStmt::Goto(t)) = body.first() {
                        let entry = target_to_keys.entry(*t).or_insert_with(Vec::new);
                        if entry.is_empty() {
                            key_order.push(*t); // track insertion order
                        }
                        entry.push(*key);
                    }
                }
                
                let all_targets: Vec<i32> = {
                    let mut v: Vec<i32> = key_order.clone();
                    v.sort();
                    v
                };

                let mut target_stmts: HashMap<i32, Vec<JsStmt>> =
                    HashMap::new();
                
                let next_switch_off = stmts.iter()
                    .filter(|(o, s)| {
                        *o > off && matches!(s, JsStmt::Switch { .. })
                    })
                    .map(|(o, _)| *o)
                    .min();

                let effective_until = {
                    let mut counts: HashMap<i32, usize> = HashMap::new();
                    for (_, body) in cases.iter() {
                        if let Some(JsStmt::Goto(t)) = body.first() {
                            *counts.entry(*t).or_insert(0) += 1;
                        }
                    }

                    counts.iter()
                        .filter(|(_, &c)| c >= 2)
                        .map(|(&t, _)| t)
                        .min()
                        .or_else(|| counts.keys().copied().min())
                        .unwrap_or(until)
                        .min(until)
                };

                for (i, &t) in all_targets.iter().enumerate() {
                    let target_idx = o2i.get(&t).copied().unwrap_or(*pos);

                    let next_target = all_targets.get(i + 1).copied().unwrap_or(effective_until);
                    let stop = match next_switch_off {
                        Some(ns) if ns < next_target && !all_targets.contains(&ns) => ns,
                        _ => next_target,
                    };
                    let mut case_body = Vec::new();
                    let mut tmp_pos = target_idx;
                    structure_emit(stmts, o2i, loop_headers, &mut tmp_pos,
                                   stop, loop_exit, &mut case_body, depth + 1, current_header);
                    
                    if let Some(throw_pos) = case_body.iter().position(|s| {
                        matches!(s, JsStmt::Expr(JsExpr::Raw(r)) if r.starts_with("throw"))
                    }) {
                        case_body.truncate(throw_pos + 1);
                    }

                    if let Some(JsStmt::Comment(c)) = case_body.last() {
                        if c.starts_with("// goto →") {
                            case_body.pop();
                        }
                    }

                    target_stmts.insert(t, case_body);
                }
                
                let mut resolved: Vec<(i32, Vec<JsStmt>)> = Vec::new();
                for t in &key_order {
                    let keys = &target_to_keys[t];
                    let body = target_stmts.get(t).cloned().unwrap_or_default();
                    for &k in keys {
                        resolved.push((k, body.clone()));
                    }
                }
                resolved.sort_by_key(|(k, _)| *k);

                out.push(JsStmt::Switch { expr: expr.clone(), cases: resolved });
                try_merge_switches(out);
                
                if let Some(ns) = next_switch_off {
                    if let Some(&idx) = o2i.get(&ns) {
                        if idx > *pos { *pos = idx; }
                    }
                }
                eprintln!("Switch targets: {:?}, next_switch_off: {:?}, until: {}",
                          all_targets, next_switch_off, until);
            }

            other => { out.push(other.clone()); }
        }
    }
}

fn find_loop_end(stmts: &[(i32, JsStmt)], start_idx: usize, header: i32) -> i32 {
    for i in start_idx..stmts.len() {
        let (_, stmt) = &stmts[i];
        let target = match stmt {
            JsStmt::Goto(t)                    => Some(*t),
            JsStmt::CondGoto { target: t, .. } => Some(*t),
            _ => None,
        };
        if let Some(t) = target {
            if t <= header && t >= header - 4 {
                let end = stmts.get(i + 1).map(|(o, _)| *o).unwrap_or(i32::MAX);
                return end;
            }
        }
    }
    i32::MAX
}

fn negate(cond: JsExpr) -> JsExpr {
    match cond {
        JsExpr::BinOp { op, left, right } => {
            let flipped = match op {
                "==" => "!=",  "!=" => "==",
                "==="=> "!==", "!=="=> "===",
                "<"  => ">=",  ">=" => "<",
                ">"  => "<=",  "<=" => ">",
                _ => return JsExpr::UnaryOp {
                    op: "!",
                    expr: Box::new(JsExpr::BinOp { op, left, right }),
                },
            };
            JsExpr::BinOp { op: flipped, left, right }
        }
        other => JsExpr::UnaryOp { op: "!", expr: Box::new(other) },
    }
}

pub fn expr_to_js(expr: &JsExpr) -> String {
    match expr {
        JsExpr::Null        => "null".into(),
        JsExpr::Bool(b)     => b.to_string(),
        JsExpr::Int(n)      => n.to_string(),
        JsExpr::Float(f)    => f.to_string(),
        JsExpr::Str(s)      => format!("\"{}\"", s.replace('"', "\\\"")),
        JsExpr::Reg(r)      => format!("v{}", r),
        JsExpr::This        => "this".into(),
        JsExpr::Raw(s)      => s.clone(),

        JsExpr::MethodCall { receiver, method, args } => {
            let r = expr_to_js(receiver);
            let a = args.iter().map(expr_to_js).collect::<Vec<_>>().join(", ");
            format!("{}.{}({})", r, method, a)
        }
        JsExpr::StaticCall { class, method, args } => {
            let a = args.iter().map(expr_to_js).collect::<Vec<_>>().join(", ");
            format!("{}.{}({})", class, method, a)
        }
        JsExpr::New { class, args } => {
            let a = args.iter().map(expr_to_js).collect::<Vec<_>>().join(", ");
            format!("new {}({})", class, a)
        }
        JsExpr::FieldGet { receiver, field } => {
            format!("{}.{}", expr_to_js(receiver), field)
        }
        JsExpr::BinOp { op, left, right } => {
            let l = expr_to_js(left);
            let r = expr_to_js(right);
            if r.is_empty() { format!("({} {})", l, op) }
            else            { format!("({} {} {})", l, op, r) }
        }
        JsExpr::UnaryOp { op, expr } => {
            format!("({}{})", op, expr_to_js(expr))
        }
        JsExpr::Index { arr, idx } => {
            format!("{}[{}]", expr_to_js(arr), expr_to_js(idx))
        }
    }
}