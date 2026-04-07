//! Emit **Apple macOS ARM64** assembly from [`crate::ir::IrModule`].
//!
//! Stack-only storage: every IR temp and variable slot gets an 8-byte word on the stack.
//! `x29` is the frame pointer; locals sit below the saved `fp`/`lr` pair (`sub sp, sp, #N`).

use std::collections::HashMap;
use std::fmt::Write;

use crate::ir::{Instr, IrModule};

fn module_needs_printf(module: &IrModule) -> bool {
    let check = |instrs: &[Instr]| {
        instrs.iter().any(|i| {
            matches!(
                i,
                Instr::PrintInt(_)
                    | Instr::PrintBool(_)
                    | Instr::PrintChar(_)
                    | Instr::PrintCharOnly(_)
                    | Instr::PrintNl
            )
        })
    };
    check(&module.main.instrs) || module.functions.iter().any(|f| check(&f.instrs))
}

/// ARM64 assembly: user functions then `_main` (mach-O style symbols with leading `_`).
pub fn emit_arm64(module: &IrModule) -> String {
    let needs_printf = module_needs_printf(module);
    let mut o = String::new();

    writeln!(o, "    .text").unwrap();
    if needs_printf {
        writeln!(o, "    .extern _printf").unwrap();
    }

    let mut print_bool_id = 0usize;
    for f in &module.functions {
        let sym = format!("_{}", f.name);
        emit_procedure(
            &mut o,
            &sym,
            &f.instrs,
            needs_printf,
            &mut print_bool_id,
            false,
        );
    }
    emit_procedure(
        &mut o,
        "_main",
        &module.main.instrs,
        needs_printf,
        &mut print_bool_id,
        true,
    );

    if needs_printf {
        writeln!(o).unwrap();
        writeln!(o, "    .section __TEXT,__cstring,cstring_literals").unwrap();
        writeln!(o, "L_pr_int_fmt:").unwrap();
        writeln!(o, "    .asciz \"%lld\\n\"").unwrap();
        writeln!(o, "L_pr_true:").unwrap();
        writeln!(o, "    .asciz \"true\\n\"").unwrap();
        writeln!(o, "L_pr_false:").unwrap();
        writeln!(o, "    .asciz \"false\\n\"").unwrap();
        writeln!(o, "L_pr_char_fmt:").unwrap();
        writeln!(o, "    .asciz \"%c\\n\"").unwrap();
        writeln!(o, "L_pr_char_only:").unwrap();
        writeln!(o, "    .asciz \"%c\"").unwrap();
        writeln!(o, "L_pr_nl_only:").unwrap();
        writeln!(o, "    .asciz \"\\n\"").unwrap();
    }

    o
}

fn emit_procedure(
    o: &mut String,
    symbol: &str,
    instrs: &[Instr],
    needs_printf: bool,
    print_bool_id: &mut usize,
    is_main: bool,
) {
    let slots = collect_slots(instrs);
    let n = slots.len();
    let locals = align16(n * 8);
    let off: HashMap<&str, usize> = slots
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i * 8))
        .collect();

    writeln!(o, "    .globl {symbol}").unwrap();
    writeln!(o, "    .p2align 2").unwrap();
    writeln!(o, "{symbol}:").unwrap();
    writeln!(o, "    stp x29, x30, [sp, #-16]!").unwrap();
    writeln!(o, "    mov x29, sp").unwrap();
    emit_sub_sp(o, locals);

    for instr in instrs {
        emit_instr(o, instr, &off, needs_printf, print_bool_id);
    }

    if is_main {
        writeln!(o, "    mov sp, x29").unwrap();
        writeln!(o, "    mov x0, #0").unwrap();
        writeln!(o, "    ldp x29, x30, [sp], #16").unwrap();
        writeln!(o, "    ret").unwrap();
    }
}

fn align16(n: usize) -> usize {
    (n + 15) & !15
}

/// `sub sp, sp, #N` for the whole locals allocation. Each immediate must fit in 12 bits (0–4095).
fn emit_sub_sp(o: &mut String, mut total: usize) {
    const MAX_IMM: usize = 4095;
    while total > 0 {
        let chunk = total.min(MAX_IMM);
        writeln!(o, "    sub sp, sp, #{}", chunk).unwrap();
        total -= chunk;
    }
}

