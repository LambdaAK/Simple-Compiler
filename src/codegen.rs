//! Emit **Apple macOS ARM64** assembly from [`crate::ir::IrProgram`].
//!
//! Stack-only storage: every IR temp and variable slot gets an 8-byte word on the stack.
//! `x29` is the frame pointer; locals sit below the saved `fp`/`lr` pair (`sub sp, sp, #N`).

use std::collections::HashMap;
use std::fmt::Write;

use crate::ir::{Instr, IrProgram};

/// ARM64 assembly for a single `\_main` (text section, machO-style symbol).
pub fn emit_arm64(ir: &IrProgram) -> String {
    let needs_printf = ir.instrs.iter().any(|i| {
        matches!(
            i,
            Instr::PrintInt(_) | Instr::PrintBool(_) | Instr::PrintChar(_)
        )
    });

    let slots = collect_slots(&ir.instrs);
    let n = slots.len();
    let locals = align16(n * 8);
    let off: HashMap<&str, usize> = slots
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i * 8))
        .collect();

    let mut o = String::new();

    writeln!(o, "    .text").unwrap();
    if needs_printf {
        writeln!(o, "    .extern _printf").unwrap();
    }
    writeln!(o, "    .globl _main").unwrap();
    writeln!(o, "    .p2align 2").unwrap();
    writeln!(o, "_main:").unwrap();
    writeln!(o, "    stp x29, x30, [sp, #-16]!").unwrap();
    writeln!(o, "    mov x29, sp").unwrap();
    if locals > 0 {
        writeln!(o, "    sub sp, sp, #{}", locals).unwrap();
    }

    let mut print_bool_id = 0usize;
    for instr in &ir.instrs {
        emit_instr(&mut o, instr, &off, needs_printf, &mut print_bool_id);
    }

    writeln!(o, "    mov sp, x29").unwrap();
    writeln!(o, "    mov x0, #0").unwrap();
    writeln!(o, "    ldp x29, x30, [sp], #16").unwrap();
    writeln!(o, "    ret").unwrap();

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
    }

    o
}

fn align16(n: usize) -> usize {
    (n + 15) & !15
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
            Instr::Const(d, _) => note(d),
            Instr::Add(d, a, b) | Instr::Sub(d, a, b) | Instr::Mul(d, a, b) | Instr::Div(d, a, b) => {
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
            Instr::JumpIfZero(c, _) => note(c),
            Instr::PrintInt(t) | Instr::PrintBool(t) | Instr::PrintChar(t) => note(t),
            Instr::Label(_) | Instr::Jump(_) => {}
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
    use crate::ir::IrProgram;

    #[test]
    fn emit_main_has_globl_and_ret() {
        let mut ir = IrProgram::new();
        ir.push(Instr::Const("t0".into(), 5));
        ir.push(Instr::StoreVar("v0".into(), "t0".into()));
        let asm = emit_arm64(&ir);
        assert!(asm.contains(".globl _main"));
        assert!(asm.contains("_main:"));
        assert!(asm.contains("ret"));
    }
}
