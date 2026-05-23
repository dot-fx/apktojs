pub mod insn;
pub mod helpers;
pub mod interpreter;

use crate::translator::dalvik::helpers::{decode_invoke35, decode_invoke3rc, decode_invoke_args, parse_packed_switch, parse_sparse_switch, sign_extend_4, sign_extend_8, three_regs};
use crate::translator::dalvik::insn::{DecodedInsn, Insn};

pub fn decode(code: &[u16]) -> Vec<DecodedInsn> {
    let mut insns = Vec::new();
    let mut pc = 0usize;

    while pc < code.len() {
        let word = code[pc];
        let op   = (word & 0xFF) as u8;
        let hi   = ((word >> 8) & 0xFF) as u8;

        let start_pc = pc as u32;
        macro_rules! next {
            () => {{ pc += 1; if pc < code.len() { code[pc] } else { 0 } }};
        }

        let insn = match op {
            0x00 => {
                match hi {
                    0x01 => {
                        // packed-switch-payload — skip it entirely
                        // word[1] = size, word[2..3] = first_key, word[4..4+size*2] = targets
                        let size = next!() as usize;
                        next!(); next!(); // first_key (i32)
                        for _ in 0..size { next!(); next!(); } // targets (i32 each)
                        pc += 1;
                        Insn::Nop
                    }
                    0x02 => {
                        // sparse-switch-payload
                        let size = next!() as usize;
                        for _ in 0..size * 2 { next!(); next!(); } // keys[] then targets[], each i32
                        pc += 1;
                        Insn::Nop
                    }
                    0x03 => {
                        // fill-array-data-payload
                        let element_width = next!() as usize;
                        let size_lo = next!() as usize;
                        let size_hi = next!() as usize;
                        let size = (size_hi << 16) | size_lo;
                        let data_words = (size * element_width + 1) / 2;
                        for _ in 0..data_words { next!(); }
                        pc += 1;
                        Insn::Nop
                    }
                    _ => { pc += 1; Insn::Nop }
                }
            }

            // move vA, vB  (4-bit regs, hi encodes both)
            0x01 => { let i = Insn::Move   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x02 => { let w = next!(); let i = Insn::Move   (hi, (w & 0xFF) as u8); pc += 1; i }
            0x03 => { let w1 = next!(); let w2 = next!(); let i = Insn::Move((w1 & 0xFF) as u8, (w2 & 0xFF) as u8); pc += 1; i }
            0x04 => { let i = Insn::MoveWide   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x05 => { let w = next!(); let i = Insn::MoveWide   (hi, (w & 0xFF) as u8); pc += 1; i }
            0x06 => { let w = next!(); let i = Insn::MoveWide(hi, (w & 0xFF) as u8); pc += 1; i }
            0x07 => { let i = Insn::MoveObject (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x08 => { let w = next!(); let i = Insn::MoveObject (hi, (w & 0xFF) as u8); pc += 1; i }
            0x09 => {
                let dst = next!();
                let src = next!();
                let i = Insn::MoveObject(dst as u8, src as u8);
                pc += 2;
                i
            }

            0x0A => { pc += 1; Insn::MoveResult(hi) }
            0x0B => { pc += 1; Insn::MoveResultWide(hi) }
            0x0C => { pc += 1; Insn::MoveResultObject(hi) }
            0x0D => { pc += 1; Insn::MoveException(hi) }

            0x0E => { pc += 1; Insn::Return(0) }   // return-void
            0x0F => { pc += 1; Insn::Return(hi) }
            0x10 => { pc += 1; Insn::ReturnWide(hi) }
            0x11 => { pc += 1; Insn::ReturnObject(hi) }

            // const/4  vA, #+B
            0x12 => { let i = Insn::Const4(hi & 0xF, sign_extend_4((hi >> 4) & 0xF)); pc += 1; i }
            0x13 => { let w = next!() as i16; let i = Insn::Const16(hi, w); pc += 1; i }
            0x14 => {
                let lo = next!();
                let hi2 = next!();
                let v = ((hi2 as i32) << 16) | (lo as i32);
                let i = Insn::Const(hi, v);
                pc += 1; i
            }
            0x15 => { let w = next!(); let i = Insn::ConstHigh16(hi, (w as i32) << 16); pc += 1; i }
            0x16 => { let w = next!() as i16; let i = Insn::ConstWide16(hi, w); pc += 1; i }
            0x17 => {
                let lo = next!();
                let hi2 = next!();
                let v = ((hi2 as i32) << 16) | (lo as i32);
                let i = Insn::ConstWide32(hi, v);
                pc += 1; i
            }
            0x18 => {
                let w0 = next!() as u64;
                let w1 = next!() as u64;
                let w2 = next!() as u64;
                let w3 = next!() as u64;
                let v = (w3 << 48) | (w2 << 32) | (w1 << 16) | w0;
                let i = Insn::ConstWide(hi, v as i64);
                pc += 1; i
            }
            0x19 => { let w = next!(); let i = Insn::ConstWideHigh16(hi, (w as i64) << 48); pc += 1; i }
            0x1A => { let w = next!() as u32; let i = Insn::ConstString(hi, w); pc += 1; i }
            0x1B => {
                let lo = next!() as u32;
                let hi2 = next!() as u32;
                let idx = (hi2 << 16) | lo;
                let i = Insn::ConstStringJumbo(hi, idx);
                pc += 1; i
            }
            0x1C => { let w = next!() as u32; let i = Insn::ConstClass(hi, w); pc += 1; i }

            // monitor
            0x1D => { pc += 1; Insn::Monitor(hi, true) }
            0x1E => { pc += 1; Insn::Monitor(hi, false) }

            // check-cast vAA, type@BBBB
            0x1F => { let w = next!() as u32; let i = Insn::CheckCast(hi, w); pc += 1; i }
            0x20 => {
                let w = next!() as u32;
                let i = Insn::InstanceOf(hi & 0xF, (hi >> 4) & 0xF, w);
                pc += 1; i
            }
            0x21 => { let i = Insn::ArrayLength(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }

            // new-instance vAA, type@BBBB
            0x22 => { let w = next!() as u32; let i = Insn::NewInstance(hi, w); pc += 1; i }
            0x23 => {
                let w = next!() as u32;
                let i = Insn::NewArray(hi & 0xF, (hi >> 4) & 0xF, w);
                pc += 1; i
            }

            // filled-new-array {vC..vG}, type@BBBB
            0x24 => {
                let type_word = next!() as u32;
                let regs_word = next!();
                let count = (hi >> 4) & 0xF;
                let reg_d = (hi & 0xF) as u8;
                let args = decode_invoke_args(regs_word, reg_d, count);
                pc += 1;
                Insn::FilledNewArray { args, type_idx: type_word }
            }

            // filled-new-array/range {vCCCC .. vNNNN}, type@BBBB
            0x25 => {
                let type_idx = next!() as u32;
                let first = next!();
                let count = hi;

                pc += 1;

                Insn::FilledNewArrayRange {
                    first,
                    count,
                    type_idx,
                }
            }

            // fill-array-data vAA, +BBBBBBBB
            0x26 => {
                let lo = next!() as i32;
                let hi2 = next!() as i32;
                let offset = (hi2 << 16) | lo;
                pc += 1;
                Insn::FillArrayData(hi, offset)
            }

            0x27 => { pc += 1; Insn::Throw(hi) }

            // goto
            0x28 => {
                let off = sign_extend_8(hi);
                pc += 1;
                Insn::Goto(off)
            }

            0x29 => {
                let w = next!() as i16;
                pc += 1;
                Insn::Goto16(w)
            }

            0x2A => {
                let lo = next!() as i32;
                let hi2 = next!() as i32;
                let off = (hi2 << 16) | lo;
                pc += 1;
                Insn::Goto32(off)
            }

            // packed-switch vAA, +BBBBBBBB
            0x2B => {
                let lo = next!() as i32;
                let hi2 = next!() as i32;
                let table_rel = (hi2 << 16) | lo;
                pc += 1;
                let insn_pc = (pc - 3) as i32;
                let table_pc = (insn_pc + table_rel) as usize;
                let (first_key, targets) = parse_packed_switch(code, table_pc, insn_pc);
                Insn::PackedSwitch { reg: hi, first_key, targets }
            }
            // sparse-switch vAA, +BBBBBBBB
            0x2C => {
                let lo = next!() as i32;
                let hi2 = next!() as i32;
                let table_rel = (hi2 << 16) | lo;
                pc += 1;
                let insn_pc = (pc - 3) as i32;
                let table_pc = (insn_pc + table_rel) as usize;
                let (keys, targets) = parse_sparse_switch(code, table_pc, insn_pc);
                Insn::SparseSwitch { reg: hi, keys, targets }
            }

            // cmp
            0x2D => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::CmplFloat(a,b,c)); pc += 1; i }
            0x2E => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::CmpgFloat(a,b,c)); pc += 1; i }
            0x2F => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::CmplDouble(a,b,c)); pc += 1; i }
            0x30 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::CmpgDouble(a,b,c)); pc += 1; i }
            0x31 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::CmpLong(a,b,c)); pc += 1; i }

            // if-test vA, vB, +CCCC
            0x32 => { let w = next!() as i32; let i = Insn::IfEq (hi & 0xF, (hi>>4)&0xF, w as i16 as i32); pc += 1; i }
            0x33 => { let w = next!() as i32; let i = Insn::IfNe (hi & 0xF, (hi>>4)&0xF, w as i16 as i32); pc += 1; i }
            0x34 => { let w = next!() as i32; let i = Insn::IfLt(hi&0xF,(hi>>4)&0xF, w as i16 as i32); pc += 1; i }
            0x35 => { let w = next!() as i32; let i = Insn::IfGe (hi & 0xF, (hi>>4)&0xF, w as i16 as i32); pc += 1; i } // if-ge
            0x36 => { let w = next!() as i32; let i = Insn::IfGt (hi & 0xF, (hi>>4)&0xF, w as i16 as i32); pc += 1; i } // if-gt
            0x37 => { let w = next!() as i32; let i = Insn::IfLe (hi & 0xF, (hi>>4)&0xF, w as i16 as i32); pc += 1; i } // if-le

            // if-testz vAA, +BBBB
            0x38 => { let w = next!() as i32; let i = Insn::IfEqz(hi, w as i16 as i32); pc += 1; i }
            0x39 => { let w = next!() as i32; let i = Insn::IfNez(hi, w as i16 as i32); pc += 1; i }
            0x3A => { let w = next!() as i32; let i = Insn::IfLtz(hi, w as i16 as i32); pc += 1; i }
            0x3B => { let w = next!() as i32; let i = Insn::IfGez(hi, w as i16 as i32); pc += 1; i } // if-gez (single)
            0x3C => { let w = next!() as i32; let i = Insn::IfGtz(hi, w as i16 as i32); pc += 1; i }
            0x3D => { let w = next!() as i32; let i = Insn::IfLez(hi, w as i16 as i32); pc += 1; i }

            // aget vAA, vBB, vCC
            0x41 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::APut(a,b,c)); pc += 1; i }
            0x44 => { let w = next!(); let i = three_regs(hi,w,Insn::AGet); pc += 1; i }
            0x45 => { let w = next!(); let i = three_regs(hi,w,Insn::AGetWide); pc += 1; i }
            0x46 => { let w = next!(); let i = three_regs(hi,w,Insn::AGetObject); pc += 1; i }
            0x47 => { let w = next!(); let i = three_regs(hi,w,Insn::AGetBoolean); pc += 1; i }
            0x48 => { let w = next!(); let i = three_regs(hi,w,Insn::AGetByte); pc += 1; i }
            0x49 => { let w = next!(); let i = three_regs(hi,w,Insn::AGetChar); pc += 1; i }
            0x4A => { let w = next!(); let i = three_regs(hi,w,Insn::AGetShort); pc += 1; i }

            0x4B => { let w = next!(); let i = three_regs(hi,w,Insn::APut); pc += 1; i }
            0x4C => { let w = next!(); let i = three_regs(hi,w,Insn::APutWide); pc += 1; i }
            0x4D => { let w = next!(); let i = three_regs(hi,w,Insn::APutObject); pc += 1; i }
            0x4E => { let w = next!(); let i = three_regs(hi,w,Insn::APutBoolean); pc += 1; i }
            0x4F => { let w = next!(); let i = three_regs(hi,w,Insn::APutByte); pc += 1; i }
            0x50 => { let w = next!(); let i = three_regs(hi,w,Insn::APutChar); pc += 1; i }
            0x51 => { let w = next!(); let i = three_regs(hi,w,Insn::APutShort); pc += 1; i }

            // iget
            0x52 => { let idx = next!() as u32; let i = Insn::IGet(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }
            0x53 => { let idx = next!() as u32; let i = Insn::IGetWide(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }
            0x54 => { let idx = next!() as u32; let i = Insn::IGetObject(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }
            0x55 => { let idx = next!() as u32; let i = Insn::IGetBoolean(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }

            0x56 => { let idx = next!() as u32; let i = Insn::IGetBoolean(hi&0xF,(hi>>4)&0xF,idx); pc += 1; i } // iget-byte
            0x57 => { let idx = next!() as u32; let i = Insn::IGetBoolean(hi&0xF,(hi>>4)&0xF,idx); pc += 1; i } // iget-char
            0x58 => { let idx = next!() as u32; let i = Insn::IGetBoolean(hi&0xF,(hi>>4)&0xF,idx); pc += 1; i } // iget-short

            // iput
            0x59 => { let idx = next!() as u32; let i = Insn::IPut       (hi&0xF,(hi>>4)&0xF,idx); pc += 1; i }
            0x5A => { let idx = next!() as u32; let i = Insn::IPutWide   (hi&0xF,(hi>>4)&0xF,idx); pc += 1; i }
            0x5B => { let idx = next!() as u32; let i = Insn::IPutObject (hi&0xF,(hi>>4)&0xF,idx); pc += 1; i }
            0x5C => { let idx = next!() as u32; let i = Insn::IPutBoolean(hi&0xF,(hi>>4)&0xF,idx); pc += 1; i }
            0x5D => { let idx = next!() as u32; let i = Insn::IPutBoolean(hi&0xF,(hi>>4)&0xF,idx); pc += 1; i } // iput-byte
            0x5E => { let idx = next!() as u32; let i = Insn::IPutBoolean(hi&0xF,(hi>>4)&0xF,idx); pc += 1; i } // iput-char
            0x5F => { let idx = next!() as u32; let i = Insn::IPutBoolean(hi&0xF,(hi>>4)&0xF,idx); pc += 1; i } // iput-short

            // sget
            0x60 => { let w = next!() as u16 as u32; let i = Insn::SGet(hi, w); pc += 1; i }
            0x61 => { let w = next!() as u16 as u32; let i = Insn::SGet(hi, w); pc += 1; i }
            0x62 => { let w = next!() as u16 as u32; let i = Insn::SGetObject(hi, w); pc += 1; i }
            0x63 => { let w = next!() as u16 as u32; let i = Insn::SGetBoolean(hi, w); pc += 1; i }
            0x64 => { let w = next!() as u32; let i = Insn::SGetBoolean(hi, w); pc += 1; i } // sget-byte
            0x65 => { let w = next!() as u32; let i = Insn::SGetBoolean(hi, w); pc += 1; i } // sget-char
            0x66 => { let w = next!() as u32; let i = Insn::SGetBoolean(hi, w); pc += 1; i } // sget-short
            0x68 => { let w = next!() as u32; let i = Insn::SPut(hi, w);        pc += 1; i } // sput-wide
            0x6A => {
                let w = next!() as u32;
                let i = Insn::SPutBoolean(hi, w);
                pc += 1;
                i
            }

            0x6B => {
                let w = next!() as u32;
                let i = Insn::SPutByte(hi, w);
                pc += 1;
                i
            }

            0x6C => {
                let w = next!() as u32;
                let i = Insn::SPutChar(hi, w);
                pc += 1;
                i
            }

            0x6D => {
                let w = next!() as u32;
                let i = Insn::SPutShort(hi, w);
                pc += 1;
                i
            }
            // sput
            0x67 => { let w = next!() as u16 as u32; let i = Insn::SPut(hi, w); pc += 1; i }
            0x69 => { let w = next!() as u16 as u32; let i = Insn::SPutObject(hi, w); pc += 1; i }

            // invoke-virtual {vC..vG}, meth@BBBB
            0x6E => { let (args, idx) = decode_invoke35(hi, &code[pc+1..]); pc += 3; Insn::InvokeVirtual { args, method_idx: idx } }
            0x6F => { let (args, idx) = decode_invoke35(hi, &code[pc+1..]); pc += 3; Insn::InvokeSuper   { args, method_idx: idx } }
            0x70 => { let (args, idx) = decode_invoke35(hi, &code[pc+1..]); pc += 3; Insn::InvokeDirect  { args, method_idx: idx } }
            0x71 => { let (args, idx) = decode_invoke35(hi, &code[pc+1..]); pc += 3; Insn::InvokeStatic  { args, method_idx: idx } }
            0x72 => { let (args, idx) = decode_invoke35(hi, &code[pc+1..]); pc += 3; Insn::InvokeInterface{ args, method_idx: idx } }

            // invoke-virtual/range {vCCCC..vNNNN}, meth@BBBB
            0x74 => { let (first, count, idx) = decode_invoke3rc(hi, &code[pc+1..]); pc += 3; Insn::InvokeVirtualRange { first, count, method_idx: idx } }
            0x76 => { let (first, count, idx) = decode_invoke3rc(hi, &code[pc+1..]); pc += 3; Insn::InvokeDirectRange  { first, count, method_idx: idx } }
            0x77 => { let (first, count, idx) = decode_invoke3rc(hi, &code[pc+1..]); pc += 3; Insn::InvokeStaticRange  { first, count, method_idx: idx } }
            0x78 => { let (first, count, idx) = decode_invoke3rc(hi, &code[pc+1..]); pc += 3; Insn::InvokeVirtualRange { first, count, method_idx: idx } } // invoke-interface/range

            // unary ops (12x format: vA, vB)
            0x7B => { let i = Insn::NegInt      (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x7C => { let i = Insn::NotInt      (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x7D => { let i = Insn::NegLong      (hi&0xF,(hi>>4)&0xF); pc+=1; i } // neg-long
            0x7E => { let i = Insn::NotLong      (hi&0xF,(hi>>4)&0xF); pc+=1; i } // not-long
            0x7F => { let i = Insn::NegFloat      (hi&0xF,(hi>>4)&0xF); pc+=1; i } // neg-float
            0x80 => { let i = Insn::NegDouble      (hi&0xF,(hi>>4)&0xF); pc+=1; i } // neg-double
            0x81 => { let i = Insn::IntToLong   (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x82 => { let i = Insn::IntToFloat  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x83 => { let i = Insn::IntToDouble (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x84 => { let i = Insn::LongToInt   (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x85 => { let i = Insn::LongToFloat (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x86 => { let i = Insn::LongToDouble(hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x87 => { let i = Insn::FloatToInt  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x88 => { let i = Insn::FloatToLong  (hi&0xF,(hi>>4)&0xF); pc+=1; i } // float-to-long
            0x89 => { let i = Insn::FloatToDouble(hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x8A => { let i = Insn::DoubleToInt (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x8B => { let i = Insn::DoubleToLong (hi&0xF,(hi>>4)&0xF); pc+=1; i } // double-to-long
            0x8C => { let i = Insn::DoubleToFloat (hi&0xF,(hi>>4)&0xF); pc+=1; i } // double-to-float
            0x8D => { let i = Insn::IntToByte   (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x8E => { let i = Insn::IntToChar   (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0x8F => { let i = Insn::IntToShort  (hi&0xF,(hi>>4)&0xF); pc+=1; i }

            // binary ops 23x (dst, src1, src2 all 8-bit)
            0x90 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AddInt(a,b,c)); pc += 1; i }
            0x91 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::SubInt(a,b,c)); pc += 1; i }
            0x92 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::MulInt(a,b,c)); pc += 1; i }
            0x93 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::DivInt(a,b,c)); pc += 1; i }
            0x94 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::RemInt(a,b,c)); pc += 1; i }
            0x95 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AndInt(a,b,c)); pc += 1; i }
            0x96 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::OrInt(a,b,c)); pc += 1; i }
            0x97 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::XorInt(a,b,c)); pc += 1; i }
            0x98 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::ShlInt(a,b,c)); pc += 1; i }
            0x99 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::ShrInt(a,b,c)); pc += 1; i }
            0x9A => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::UshrInt(a,b,c)); pc += 1; i }
            0x9B => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AddLong(a,b,c)); pc += 1; i }
            0x9C => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::SubLong(a,b,c)); pc += 1; i }
            0x9D => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::MulLong(a,b,c)); pc += 1; i }
            0x9E => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::DivLong(a,b,c)); pc += 1; i }
            0x9F => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::RemLong(a,b,c));  pc+=1; i } // rem-long (approx)
            0xA8 => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::AndLong(a,b,c));  pc+=1; i } // and-long
            0xA9 => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::OrLong(a,b,c));   pc+=1; i } // or-long
            0xAA => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::XorLong(a,b,c));  pc+=1; i } // xor-long
            0xAB => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::ShlLong(a,b,c));  pc+=1; i } // shl-long
            0xAC => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::ShrLong(a,b,c));  pc+=1; i } // shr-long
            0xAD => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::UshrLong(a,b,c)); pc+=1; i } // ushr-long
            0xA0 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AddFloat(a,b,c)); pc += 1; i }
            0xA1 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::SubFloat(a,b,c)); pc += 1; i }
            0xA2 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::MulFloat(a,b,c)); pc += 1; i }
            0xA3 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::DivFloat(a,b,c)); pc += 1; i }
            0xA4 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AddDouble(a,b,c)); pc += 1; i }
            0xA5 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::SubDouble(a,b,c)); pc += 1; i }
            0xA6 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::MulDouble(a,b,c)); pc += 1; i }
            0xA7 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::DivDouble(a,b,c)); pc += 1; i }
            0xAE => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::RemFloat(a,b,c));  pc+=1; i }
            0xAF => { let w = next!(); let i = three_regs(hi,w,|a,b,c| Insn::RemDouble(a,b,c)); pc+=1; i }

            // 2addr (12x format: dst/src1 = vA, src2 = vB)
            0xB0 => { let i = Insn::AddInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB1 => { let i = Insn::SubInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB2 => { let i = Insn::MulInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB3 => { let i = Insn::DivInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB4 => { let i = Insn::RemInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB5 => { let i = Insn::AndInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB6 => { let i = Insn::OrInt2Addr (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB7 => { let i = Insn::XorInt2Addr   (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xB8 => { let i = Insn::ShlInt2Addr   (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xB9 => { let i = Insn::ShrInt2Addr   (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xBA => { let i = Insn::UshrInt2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xBB => { let i = Insn::AddLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xBC => { let i = Insn::SubLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xBD => { let i = Insn::MulLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xBE => { let i = Insn::DivLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xBF => { let i = Insn::RemLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }

            0xC0 => { let i = Insn::AndLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC1 => { let i = Insn::OrLong2Addr   (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC2 => { let i = Insn::XorLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC3 => { let i = Insn::ShlLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC4 => { let i = Insn::ShrLong2Addr  (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC5 => { let i = Insn::UshrLong2Addr (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC6 => { let i = Insn::AddFloat2Addr (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC7 => { let i = Insn::SubFloat2Addr (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC8 => { let i = Insn::MulFloat2Addr (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xC9 => { let i = Insn::DivFloat2Addr (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xCA => { let i = Insn::RemFloat2Addr (hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xCB => { let i = Insn::AddDouble2Addr(hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xCC => { let i = Insn::SubDouble2Addr(hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xCE => { let i = Insn::DivDouble2Addr(hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xCF => { let i = Insn::RemDouble2Addr(hi&0xF,(hi>>4)&0xF); pc+=1; i }
            0xCD => { let i = Insn::MulLong2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }

            // lit16 / lit8
            0xD0 => { let w = next!() as i16; let i = Insn::AddIntLit16(hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD1 => { let w = next!() as i16; let i = Insn::RsubIntLit16(hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD2 => { let w = next!() as i16; let i = Insn::MulIntLit16(hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD3 => { let w = next!() as i16; let i = Insn::DivIntLit16 (hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD4 => { let w = next!() as i16; let i = Insn::AndIntLit16(hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD5 => { let w = next!() as i16; let i = Insn::RemIntLit16 (hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD6 => { let w = next!() as i16; let i = Insn::OrIntLit16 (hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD7 => { let w = next!() as i16; let i = Insn::XorIntLit16 (hi&0xF,(hi>>4)&0xF, w); pc += 1; i }

            0xD8 => { let w = next!(); let i = Insn::AddIntLit8 (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xD9 => { let w = next!(); let i = Insn::RsubIntLit8(hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xDA => { let w = next!(); let i = Insn::MulIntLit8 (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xDB => { let w = next!(); let i = Insn::DivIntLit8 (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xDC => { let w = next!(); let i = Insn::RemIntLit8 (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xDD => { let w = next!(); let i = Insn::AndIntLit8 (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xDE => { let w = next!(); let i = Insn::OrIntLit8  (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xDF => { let w = next!(); let i = Insn::XorIntLit8 (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xE0 => { let w = next!(); let i = Insn::ShlIntLit8 (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xE1 => { let w = next!(); let i = Insn::ShrIntLit8 (hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xE2 => { let w = next!(); let i = Insn::UshrIntLit8(hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }
            0xE8 => { let w = next!(); let i = Insn::ShrIntLit8(hi, (w & 0xFF) as u8, (w >> 8) as i8); pc += 1; i }

            0xF8 => { let i = Insn::LongToInt(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i } // ushr-long/2addr placeholder


            _ => { pc += 1; Insn::Unknown(word) }
        };

        let len = (pc as u32 - start_pc) as u8;

        insns.push(DecodedInsn {
            pc: start_pc,
            len,
            insn,
        });
    }

    insns
}