#[derive(Debug, Clone)]
pub enum Insn {
    Nop,
    Return(u8),        // return-void / return vX
    ReturnWide(u8),
    ReturnObject(u8),
    Goto(i8),
    Goto16(i16),
    Goto32(i32),
    IfEq(u8, u8, i32),
    IfNe(u8, u8, i32),
    IfLt(u8, u8, i32),
    IfGe(u8, u8, i32),
    IfGt(u8, u8, i32),
    IfLe(u8, u8, i32),
    IfEqz(u8, i32),
    IfNez(u8, i32),
    IfLtz(u8, i32),
    IfGtz(u8, i32),
    IfLez(u8, i32),
    IfGez(u8, i32),
    PackedSwitch { reg: u8, first_key: i32, targets: Vec<i32> },  // targets = absolute offsets
    SparseSwitch { reg: u8, keys: Vec<i32>, targets: Vec<i32> },

    Move(u8, u8),          // dst, src
    MoveWide(u8, u8),
    MoveObject(u8, u8),
    MoveResult(u8),
    MoveResultWide(u8),
    MoveResultObject(u8),
    MoveException(u8),

    Const4(u8, i8),
    Const16(u8, i16),
    Const(u8, i32),
    ConstHigh16(u8, i32),
    ConstWide16(u8, i16),
    ConstWide32(u8, i32),
    ConstWide(u8, i64),
    ConstWideHigh16(u8, i64),
    ConstString(u8, u32),      // reg, string_index
    ConstStringJumbo(u8, u32),
    ConstClass(u8, u32),       // reg, type_index
    ConstNull(u8),             // synthesised from const v,0

    IGet(u8, u8, u32),         // dst, obj-reg, field_idx
    IGetWide(u8, u8, u32),
    IGetObject(u8, u8, u32),
    IGetBoolean(u8, u8, u32),
    IPut(u8, u8, u32),         // src, obj-reg, field_idx
    IPutWide(u8, u8, u32),
    IPutObject(u8, u8, u32),
    IPutBoolean(u8, u8, u32),
    SGet(u8, u32),             // dst, field_idx
    SGetObject(u8, u32),
    SGetBoolean(u8, u32),
    SPut(u8, u32),
    SPutObject(u8, u32),

    InvokeVirtual       { args: Vec<u8>, method_idx: u32 },
    InvokeSuper         { args: Vec<u8>, method_idx: u32 },
    InvokeDirect        { args: Vec<u8>, method_idx: u32 },
    InvokeStatic        { args: Vec<u8>, method_idx: u32 },
    InvokeInterface     { args: Vec<u8>, method_idx: u32 },
    InvokeVirtualRange  { first: u8, count: u8, method_idx: u32 },
    InvokeDirectRange   { first: u8, count: u8, method_idx: u32 },
    InvokeStaticRange   { first: u8, count: u8, method_idx: u32 },

    NewInstance(u8, u32),      // dst, type_idx
    NewArray(u8, u8, u32),     // dst, len-reg, type_idx
    FilledNewArray { args: Vec<u8>, type_idx: u32 },
    FillArrayData(u8, i32),    // reg, offset

    CheckCast(u8, u32),
    InstanceOf(u8, u8, u32),

    ArrayLength(u8, u8),
    AGet(u8, u8, u8),          // dst, array-reg, idx-reg
    AGetObject(u8, u8, u8),
    APut(u8, u8, u8),
    APutObject(u8, u8, u8),

    NegInt(u8, u8),
    NotInt(u8, u8),
    IntToLong(u8, u8),
    IntToFloat(u8, u8),
    IntToDouble(u8, u8),
    LongToInt(u8, u8),
    LongToFloat(u8, u8),
    LongToDouble(u8, u8),
    FloatToInt(u8, u8),
    FloatToDouble(u8, u8),
    DoubleToInt(u8, u8),
    IntToByte(u8, u8),
    IntToChar(u8, u8),
    IntToShort(u8, u8),

    AddInt(u8, u8, u8),
    SubInt(u8, u8, u8),
    MulInt(u8, u8, u8),
    DivInt(u8, u8, u8),
    RemInt(u8, u8, u8),
    AndInt(u8, u8, u8),
    OrInt(u8, u8, u8),
    XorInt(u8, u8, u8),
    ShlInt(u8, u8, u8),
    ShrInt(u8, u8, u8),
    UshrInt(u8, u8, u8),
    AddLong(u8, u8, u8),
    SubLong(u8, u8, u8),
    MulLong(u8, u8, u8),
    DivLong(u8, u8, u8),
    AddFloat(u8, u8, u8),
    SubFloat(u8, u8, u8),
    MulFloat(u8, u8, u8),
    DivFloat(u8, u8, u8),
    AddDouble(u8, u8, u8),
    SubDouble(u8, u8, u8),
    MulDouble(u8, u8, u8),
    DivDouble(u8, u8, u8),

