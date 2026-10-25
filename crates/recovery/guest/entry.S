/* Architectural entry/continuation for the C initial SD loader. */
.syntax unified
.arch armv7-a
.arm
.section .text.ffinit_entry,"ax",%progbits
.global ffinit_entry
.type ffinit_entry,%function
ffinit_entry:
    setend le
    cpsid aif, #0x13
    mrc p15, 0, r0, c0, c0, 5
    ands r0, r0, #3
    bne ffinit_halt
    ldr sp, =ffinit_stack_top
    b ffinit_prepare
.size ffinit_entry, .-ffinit_entry

.section .text.ffinit_dram_ready,"ax",%progbits
.global ffinit_dram_ready
.type ffinit_dram_ready,%function
ffinit_dram_ready:
    mrc p15, 0, r0, c0, c0, 5
    ands r0, r0, #3
    bne ffinit_halt
    ldr sp, =ffinit_stack_top
    b ffinit_load
.size ffinit_dram_ready, .-ffinit_dram_ready

.section .text.ffinit_resume_setup,"ax",%progbits
.global ffinit_resume_setup
.type ffinit_resume_setup,%function
ffinit_resume_setup:
    ldr pc, =ffinit_stock_resume
.size ffinit_resume_setup, .-ffinit_resume_setup

.section .text.ffinit_enter_ram,"ax",%progbits
.global ffinit_enter_ram
.type ffinit_enter_ram,%function
ffinit_enter_ram:
    mov r0, #0
    bx r0
.size ffinit_enter_ram, .-ffinit_enter_ram

.section .text.ffinit_halt,"ax",%progbits
.global ffinit_halt
.type ffinit_halt,%function
ffinit_halt:
1:  wfi
    b 1b
.size ffinit_halt, .-ffinit_halt

.section .text.ffinit_counter_start,"ax",%progbits
.global ffinit_counter_start
.type ffinit_counter_start,%function
ffinit_counter_start:
    mov r0, #7
    mov r1, #0x80000000
    mcr p15, 0, r0, c9, c12, 0
    mcr p15, 0, r1, c9, c12, 1
    bx lr
.size ffinit_counter_start, .-ffinit_counter_start

.section .text.ffinit_counter_read,"ax",%progbits
.global ffinit_counter_read
.type ffinit_counter_read,%function
ffinit_counter_read:
    mrc p15, 0, r0, c9, c13, 0
    bx lr
.size ffinit_counter_read, .-ffinit_counter_read

.section .text.ffinit_counter_stop,"ax",%progbits
.global ffinit_counter_stop
.type ffinit_counter_stop,%function
ffinit_counter_stop:
    mov r0, #0x80000000
    mov r1, #6
    mcr p15, 0, r0, c9, c12, 2
    mcr p15, 0, r1, c9, c12, 0
    bx lr
.size ffinit_counter_stop, .-ffinit_counter_stop
