use std::collections::HashMap;
use crate::extensions::tachiyomi_loader::translator::dalvik::insn::Insn;
use crate::extensions::tachiyomi_loader::translator::dalvik::interpreter::{JsExpr, JsStmt, TaggedStmt};
use crate::extensions::tachiyomi_loader::translator::emit::render;

pub struct LiftCtx {
    pub regs:         HashMap<u8, JsExpr>,
    pub tagged:       Vec<TaggedStmt>,
    pub warnings:     Vec<String>,
    pub result:       Option<JsExpr>,
    pub pending_call: Option<(i32, JsExpr)>,
    pub pending_new:  HashMap<u8, String>,
    pub method_name:  String,
    pub this_reg:     Option<u8>,
    pub dex_shard:    usize,
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
        self.regs.insert(r, expr.clone());
        self.push(offset, JsStmt::Assign { reg: r, expr });
    }

    fn push(&mut self, offset: i32, stmt: JsStmt) {
        self.tagged.push(TaggedStmt { offset, stmt });
    }

    fn warn(&mut self, s: impl Into<String>) { self.warnings.push(s.into()); }

    pub fn flush_pending_call(&mut self, at_offset: i32) {
        if let Some((off, call)) = self.pending_call.take() {
            let suppress = matches!(&call,
                JsExpr::MethodCall { method, .. } if method == "getClass"
            );
            if !suppress {
                self.push(off, JsStmt::Expr(call));
            }
            let _ = at_offset;
        }
    }

    fn field_ref(&self, fi: u32) -> String {
        format!("_field{}_{}", self.dex_shard, fi)
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
                self.regs.insert(*d, e);
            }

            // Moves
            Insn::Move(d, s) | Insn::MoveWide(d, s) | Insn::MoveObject(d, s) => {
                let e = self.reg(*s);
                self.set(*d, e, off);
            }

            Insn::MoveResult(d) | Insn::MoveResultWide(d) | Insn::MoveResultObject(d) => {
                let e = self.result.take()
                    .unwrap_or(JsExpr::Raw("/* no-result */".into()));
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

            Insn::ConstString(d, idx) | Insn::ConstStringJumbo(d, idx) =>
                self.set(*d, JsExpr::Raw(format!("/* string#{} */", idx)), off),

            Insn::ConstClass(d, idx) =>
                self.set(*d, JsExpr::Raw(format!("/* class#{} */", idx)), off),

            Insn::ConstNull(d) =>
                self.set(*d, JsExpr::Null, off),

            // Returns
            Insn::Return(0) => self.push(off, JsStmt::Return(None)),
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
                self.set(*d, JsExpr::Raw(format!("/* static_field#{} */", fi)), off);
            }

            Insn::SPut(src, fi) | Insn::SPutObject(src, fi) => {
                let value = self.reg(*src);
                self.push(off, JsStmt::Comment(format!("/* sput #{}: {} */", fi, render::expr_to_js(&value))));
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
                        let ctor_args: Vec<JsExpr> = args.iter().skip(1)
                            .map(|r| self.reg(*r)).collect();
                        let expr = JsExpr::New { class: type_ph, args: ctor_args };
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
                let call = JsExpr::StaticCall {
                    class:  format!("/* static_meth{} */", method_idx),
                    method: String::new(),
                    args:   arg_exprs,
                };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeVirtualRange { first, count, method_idx } => {
                let args: Vec<JsExpr> = (*first..*first + *count).map(|r| self.reg(r)).collect();
                let recv      = args.first().cloned().unwrap_or(JsExpr::This);
                let call_args = if args.len() > 1 { args[1..].to_vec() } else { vec![] };
                let call = JsExpr::MethodCall {
                    receiver: Box::new(recv),
                    method:   format!("_meth{}", method_idx),
                    args:     call_args,
                };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeStaticRange { first, count, method_idx } => {
                let args: Vec<JsExpr> = (*first..*first + *count).map(|r| self.reg(r)).collect();
                let call = JsExpr::StaticCall {
                    class:  format!("/* static_meth{} */", method_idx),
                    method: String::new(),
                    args,
                };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::InvokeDirectRange { first, count, method_idx } => {
                let args: Vec<JsExpr> = (*first..*first + *count).map(|r| self.reg(r)).collect();
                let recv      = args.first().cloned().unwrap_or(JsExpr::This);
                let call_args = if args.len() > 1 { args[1..].to_vec() } else { vec![] };
                let call = JsExpr::MethodCall {
                    receiver: Box::new(recv),
                    method:   format!("_meth{}", method_idx),
                    args:     call_args,
                };
                self.result       = Some(call.clone());
                self.pending_call = Some((off, call));
            }

            Insn::NewInstance(d, type_idx) => {
                self.pending_new.insert(*d, format!("/* type#{} */", type_idx));
            }

            Insn::NewArray(d, len_reg, type_idx) => {
                let len = self.reg(*len_reg);
                self.set(*d, JsExpr::Raw(
                    format!("new Array({}) /* type#{} */", render::expr_to_js(&len), type_idx)
                ), off);
            }

            Insn::FilledNewArray { args, type_idx } => {
                let exprs: Vec<String> = args.iter().map(|r| render::expr_to_js(&self.reg(*r))).collect();
                self.result = Some(JsExpr::Raw(
                    format!("[{}] /* type#{} */", exprs.join(", "), type_idx)
                ));
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
                self.set(*d, JsExpr::Raw(
                    format!("({} instanceof /* type#{} */)", render::expr_to_js(&oe), type_idx)
                ), off);
            }

            // Branches
            Insn::Goto(rel)   => self.push(off, JsStmt::Goto(off + *rel as i32)),
            Insn::Goto16(rel) => self.push(off, JsStmt::Goto(off + *rel as i32)),
            Insn::Goto32(rel) => self.push(off, JsStmt::Goto(off + *rel)),

            Insn::IfEqz(r, rel) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::UnaryOp { op: "!", expr: Box::new(e) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfNez(r, rel) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::CondGoto { cond: e, target: off + *rel as i32 });
            }
            Insn::IfEq(a, b, rel) => {
                let ea = self.reg(*a); let eb = self.reg(*b);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp { op: "==", left: Box::new(ea), right: Box::new(eb) },
                    target: off + *rel as i32,
                });
            }
            Insn::IfNe(a, b, rel) => {
                let ea = self.reg(*a); let eb = self.reg(*b);
                self.push(off, JsStmt::CondGoto {
                    cond: JsExpr::BinOp { op: "!=", left: Box::new(ea), right: Box::new(eb) },
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
                let cases: Vec<(i32, Vec<JsStmt>)> = targets.iter().enumerate()
                    .map(|(i, &abs_target)| (*first_key + i as i32, vec![JsStmt::Goto(abs_target)]))
                    .collect();
                self.push(off, JsStmt::Switch { expr: e, cases });
            }

            Insn::SparseSwitch { reg, keys, targets } => {
                let e = self.reg(*reg);
                let cases: Vec<(i32, Vec<JsStmt>)> = keys.iter().zip(targets.iter())
                    .map(|(&key, &abs_target)| (key, vec![JsStmt::Goto(abs_target)]))
                    .collect();
                self.push(off, JsStmt::Switch { expr: e, cases });
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
                self.set(*d, JsExpr::Raw(format!("String.fromCharCode({})", render::expr_to_js(&e))), off);
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
                self.set(*d, JsExpr::Raw(
                    format!("Math.sign({} - {})", render::expr_to_js(&ae), render::expr_to_js(&be))
                ), off);
            }

            Insn::Throw(r) => {
                let e = self.reg(*r);
                self.push(off, JsStmt::Expr(JsExpr::Raw(format!("throw {}", render::expr_to_js(&e)))));
            }

            Insn::FillArrayData(arr, _) => {
                let ae = self.reg(*arr);
                self.push(off, JsStmt::Comment(
                    format!("// fill_array_data({}) /* TODO */", render::expr_to_js(&ae))
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
        let receiver  = if args.is_empty() { JsExpr::This } else { self.reg(args[0]) };
        let call_args = args.iter().skip(1).map(|r| self.reg(*r)).collect();
        JsExpr::MethodCall {
            receiver: Box::new(receiver),
            method:   format!("_meth{}", method_idx),
            args:     call_args,
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