    AddInt2Addr(u8, u8),
    SubInt2Addr(u8, u8),
    MulInt2Addr(u8, u8),
    DivInt2Addr(u8, u8),
    RemInt2Addr(u8, u8),
    AndInt2Addr(u8, u8),
    OrInt2Addr(u8, u8),
    AddLong2Addr(u8, u8),
    SubLong2Addr(u8, u8),
    MulLong2Addr(u8, u8),
    DivLong2Addr(u8, u8),
    AddIntLit16(u8, u8, i16),
    MulIntLit16(u8, u8, i16),
    AndIntLit16(u8, u8, i16),
    OrIntLit16(u8, u8, i16),
    AddIntLit8(u8, u8, i8),
    RsubIntLit8(u8, u8, i8),
    MulIntLit8(u8, u8, i8),
    DivIntLit8(u8, u8, i8),
    RemIntLit8(u8, u8, i8),
    AndIntLit8(u8, u8, i8),
    OrIntLit8(u8, u8, i8),
    XorIntLit8(u8, u8, i8),
    ShlIntLit8(u8, u8, i8),
    ShrIntLit8(u8, u8, i8),
    UshrIntLit8(u8, u8, i8),

    CmpLong(u8, u8, u8),
    CmplFloat(u8, u8, u8),
    CmpgFloat(u8, u8, u8),
    CmplDouble(u8, u8, u8),
    CmpgDouble(u8, u8, u8),

    Monitor(u8, bool),
    Throw(u8),

    Unknown(u16),
}

impl Insn {
    pub fn length_in_units(&self) -> i32 {
        match self {
            Insn::Nop
            | Insn::Return(_)
            | Insn::Move(_, _)
            | Insn::Const4(_, _)
            | Insn::Goto(_) => 1,

            Insn::Goto16(_)
            | Insn::IfEq(_, _, _)
            | Insn::Const16(_, _)
            | Insn::ConstString(_, _) => 2,

            Insn::Const(_, _)
            | Insn::FillArrayData(_, _)
            | Insn::Goto32(_) => 3,

            Insn::ConstWide(_, _) => 5,
            Insn::PackedSwitch { .. } | Insn::SparseSwitch { .. } => 3,

            _ => 2,
        }
    }
}

