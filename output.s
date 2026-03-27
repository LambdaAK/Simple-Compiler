    .text
    .extern _printf
    .globl _main
    .p2align 2
_main:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    sub sp, sp, #80
    mov x8, #0
    str x8, [sp, #0]
    ldr x8, [sp, #0]
    str x8, [sp, #8]
L_for_head_0:
    ldr x8, [sp, #8]
    str x8, [sp, #16]
    movz x8, #1000, lsl #0
    str x8, [sp, #24]
    ldr x9, [sp, #16]
    ldr x10, [sp, #24]
    cmp x9, x10
    cset x8, lt
    str x8, [sp, #32]
    ldr x8, [sp, #32]
    cbz x8, L_for_end_1
    ldr x8, [sp, #8]
    str x8, [sp, #40]
    ldr x8, [sp, #40]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_int_fmt@PAGE
    add x0, x0, L_pr_int_fmt@PAGEOFF
    bl _printf
    add sp, sp, #32
    ldr x8, [sp, #8]
    str x8, [sp, #48]
    mov x8, #1
    str x8, [sp, #56]
    ldr x9, [sp, #48]
    ldr x10, [sp, #56]
    add x8, x9, x10
    str x8, [sp, #64]
    ldr x8, [sp, #64]
    str x8, [sp, #8]
    b L_for_head_0
L_for_end_1:
    mov sp, x29
    mov x0, #0
    ldp x29, x30, [sp], #16
    ret

    .section __TEXT,__cstring,cstring_literals
L_pr_int_fmt:
    .asciz "%lld\n"
L_pr_true:
    .asciz "true\n"
L_pr_false:
    .asciz "false\n"
L_pr_char_fmt:
    .asciz "%c\n"
