.syntax unified
.arm
.section .start,"ax",%progbits
.global lupi_entry
lupi_entry:
    ldr r0, =lupi_source
    ldr r1, =lupi_sram
    ldr r2, =lupi_length
1:  ldr r3, [r0], #4
    str r3, [r1], #4
    subs r2, r2, #4
    bne 1b
    dsb
    isb
    ldr sp, =0xfff01ff8
    ldr pc, =lupi_main
.ltorg