/// `add sp, sp, #N` — undo [`emit_sub_sp`].
fn emit_add_sp(o: &mut String, mut total: usize) {
    const MAX_IMM: usize = 4095;
    while total > 0 {
        let chunk = total.min(MAX_IMM);
        writeln!(o, "    add sp, sp, #{}", chunk).unwrap();
        total -= chunk;
    }
}

/// Deterministic slot order: first time a name appears in a full IR walk.
fn collect_slots(instrs: &[Instr]) -> Vec<String> {
    let mut seen = HashMap::<String, ()>::new();
    let mut order = Vec::new();

    let mut note = |s: &str| {
        if !seen.contains_key(s) {
            seen.insert(s.to_string(), ());
            order.push(s.to_string());
        }
    };

    for instr in instrs {
        match instr {
            Instr::RecvParam(d, _) | Instr::RecvParamStack(d, _) => note(d),
            Instr::Const(d, _) => note(d),
            Instr::Add(d, a, b)
            | Instr::Sub(d, a, b)
            | Instr::Mul(d, a, b)
            | Instr::Div(d, a, b)
            | Instr::Mod(d, a, b) => {
                note(d);
                note(a);
                note(b);
            }
            Instr::Neg(d, a) | Instr::Not(d, a) => {
                note(d);
                note(a);
            }
            Instr::Eq(d, a, b)
            | Instr::Ne(d, a, b)
            | Instr::Lt(d, a, b)
            | Instr::Le(d, a, b)
            | Instr::Gt(d, a, b)
            | Instr::Ge(d, a, b)
            | Instr::And(d, a, b)
            | Instr::Or(d, a, b) => {
                note(d);
                note(a);
                note(b);
            }
            Instr::LoadVar(d, v) => {
                note(d);
                note(v);
            }
            Instr::StoreVar(v, s) => {
                note(v);
                note(s);
            }
            Instr::StoreVarImm(v, _) => note(v),
            Instr::LoadIndex(d, first, idx, _) => {
                note(d);
                note(first);
                note(idx);
            }
            Instr::StoreIndex(first, idx, src, _) => {
                note(first);
                note(idx);
                note(src);
            }
            Instr::AddrLocal(d, base, _) => {
                note(d);
                note(base);
            }
            Instr::LoadMem(d, p) => {
                note(d);
                note(p);
            }
            Instr::StoreMem(p, s) => {
                note(p);
                note(s);
            }
            Instr::JumpIfZero(c, _) => note(c),
            Instr::Call { dst, args, .. } => {
                if let Some(d) = dst {
                    note(d);
                }
                for a in args {
                    note(a);
                }
            }
            Instr::PrintInt(t) | Instr::PrintBool(t) | Instr::PrintChar(t) | Instr::PrintCharOnly(t) => {
                note(t);
            }
            Instr::PrintNl => {}
            Instr::Label(_) | Instr::Jump(_) => {}
            Instr::Ret(Some(t)) => note(t),
            Instr::Ret(None) => {}
        }
    }
    order
}

fn asm_label(name: &str) -> String {
    let mut s = String::from("L_");
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    s
}

fn ldr_stack(o: &mut String, reg: &str, slot_off: usize) {
    writeln!(o, "    ldr {reg}, [sp, #{}]", slot_off).unwrap();
}

fn str_stack(o: &mut String, reg: &str, slot_off: usize) {
    writeln!(o, "    str {reg}, [sp, #{}]", slot_off).unwrap();
}

/// Load `imm` into `rd` (64-bit). Uses one `mov` when the assembler constant fits.
/// `rd = sp + off` (handles large frame offsets).
fn add_sp_offset(o: &mut String, rd: &str, off: usize) {
    if off <= 4095 {
        writeln!(o, "    add {rd}, sp, #{}", off).unwrap();
    } else {
        mov_i64(o, rd, off as i64);
        writeln!(o, "    add {rd}, sp, {rd}").unwrap();
    }
}