pub fn decode(code: &[u16]) -> Vec<Insn> {
    let mut insns = Vec::new();
    let mut pc = 0usize;

    while pc < code.len() {
        let word = code[pc];
        let op   = (word & 0xFF) as u8;
        let hi   = ((word >> 8) & 0xFF) as u8;

        macro_rules! next {
            () => {{ pc += 1; if pc < code.len() { code[pc] } else { 0 } }};
        }

        let insn = match op {
            0x00 => { pc += 1; Insn::Nop },

            // move vA, vB  (4-bit regs, hi encodes both)
            0x01 => { let i = Insn::Move   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x02 => { let w = next!(); let i = Insn::Move   (hi, (w & 0xFF) as u8); pc += 1; i }
            0x03 => { let w1 = next!(); let w2 = next!(); let i = Insn::Move((w1 & 0xFF) as u8, (w2 & 0xFF) as u8); pc += 1; i }
            0x04 => { let i = Insn::MoveWide   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x05 => { let w = next!(); let i = Insn::MoveWide   (hi, (w & 0xFF) as u8); pc += 1; i }
            0x06 => { let w = next!(); let i = Insn::MoveWide(hi, (w & 0xFF) as u8); pc += 1; i }
            0x07 => { let i = Insn::MoveObject (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x08 => { let w = next!(); let i = Insn::MoveObject (hi, (w & 0xFF) as u8); pc += 1; i }
            0x09 => { let dst = next!(); let src = next!(); let i = Insn::MoveWide((dst & 0xFF) as u8, (src & 0xFF) as u8); pc += 2; i }

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
                let table_rel = (hi2 << 16) | lo;  // relative to this instruction's pc
                pc += 1;
                // pc now points one past the 3-word instruction.
                // The switch instruction started at (pc - 3) in 0-based units.
                let insn_pc = (pc - 3) as i32;  // absolute pc of the packed-switch word
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
            0x34 => { let w = next!() as i32; Insn::IfLt(hi&0xF, (hi>>4)&0xF, w as i16 as i32) }
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
            0x44 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AGet(a,b,c)); pc += 1; i }
            0x46 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AGetObject(a,b,c)); pc += 1; i }
            0x47 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::APutObject(a,b,c)); pc += 1; i }
            // aput
            0x4B => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::APut(a,b,c)); pc += 1; i }
            0x4D => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::APutObject(a,b,c)); pc += 1; i }

            // iget
            0x52 => { let idx = next!() as u32; let i = Insn::IGet(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }
            0x53 => { let idx = next!() as u32; let i = Insn::IGetWide(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }
            0x54 => { let idx = next!() as u32; let i = Insn::IGetObject(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }
            0x55 => { let idx = next!() as u32; let i = Insn::IGetBoolean(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }

            // iput
            0x59 => { let idx = next!() as u32; let i = Insn::IPut(hi&0xF, (hi>>4)&0xF, idx); pc += 1; i }
            0x5A => { let w = next!(); let idx = next!() as u32; let i = Insn::IPutWide(hi&0xF,(hi>>4)&0xF,idx|((w as u32)<<16)); pc += 1; i }
            0x5B => { let w = next!(); let idx = next!() as u32; let i = Insn::IPutObject(hi&0xF,(hi>>4)&0xF,idx|((w as u32)<<16)); pc += 1; i }
            0x5C => { let w = next!(); let idx = next!() as u32; let i = Insn::IPutBoolean(hi&0xF,(hi>>4)&0xF,idx|((w as u32)<<16)); pc += 1; i }
            0x5F => { let w = next!(); let idx = next!() as u32; let i = Insn::IPutBoolean(hi&0xF,(hi>>4)&0xF,idx|((w as u32)<<16)); pc += 1; i }

            // sget
            0x60 => { let w = next!() as u16 as u32; let i = Insn::SGet(hi, w); pc += 1; i }
            0x61 => { let w = next!() as u16 as u32; let i = Insn::SGet(hi, w); pc += 1; i }
            0x62 => { let w = next!() as u16 as u32; let i = Insn::SGetObject(hi, w); pc += 1; i }
            0x63 => { let w = next!() as u16 as u32; let i = Insn::SGetBoolean(hi, w); pc += 1; i }
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
            0x78 => { let w = next!(); let idx = next!() as u32; let i = Insn::IPutWide(hi & 0xF, (hi>>4)&0xF, idx|((w as u32)<<16)); pc += 1; i }

            // unary ops (12x format: vA, vB)
            0x7B => { let i = Insn::NegInt      (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x7C => { let i = Insn::NotInt      (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x7D => { let i = Insn::IntToLong   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i } // neg-long — reuse IntToLong or add NegLong
            0x7E => { let i = Insn::IntToFloat  (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i } // not-long
            0x7F => { let i = Insn::IntToDouble (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i } // neg-float
            0x80 => { let i = Insn::LongToInt   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i } // neg-double
            0x81 => { let i = Insn::IntToLong   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x82 => { let i = Insn::IntToFloat  (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x83 => { let i = Insn::IntToDouble (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x84 => { let i = Insn::LongToInt   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x85 => { let i = Insn::LongToFloat (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x86 => { let i = Insn::LongToDouble(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x87 => { let i = Insn::FloatToInt  (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x88 => { let i = Insn::FloatToDouble(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i } // float-to-long
            0x89 => { let i = Insn::FloatToDouble(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x8A => { let i = Insn::DoubleToInt (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x8B => { let i = Insn::DoubleToInt (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i } // double-to-long
            0x8C => { let i = Insn::DoubleToInt (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i } // double-to-float
            0x8D => { let i = Insn::IntToByte   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x8E => { let i = Insn::IntToChar   (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0x8F => { let i = Insn::IntToShort  (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }

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
            0xA0 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AddFloat(a,b,c)); pc += 1; i }
            0xA1 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::SubFloat(a,b,c)); pc += 1; i }
            0xA2 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::MulFloat(a,b,c)); pc += 1; i }
            0xA3 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::DivFloat(a,b,c)); pc += 1; i }
            0xA4 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::AddDouble(a,b,c)); pc += 1; i }
            0xA5 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::SubDouble(a,b,c)); pc += 1; i }
            0xA6 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::MulDouble(a,b,c)); pc += 1; i }
            0xA7 => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::DivDouble(a,b,c)); pc += 1; i }

            // 2addr (12x format: dst/src1 = vA, src2 = vB)
            0xB0 => { let i = Insn::AddInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB1 => { let i = Insn::SubInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB2 => { let i = Insn::MulInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB3 => { let i = Insn::DivInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB4 => { let i = Insn::RemInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB5 => { let i = Insn::AndInt2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }
            0xB6 => { let i = Insn::OrInt2Addr (hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }

            0xCD => { let i = Insn::MulLong2Addr(hi & 0xF, (hi >> 4) & 0xF); pc += 1; i }

            // lit16 / lit8
            0xD0 => { let w = next!() as i16; let i = Insn::AddIntLit16(hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD2 => { let w = next!() as i16; let i = Insn::MulIntLit16(hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD4 => { let w = next!() as i16; let i = Insn::AndIntLit16(hi&0xF,(hi>>4)&0xF, w); pc += 1; i }
            0xD6 => { let w = next!() as i16; let i = Insn::OrIntLit16 (hi&0xF,(hi>>4)&0xF, w); pc += 1; i }

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
            0xBA => { let w = next!(); let i = three_regs(hi, w, |a,b,c| Insn::RemInt(a,b,c)); pc += 1; i } // add RemDouble to enum ideally


            _ => { pc += 1; Insn::Unknown(word) }
        };

        insns.push(insn);
    }

    insns
}


fn sign_extend_4(v: u8) -> i8 {
    if v & 0x8 != 0 { (v as i8) | (-16i8) } else { v as i8 }
}

fn sign_extend_8(v: u8) -> i8 {
    v as i8
}

fn three_regs<F: Fn(u8, u8, u8) -> Insn>(dst: u8, second_word: u16, f: F) -> Insn {
    let b = (second_word & 0xFF) as u8;
    let c = (second_word >> 8) as u8;
    f(dst, b, c)
}

fn decode_invoke35(count_and_d: u8, rest: &[u16]) -> (Vec<u8>, u32) {
    let method_idx = rest[0] as u32;
    let regs_word  = rest[1];
    let count = (count_and_d >> 4) & 0xF;
    let reg_d = count_and_d & 0xF;
    (decode_invoke_args(regs_word, reg_d, count), method_idx)
}

fn decode_invoke_args(regs_word: u16, reg_g: u8, count: u8) -> Vec<u8> {
    let d = (regs_word & 0xF) as u8;
    let e = ((regs_word >> 4) & 0xF) as u8;
    let f = ((regs_word >> 8) & 0xF) as u8;
    let g = ((regs_word >> 12) & 0xF) as u8;
    let a = reg_g; // 5th register is in high nibble of first word
    let all = [d, e, f, g, a];
    all[..count.min(5) as usize].to_vec()
}

fn decode_invoke3rc(hi: u8, rest: &[u16]) -> (u8, u8, u32) {
    if rest.len() < 2 { return (0, 0, 0); }
    let method_idx = rest[0] as u32;
    let first      = (rest[1] & 0xFF) as u8;
    (first, hi, method_idx)  // hi IS the count for /range opcodes
}

/// Parse a packed-switch table and return (first_key, absolute_targets).
/// `table_pc` is the absolute code-unit index of the table header.
/// `insn_pc`  is the absolute code-unit index of the packed-switch instruction.
fn parse_packed_switch(code: &[u16], table_pc: usize, insn_pc: i32) -> (i32, Vec<i32>) {
    // Table layout (all u16 words):
    //   [0x0100]          ident
    //   [size: u16]       number of entries
    //   [first_key lo]    \  i32 little-endian
    //   [first_key hi]    /
    //   [target0 lo]      \  i32 relative offsets, one per entry
    //   [target0 hi]      /
    //   ...
    if table_pc + 4 > code.len() { return (0, vec![]); }
    // word 0: ident 0x0100 — skip
    let size      = code[table_pc + 1] as usize;
    let fk_lo     = code[table_pc + 2] as i32;
    let fk_hi     = code[table_pc + 3] as i32;
    let first_key = (fk_hi << 16) | fk_lo;

    let data_start = table_pc + 4;
    let mut targets = Vec::with_capacity(size);
    for i in 0..size {
        let base = data_start + i * 2;
        if base + 1 >= code.len() { break; }
        let lo  = code[base]     as i32;
        let hi2 = code[base + 1] as i32;
        let rel = (hi2 << 16) | lo;
        targets.push(insn_pc + rel);  // convert to absolute
    }
    (first_key, targets)
}

/// Parse a sparse-switch table and return (keys, absolute_targets).
fn parse_sparse_switch(code: &[u16], table_pc: usize, insn_pc: i32) -> (Vec<i32>, Vec<i32>) {
    // Table layout:
    //   [0x0200]          ident
    //   [size: u16]       number of entries
    //   [key0 lo][key0 hi]   \  i32 keys, sorted
    //   ...
    //   [target0 lo][target0 hi]  \  i32 relative offsets
    //   ...
    if table_pc + 2 > code.len() { return (vec![], vec![]); }
    let size       = code[table_pc + 1] as usize;
    let keys_start = table_pc + 2;
    let tgts_start = keys_start + size * 2;

    let mut keys    = Vec::with_capacity(size);
    let mut targets = Vec::with_capacity(size);

    for i in 0..size {
        let kb = keys_start + i * 2;
        if kb + 1 >= code.len() { break; }
        let k = ((code[kb + 1] as i32) << 16) | (code[kb] as i32);
        keys.push(k);
    }
    for i in 0..size {
        let tb = tgts_start + i * 2;
        if tb + 1 >= code.len() { break; }
        let rel = ((code[tb + 1] as i32) << 16) | (code[tb] as i32);
        targets.push(insn_pc + rel);
    }
    (keys, targets)
}