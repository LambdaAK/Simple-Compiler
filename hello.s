.global _main
.extern _printf

.section __TEXT,__text

_main:
    ; load address of string into x0 (1st argument)
    adrp x0, message@PAGE
    add  x0, x0, message@PAGEOFF

    bl _printf   ; call printf
    bl _printf   ; call printf


    mov x0, #0   ; return 0
    ret

.section __DATA,__data
message:
    .asciz "Hello, world!\n"