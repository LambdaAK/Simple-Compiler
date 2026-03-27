    .text
    .extern _printf
    .globl _fib
    .p2align 2
_fib:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    sub sp, sp, #144
    str x0, [sp, #0]
    ldr x8, [sp, #0]
    str x8, [sp, #8]
    mov x8, #1
    str x8, [sp, #16]
    ldr x9, [sp, #8]
    ldr x10, [sp, #16]
    cmp x9, x10
    cset x8, eq
    str x8, [sp, #24]
    ldr x8, [sp, #24]
    cbz x8, L_fib_end_if_0
    mov x8, #1
    str x8, [sp, #32]
    ldr x0, [sp, #32]
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret
L_fib_end_if_0:
    ldr x8, [sp, #0]
    str x8, [sp, #40]
    mov x8, #2
    str x8, [sp, #48]
    ldr x9, [sp, #40]
    ldr x10, [sp, #48]
    cmp x9, x10
    cset x8, eq
    str x8, [sp, #56]
    ldr x8, [sp, #56]
    cbz x8, L_fib_end_if_1
    mov x8, #1
    str x8, [sp, #64]
    ldr x0, [sp, #64]
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret
L_fib_end_if_1:
    ldr x8, [sp, #0]
    str x8, [sp, #72]
    mov x8, #1
    str x8, [sp, #80]
    ldr x9, [sp, #72]
    ldr x10, [sp, #80]
    sub x8, x9, x10
    str x8, [sp, #88]
    ldr x0, [sp, #88]
    bl _fib
    str x0, [sp, #96]
    ldr x8, [sp, #0]
    str x8, [sp, #104]
    mov x8, #2
    str x8, [sp, #112]
    ldr x9, [sp, #104]
    ldr x10, [sp, #112]
    sub x8, x9, x10
    str x8, [sp, #120]
    ldr x0, [sp, #120]
    bl _fib
    str x0, [sp, #128]
    ldr x9, [sp, #96]
    ldr x10, [sp, #128]
    add x8, x9, x10
    str x8, [sp, #136]
    ldr x0, [sp, #136]
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret
    .globl _fact
    .p2align 2
_fact:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    sub sp, sp, #96
    str x0, [sp, #0]
    ldr x8, [sp, #0]
    str x8, [sp, #8]
    mov x8, #0
    str x8, [sp, #16]
    ldr x9, [sp, #8]
    ldr x10, [sp, #16]
    cmp x9, x10
    cset x8, eq
    str x8, [sp, #24]
    ldr x8, [sp, #24]
    cbz x8, L_fact_end_if_0
    mov x8, #1
    str x8, [sp, #32]
    ldr x0, [sp, #32]
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret
L_fact_end_if_0:
    ldr x8, [sp, #0]
    str x8, [sp, #40]
    ldr x8, [sp, #0]
    str x8, [sp, #48]
    mov x8, #1
    str x8, [sp, #56]
    ldr x9, [sp, #48]
    ldr x10, [sp, #56]
    sub x8, x9, x10
    str x8, [sp, #64]
    ldr x0, [sp, #64]
    bl _fact
    str x0, [sp, #72]
    ldr x9, [sp, #40]
    ldr x10, [sp, #72]
    mul x8, x9, x10
    str x8, [sp, #80]
    ldr x0, [sp, #80]
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret
    .globl _main
    .p2align 2
_main:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    sub sp, sp, #16
    mov x8, #100
    str x8, [sp, #0]
    ldr x0, [sp, #0]
    bl _fact
    str x0, [sp, #8]
    ldr x8, [sp, #8]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_int_fmt@PAGE
    add x0, x0, L_pr_int_fmt@PAGEOFF
    bl _printf
    add sp, sp, #32
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
L_pr_char_only:
    .asciz "%c"
L_pr_nl_only:
    .asciz "\n"
