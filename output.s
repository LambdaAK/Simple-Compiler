    .text
    .extern _printf
    .globl _main
    .p2align 2
_main:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    sub sp, sp, #336
    mov x8, #72
    str x8, [sp, #0]
    mov x8, #101
    str x8, [sp, #8]
    mov x8, #108
    str x8, [sp, #16]
    mov x8, #108
    str x8, [sp, #24]
    mov x8, #111
    str x8, [sp, #32]
    mov x8, #32
    str x8, [sp, #40]
    mov x8, #32
    str x8, [sp, #48]
    mov x8, #87
    str x8, [sp, #56]
    mov x8, #111
    str x8, [sp, #64]
    mov x8, #114
    str x8, [sp, #72]
    mov x8, #108
    str x8, [sp, #80]
    mov x8, #100
    str x8, [sp, #88]
    mov x8, #33
    str x8, [sp, #96]
    mov x8, #0
    str x8, [sp, #104]
    mov x8, #0
    str x8, [sp, #112]
    ldr x10, [sp, #112]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #120]
    ldr x8, [sp, #120]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #120]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #1
    str x8, [sp, #128]
    ldr x10, [sp, #128]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #136]
    ldr x8, [sp, #136]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #136]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #2
    str x8, [sp, #144]
    ldr x10, [sp, #144]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #152]
    ldr x8, [sp, #152]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #152]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #3
    str x8, [sp, #160]
    ldr x10, [sp, #160]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #168]
    ldr x8, [sp, #168]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #168]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #4
    str x8, [sp, #176]
    ldr x10, [sp, #176]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #184]
    ldr x8, [sp, #184]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #184]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #5
    str x8, [sp, #192]
    ldr x10, [sp, #192]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #200]
    ldr x8, [sp, #200]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #200]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #6
    str x8, [sp, #208]
    ldr x10, [sp, #208]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #216]
    ldr x8, [sp, #216]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #216]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #7
    str x8, [sp, #224]
    ldr x10, [sp, #224]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #232]
    ldr x8, [sp, #232]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #232]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #8
    str x8, [sp, #240]
    ldr x10, [sp, #240]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #248]
    ldr x8, [sp, #248]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #248]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #9
    str x8, [sp, #256]
    ldr x10, [sp, #256]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #264]
    ldr x8, [sp, #264]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #264]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #10
    str x8, [sp, #272]
    ldr x10, [sp, #272]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #280]
    ldr x8, [sp, #280]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #280]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #11
    str x8, [sp, #288]
    ldr x10, [sp, #288]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #296]
    ldr x8, [sp, #296]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #296]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #12
    str x8, [sp, #304]
    ldr x10, [sp, #304]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #312]
    ldr x8, [sp, #312]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #312]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
    mov x8, #13
    str x8, [sp, #320]
    ldr x10, [sp, #320]
    add x9, sp, #0
    ldr x8, [x9, x10, lsl #3]
    str x8, [sp, #328]
    ldr x8, [sp, #328]
    cbz x8, L_print_string_end_0
    ldr x8, [sp, #328]
    sub sp, sp, #32
    str x8, [sp]
    adrp x0, L_pr_char_only@PAGE
    add x0, x0, L_pr_char_only@PAGEOFF
    bl _printf
    add sp, sp, #32
L_print_string_end_0:
    sub sp, sp, #32
    adrp x0, L_pr_nl_only@PAGE
    add x0, x0, L_pr_nl_only@PAGEOFF
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
