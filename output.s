    .text
    .extern _printf
    .globl _main
    .p2align 2
_main:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    sub sp, sp, #176
    mov x8, #10
    str x8, [sp, #0]
    ldr x8, [sp, #0]
    str x8, [sp, #8]
    mov x8, #3
    str x8, [sp, #16]
    ldr x8, [sp, #16]
    str x8, [sp, #24]
    ldr x8, [sp, #8]
    str x8, [sp, #32]
    ldr x8, [sp, #24]
    str x8, [sp, #40]
    mov x8, #2
    str x8, [sp, #48]
    ldr x9, [sp, #40]
    ldr x10, [sp, #48]
    mul x8, x9, x10
    str x8, [sp, #56]
    ldr x9, [sp, #32]
    ldr x10, [sp, #56]
    add x8, x9, x10
    str x8, [sp, #64]
    ldr x8, [sp, #64]
    str x8, [sp, #72]
    ldr x8, [sp, #72]
    str x8, [sp, #80]
    mov x8, #15
    str x8, [sp, #88]
    ldr x9, [sp, #80]
    ldr x10, [sp, #88]
    cmp x9, x10
    cset x8, gt
    str x8, [sp, #96]
    ldr x8, [sp, #96]
    str x8, [sp, #104]
    ldr x8, [sp, #104]
    str x8, [sp, #112]
    ldr x8, [sp, #112]
    cbz x8, L_else_1
    ldr x8, [sp, #72]
    str x8, [sp, #120]
    mov x8, #1
    str x8, [sp, #128]
    ldr x9, [sp, #120]
    ldr x10, [sp, #128]
    sub x8, x9, x10
    str x8, [sp, #136]
    ldr x8, [sp, #136]
    str x8, [sp, #72]
    b L_end_if_0
L_else_1:
    mov x8, #0
    str x8, [sp, #144]
    ldr x8, [sp, #144]
    str x8, [sp, #72]
L_end_if_0:
    ldr x8, [sp, #72]
    str x8, [sp, #152]
    ldr x8, [sp, #152]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_int_fmt@PAGE
    add x0, x0, L_pr_int_fmt@PAGEOFF
    bl _printf
    add sp, sp, #32
    ldr x8, [sp, #104]
    str x8, [sp, #160]
    ldr x8, [sp, #160]
    cbz x8, L_prb_0_false
    adrp x0, L_pr_true@PAGE
    add x0, x0, L_pr_true@PAGEOFF
    bl _printf
    b L_prb_0_end
L_prb_0_false:
    adrp x0, L_pr_false@PAGE
    add x0, x0, L_pr_false@PAGEOFF
    bl _printf
L_prb_0_end:
    mov x8, #1
    str x8, [sp, #168]
    ldr x8, [sp, #168]
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
