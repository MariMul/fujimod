.text
.balign 4
.global a64_copy_to_physical
.type a64_copy_to_physical, %function
a64_copy_to_physical:
    cbz x2, 2f
1:
    ldrb w3, [x1], #1
    strb w3, [x0], #1
    subs x2, x2, #1
    b.ne 1b
2:
    ret
.size a64_copy_to_physical, .-a64_copy_to_physical

/* Replace the setup continuation load without using a secondary core's stack or
 * changing any other register/NZCV. Replay the zero handoff locally. */
.global a64_boot_zero_gate
.type a64_boot_zero_gate, %function
a64_boot_zero_gate:
1:
    adrp x30, a64_boot_ready
    add x30, x30, :lo12:a64_boot_ready
    ldar w30, [x30]
    cbnz w30, 2f
    wfe
    b 1b
2:
    mov x30, xzr
    isb
    ret
.size a64_boot_zero_gate, .-a64_boot_zero_gate

.bss
.balign 64
.global a64_boot_ready
.type a64_boot_ready, %object
a64_boot_ready:
    .zero 4
.size a64_boot_ready, .-a64_boot_ready
