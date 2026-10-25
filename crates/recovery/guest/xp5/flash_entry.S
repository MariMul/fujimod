.text
.balign 4
.global xp5_flash_entry
.type xp5_flash_entry,%function
xp5_flash_entry:
    /* Save two scratch registers on the incoming per-core entry stack before
     * moving to the appended, private primary stack. All values are restored. */
    stp x0, x1, [sp, #-16]!
    mrs x0, mpidr_el1
    and x0, x0, #255
    cbnz x0, 9f
    mov x0, sp
    adrp x1, xp5_flash_stack_top
    add x1, x1, :lo12:xp5_flash_stack_top
    mov sp, x1
    sub sp, sp, #256
    stp x2, x3, [sp]
    stp x4, x5, [sp, #16]
    stp x6, x7, [sp, #32]
    stp x8, x9, [sp, #48]
    stp x10, x11, [sp, #64]
    stp x12, x13, [sp, #80]
    stp x14, x15, [sp, #96]
    stp x16, x17, [sp, #112]
    stp x18, x19, [sp, #128]
    stp x20, x21, [sp, #144]
    stp x22, x23, [sp, #160]
    stp x24, x25, [sp, #176]
    stp x26, x27, [sp, #192]
    stp x28, x29, [sp, #208]
    stp x30, x0, [sp, #224]
    mrs x1, nzcv
    str x1, [sp, #240]
    bl xp5_flash_prepare
    ldr x1, [sp, #240]
    msr nzcv, x1
    ldp x2, x3, [sp]
    ldp x4, x5, [sp, #16]
    ldp x6, x7, [sp, #32]
    ldp x8, x9, [sp, #48]
    ldp x10, x11, [sp, #64]
    ldp x12, x13, [sp, #80]
    ldp x14, x15, [sp, #96]
    ldp x16, x17, [sp, #112]
    ldp x18, x19, [sp, #128]
    ldp x20, x21, [sp, #144]
    ldp x22, x23, [sp, #160]
    ldp x24, x25, [sp, #176]
    ldp x26, x27, [sp, #192]
    ldp x28, x29, [sp, #208]
    ldp x30, x0, [sp, #224]
    mov sp, x0
    ldp x0, x1, [sp], #16
.global xp5_flash_setup_branch
xp5_flash_setup_branch:
    b . /* Bound by the packager after verifying this exact instruction. */
9:  wfi
    b 9b
.size xp5_flash_entry, .-xp5_flash_entry
.bss
.balign 16
.global xp5_flash_stack
xp5_flash_stack:
    .zero 2048
.global xp5_flash_stack_top
xp5_flash_stack_top:
