use std::collections::HashMap;
use crate::translator::dalvik::insn::{DecodedInsn, Insn};
use crate::translator::dalvik::interpreter::ctx::LiftCtx;
use crate::translator::dalvik::interpreter::{reloop, ssa, JsExpr, JsStmt, RegId, TaggedStmt};
use crate::translator::dalvik::interpreter::cleanup::{cleanup};
use crate::translator::resolver::pool::Pool;

pub fn lift(
    insns:          &[Insn],
    insns_raw:      &[DecodedInsn],
    method_name:    &str,
    current_class:  &str,
    registers_size: u16,
    params_size:    u16,
    is_static:      bool,
    dex_shard:      usize,
    pool: &Pool,
) -> (Vec<JsStmt>, Vec<String>) {
    let this_reg = if is_static { None } else { Some((registers_size - params_size) as u8) };

    let first_param = (registers_size - params_size) as u8;

    let mut ctx = LiftCtx {
        regs:         HashMap::new(),
        known_constants: HashMap::new(),
        tagged:       Vec::new(),
        warnings:     Vec::new(),
        result:       None,
        pending_call: None,
        pending_new:  HashMap::new(),
        method_name:  method_name.to_string(),
        current_class: current_class.to_string(),
        this_reg,
        dex_shard,
        pool,
    };

    if let Some(tr) = this_reg {
        ctx.regs.insert(tr, JsExpr::This);
    }

    let mut arg_idx = 0usize;
    for i in 0..params_size {
        let r = first_param + i as u8;
        if Some(r) == ctx.this_reg { continue; }
        ctx.regs.insert(r, JsExpr::Reg(RegId { reg: r, version: 0 }));
        ctx.tagged.push(TaggedStmt {
            offset: -(arg_idx as i32 + 1),
            stmt: JsStmt::Assign { reg: RegId { reg: r, version: 0 }, expr: JsExpr::Raw(format!("arguments[{}]", arg_idx)) },
        });
        arg_idx += 1;
    }

    let mut offset = 0i32;
    for (idx, insn) in insns.iter().enumerate() {
        let is_branch = matches!(insn,
            Insn::Goto(_) | Insn::Goto16(_) | Insn::Goto32(_)
            | Insn::IfEqz(..) | Insn::IfNez(..)
            | Insn::IfEq(..) | Insn::IfNe(..)
            | Insn::IfLt(..) | Insn::IfGt(..)
            | Insn::IfLe(..) | Insn::IfGe(..)
            | Insn::IfLtz(..) | Insn::IfGtz(..)
            | Insn::IfLez(..) | Insn::IfGez(..)
            | Insn::PackedSwitch { .. } | Insn::SparseSwitch { .. }
        );
        if is_branch {
            let is_backward = matches!(insn, Insn::Goto(rel) if (*rel as i32) < 0)
                || matches!(insn, Insn::Goto16(rel) if (*rel as i32) < 0)
                || matches!(insn, Insn::Goto32(rel) if *rel < 0);
            if is_backward {
                ctx.pending_new.clear();
            }
        }

        ctx.process(insn, offset);
        offset += insns_raw.get(idx).map(|w| w.len as i32).unwrap_or(1);
    }
    ctx.flush_pending_call(offset);
    let tagged = ctx.tagged;
    let stmts  = reloop::structure_cfg(tagged);
    let stmts  = ssa::rename(stmts);
    let stmts = cleanup(stmts);
    (stmts, ctx.warnings)
}