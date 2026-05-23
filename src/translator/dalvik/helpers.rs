use crate::translator::dalvik::insn::Insn;

pub fn sign_extend_4(v: u8) -> i8 {
    if v & 0x8 != 0 { (v as i8) | (-16i8) } else { v as i8 }
}

pub fn sign_extend_8(v: u8) -> i8 {
    v as i8
}

pub fn three_regs<F: Fn(u8, u8, u8) -> Insn>(dst: u8, second_word: u16, f: F) -> Insn {
    let b = (second_word & 0xFF) as u8;
    let c = (second_word >> 8) as u8;
    f(dst, b, c)
}

pub fn decode_invoke35(count_and_d: u8, rest: &[u16]) -> (Vec<u8>, u32) {
    let method_idx = rest[0] as u32;
    let regs_word  = rest[1];
    let count = (count_and_d >> 4) & 0xF;
    let reg_d = count_and_d & 0xF;
    (decode_invoke_args(regs_word, reg_d, count), method_idx)
}

pub fn decode_invoke_args(regs_word: u16, reg_g: u8, count: u8) -> Vec<u8> {
    let d = (regs_word & 0xF) as u8;
    let e = ((regs_word >> 4) & 0xF) as u8;
    let f = ((regs_word >> 8) & 0xF) as u8;
    let g = ((regs_word >> 12) & 0xF) as u8;
    let a = reg_g;
    let all = [d, e, f, g, a];
    all[..count.min(5) as usize].to_vec()
}

pub fn decode_invoke3rc(hi: u8, rest: &[u16]) -> (u8, u8, u32) {
    if rest.len() < 2 { return (0, 0, 0); }
    let method_idx = rest[0] as u32;
    let first      = (rest[1] & 0xFF) as u8;
    (first, hi, method_idx)
}

pub fn parse_packed_switch(code: &[u16], table_pc: usize, insn_pc: i32) -> (i32, Vec<i32>) {
    if table_pc + 4 > code.len() { return (0, vec![]); }
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
        targets.push(insn_pc + rel);
    }
    (first_key, targets)
}

pub fn parse_sparse_switch(code: &[u16], table_pc: usize, insn_pc: i32) -> (Vec<i32>, Vec<i32>) {
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