fn mov_i64(o: &mut String, rd: &str, imm: i64) {
    if imm >= -0x100 && imm <= 0xff {
        writeln!(o, "    mov {rd}, #{imm}").unwrap();
        return;
    }
    let u = imm as u64;
    writeln!(o, "    movz {}, #{}, lsl #0", rd, u & 0xffff).unwrap();
    for shift in (16..64).step_by(16) {
        let chunk = (u >> shift) & 0xffff;
        if chunk != 0 {
            writeln!(o, "    movk {}, #{}, lsl #{}", rd, chunk, shift).unwrap();
        }
    }
}

fn emit_instr(
    o: &mut String,
    instr: &Instr,
    off: &HashMap<&str, usize>,
    needs_printf: bool,
    print_bool_id: &mut usize,
) {
    let oa = |name: &str| *off.get(name).expect("slot in layout");

    match instr {
        Instr::RecvParam(slot, reg_idx) => {
            let reg = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"][*reg_idx as usize];
            str_stack(o, reg, oa(slot));
        }
        Instr::RecvParamStack(slot, stack_word) => {
            let off = 16usize + (*stack_word as usize) * 8;
            if off <= 4095 {
                writeln!(o, "    ldr x8, [x29, #{}]", off).unwrap();
            } else {
                mov_i64(o, "x9", off as i64);
                writeln!(o, "    add x9, x29, x9").unwrap();
                writeln!(o, "    ldr x8, [x9]").unwrap();
            }
            str_stack(o, "x8", oa(slot));
        }
        Instr::Const(dst, imm) => {
            mov_i64(o, "x8", *imm);
            str_stack(o, "x8", oa(dst));
        }
        Instr::Add(dst, a, b) => {
            ldr_stack(o, "x9", oa(a));
            ldr_stack(o, "x10", oa(b));
            writeln!(o, "    add x8, x9, x10").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::Sub(dst, a, b) => {
            ldr_stack(o, "x9", oa(a));
            ldr_stack(o, "x10", oa(b));
            writeln!(o, "    sub x8, x9, x10").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::Mul(dst, a, b) => {
            ldr_stack(o, "x9", oa(a));
            ldr_stack(o, "x10", oa(b));
            writeln!(o, "    mul x8, x9, x10").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::Div(dst, a, b) => {
            ldr_stack(o, "x9", oa(a));
            ldr_stack(o, "x10", oa(b));
            writeln!(o, "    sdiv x8, x9, x10").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::Mod(dst, a, b) => {
            ldr_stack(o, "x9", oa(a));
            ldr_stack(o, "x10", oa(b));
            writeln!(o, "    sdiv x11, x9, x10").unwrap();
            writeln!(o, "    msub x8, x11, x10, x9").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::Neg(dst, a) => {
            ldr_stack(o, "x9", oa(a));
            writeln!(o, "    neg x8, x9").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::Not(dst, a) => {
            ldr_stack(o, "x9", oa(a));
            writeln!(o, "    mov x10, #1").unwrap();
            writeln!(o, "    sub x8, x10, x9").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::Eq(dst, a, b) => {
            cmp_cset(o, dst, a, b, off, "eq");
        }
        Instr::Ne(dst, a, b) => {
            cmp_cset(o, dst, a, b, off, "ne");
        }
        Instr::Lt(dst, a, b) => {
            cmp_cset(o, dst, a, b, off, "lt");
        }
        Instr::Le(dst, a, b) => {
            cmp_cset(o, dst, a, b, off, "le");
        }
        Instr::Gt(dst, a, b) => {
            cmp_cset(o, dst, a, b, off, "gt");
        }
        Instr::Ge(dst, a, b) => {
            cmp_cset(o, dst, a, b, off, "ge");
        }
        Instr::And(dst, a, b) => {
            ldr_stack(o, "x9", oa(a));
            ldr_stack(o, "x10", oa(b));
            writeln!(o, "    and x8, x9, x10").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::Or(dst, a, b) => {
            ldr_stack(o, "x9", oa(a));
            ldr_stack(o, "x10", oa(b));
            writeln!(o, "    orr x8, x9, x10").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::LoadVar(dst, var) => {
            ldr_stack(o, "x8", oa(var));
            str_stack(o, "x8", oa(dst));
        }
        Instr::StoreVar(var, src) => {
            ldr_stack(o, "x8", oa(src));
            str_stack(o, "x8", oa(var));
        }
        Instr::StoreVarImm(var, imm) => {
            mov_i64(o, "x8", *imm);
            str_stack(o, "x8", oa(var));
        }
        Instr::LoadIndex(dst, first_slot, idx, _len) => {
            ldr_stack(o, "x10", oa(idx));
            add_sp_offset(o, "x9", oa(first_slot));
            writeln!(o, "    ldr x8, [x9, x10, lsl #3]").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::StoreIndex(first_slot, idx, src, _len) => {
            ldr_stack(o, "x10", oa(idx));
            ldr_stack(o, "x8", oa(src));
            add_sp_offset(o, "x9", oa(first_slot));
            writeln!(o, "    str x8, [x9, x10, lsl #3]").unwrap();
        }
        Instr::AddrLocal(dst, base_slot, byte_off) => {
            let total = oa(base_slot) + byte_off;
            add_sp_offset(o, "x8", total);
            str_stack(o, "x8", oa(dst));
        }
        Instr::LoadMem(dst, ptr) => {
            ldr_stack(o, "x9", oa(ptr));
            writeln!(o, "    ldr x8, [x9]").unwrap();
            str_stack(o, "x8", oa(dst));
        }
        Instr::StoreMem(ptr, src) => {
            ldr_stack(o, "x9", oa(ptr));
            ldr_stack(o, "x8", oa(src));
            writeln!(o, "    str x8, [x9]").unwrap();
        }
        Instr::Label(name) => {
            writeln!(o, "{}:", asm_label(name)).unwrap();
        }
        Instr::Jump(name) => {
            writeln!(o, "    b {}", asm_label(name)).unwrap();
        }
        Instr::JumpIfZero(cond, name) => {
            ldr_stack(o, "x8", oa(cond));
            writeln!(o, "    cbz x8, {}", asm_label(name)).unwrap();
        }
        Instr::PrintInt(temp) => {
            assert!(needs_printf);
            // Apple arm64 variadic: first `...` arg is stored at `[sp]` of a freshly
            // reserved block (not passed in x1); see clang -S for `printf("%lld", x)`.
            ldr_stack(o, "x8", oa(temp));
            writeln!(o, "    sub sp, sp, #32").unwrap();
            writeln!(o, "    str x8, [sp]").unwrap();
            writeln!(o, "    adrp x0, L_pr_int_fmt@PAGE").unwrap();
            writeln!(o, "    add x0, x0, L_pr_int_fmt@PAGEOFF").unwrap();
            writeln!(o, "    bl _printf").unwrap();
            writeln!(o, "    add sp, sp, #32").unwrap();
        }
        Instr::PrintBool(temp) => {
            assert!(needs_printf);
            let id = *print_bool_id;
            *print_bool_id += 1;
            let lbl_f = format!("L_prb_{id}_false");
            let lbl_end = format!("L_prb_{id}_end");
            ldr_stack(o, "x8", oa(temp));
            writeln!(o, "    cbz x8, {}", lbl_f).unwrap();
            writeln!(o, "    adrp x0, L_pr_true@PAGE").unwrap();
            writeln!(o, "    add x0, x0, L_pr_true@PAGEOFF").unwrap();
            writeln!(o, "    bl _printf").unwrap();
            writeln!(o, "    b {}", lbl_end).unwrap();
            writeln!(o, "{}:", lbl_f).unwrap();
            writeln!(o, "    adrp x0, L_pr_false@PAGE").unwrap();
            writeln!(o, "    add x0, x0, L_pr_false@PAGEOFF").unwrap();
            writeln!(o, "    bl _printf").unwrap();
            writeln!(o, "{}:", lbl_end).unwrap();
        }
        Instr::PrintChar(temp) => {
            assert!(needs_printf);
            ldr_stack(o, "x8", oa(temp));
            writeln!(o, "    sub sp, sp, #32").unwrap();
            writeln!(o, "    str x8, [sp]").unwrap();
            writeln!(o, "    adrp x0, L_pr_char_fmt@PAGE").unwrap();
            writeln!(o, "    add x0, x0, L_pr_char_fmt@PAGEOFF").unwrap();
            writeln!(o, "    bl _printf").unwrap();
            writeln!(o, "    add sp, sp, #32").unwrap();
        }
        Instr::PrintCharOnly(temp) => {
            assert!(needs_printf);
            ldr_stack(o, "x8", oa(temp));
            writeln!(o, "    sub sp, sp, #32").unwrap();
            writeln!(o, "    str x8, [sp]").unwrap();
            writeln!(o, "    adrp x0, L_pr_char_only@PAGE").unwrap();
            writeln!(o, "    add x0, x0, L_pr_char_only@PAGEOFF").unwrap();
            writeln!(o, "    bl _printf").unwrap();
            writeln!(o, "    add sp, sp, #32").unwrap();
        }
        Instr::PrintNl => {
            assert!(needs_printf);
            writeln!(o, "    sub sp, sp, #32").unwrap();
            writeln!(o, "    adrp x0, L_pr_nl_only@PAGE").unwrap();
            writeln!(o, "    add x0, x0, L_pr_nl_only@PAGEOFF").unwrap();
            writeln!(o, "    bl _printf").unwrap();
            writeln!(o, "    add sp, sp, #32").unwrap();
        }
        Instr::Call {
            dst,
            callee,
            args,
        } => {
            const ARGREG: [&str; 8] = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"];
            let n_reg = args.len().min(8);
            let n_stack = args.len().saturating_sub(8);
            let stack_bytes = align16(n_stack * 8);
            if n_stack > 0 {
                emit_sub_sp(o, stack_bytes);
                for k in 0..n_stack {
                    let src_off = oa(&args[8 + k]);
                    let dst_off = k * 8;
                    if src_off + stack_bytes <= 4095 {
                        writeln!(
                            o,
                            "    ldr x9, [sp, #{}]",
                            src_off + stack_bytes
                        )
                        .unwrap();
                    } else {
                        add_sp_offset(o, "x10", src_off + stack_bytes);
                        writeln!(o, "    ldr x9, [x10]").unwrap();
                    }
                    writeln!(o, "    str x9, [sp, #{}]", dst_off).unwrap();
                }
            }
            for i in 0..n_reg {
                if n_stack > 0 {
                    let so = oa(&args[i]);
                    if so + stack_bytes <= 4095 {
                        writeln!(o, "    ldr {}, [sp, #{}]", ARGREG[i], so + stack_bytes)
                            .unwrap();
                    } else {
                        add_sp_offset(o, "x10", so + stack_bytes);
                        writeln!(o, "    ldr {}, [x10]", ARGREG[i]).unwrap();
                    }
                } else {
                    ldr_stack(o, ARGREG[i], oa(&args[i]));
                }
            }
            writeln!(o, "    bl _{}", callee).unwrap();
            if n_stack > 0 {
                emit_add_sp(o, stack_bytes);
            }
            if let Some(d) = dst {
                str_stack(o, "x0", oa(d));
            }
        }
        Instr::Ret(src) => {
            if let Some(t) = src {
                ldr_stack(o, "x0", oa(t));
            }
            writeln!(o, "    mov sp, x29").unwrap();
            writeln!(o, "    ldp x29, x30, [sp], #16").unwrap();
            writeln!(o, "    ret").unwrap();
        }
    }
}

fn cmp_cset(
    o: &mut String,
    dst: &str,
    a: &str,
    b: &str,
    off: &HashMap<&str, usize>,
    cond: &str,
) {
    let oa = |name: &str| *off.get(name).expect("slot");
    ldr_stack(o, "x9", oa(a));
    ldr_stack(o, "x10", oa(b));
    writeln!(o, "    cmp x9, x10").unwrap();
    writeln!(o, "    cset x8, {}", cond).unwrap();
    str_stack(o, "x8", oa(dst));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{IrModule, IrProgram};

    #[test]
    fn emit_main_has_globl_and_ret() {
        let mut main = IrProgram::new();
        main.push(Instr::Const("t0".into(), 5));
        main.push(Instr::StoreVar("v0".into(), "t0".into()));
        let module = IrModule {
            main,
            functions: vec![],
        };
        let asm = emit_arm64(&module);
        assert!(asm.contains(".globl _main"));
        assert!(asm.contains("_main:"));
        assert!(asm.contains("ret"));
    }
}
