#include "flash_boot.h"

/* Serial NOR register protocol. Catalog configuration supplies all device
 * addresses; polling always has a finite budget. */
static uint32_t read32(uintptr_t address) { return *(volatile uint32_t *)address; }
static void write32(uintptr_t address, uint32_t value) { *(volatile uint32_t *)address = value; }
static void field(uintptr_t address, uint32_t mask, uint32_t value)
{
    write32(address, (read32(address) & ~mask) | (value & mask));
}
static int wait32(uintptr_t address, uint32_t mask, uint32_t value, uint32_t limit)
{
    while (limit--) if ((read32(address) & mask) == value) return 0;
    return -1;
}
static int pause_controller(const struct xp5_flash_boot *b, int pause)
{
    if ((read32(b->controller + 0xe0) & 0x10) == 0) return 0;
    field(b->controller + 0xfc, 1, pause ? 1 : 0);
    return wait32(b->controller + 0xe0, 0x80, pause ? 0x80 : 0, (uint32_t)b->poll_limit);
}
static int set_field(const struct xp5_flash_boot *b, uint32_t offset, uint32_t mask, uint32_t value)
{
    if (pause_controller(b, 1)) return -1;
    field(b->controller + offset, mask, value);
    return pause_controller(b, 0);
}
static int set_pair(const struct xp5_flash_boot *b, uint32_t offset,
                    uint32_t first_mask, uint32_t first, uint32_t second_mask, uint32_t second)
{
    if (pause_controller(b, 1)) return -1;
    field(b->controller + offset, first_mask, first);
    field(b->controller + offset, second_mask, second);
    return pause_controller(b, 0);
}
int xp5_flash_initialize(const struct xp5_flash_boot *b)
{
    uintptr_t base = b->controller;
    field(base + 0x10, 0x10001, 0);
    if (wait32(base + 0xe0, 0x18, 0x18, (uint32_t)b->poll_limit)) return -1;
    /* These stock setters reject a zero status bit 4; the preceding disabled
     * status wait establishes the prerequisite rather than bypassing it. */
    field(base + 0xc4, 1, 1);
    field(base + 0xc8, 0x10, 0);
    field(base + 0x10, 0x1000, 0);
    field(base + 0x10, 0x300, 0);
    field(base + 0x10, 0x10, 0);
    field(base + 0x10, 1, 1);
    if (wait32(base + 0xe0, 0x18, 8, (uint32_t)b->poll_limit)) return -1;
    if (set_field(b, 0x14, 0xffff0000, 0xffff0000)
            || set_pair(b, 0x14, 0xf0, 0x10, 0xf, 0)
            || set_pair(b, 0x18, 0xf000, 0x1000, 0xf00, 0)
            || set_pair(b, 0x18, 0xff000000, 10u << 24, 0xff0000, 10u << 16)
            || set_pair(b, 0x18, 1, 0, 0xf0, 0)) return -1;
    if (pause_controller(b, 1)) return -1;
    write32(base + 0xc, 2);
    if (pause_controller(b, 0)) return -1;
    write32(base + 0x20, 1);
    if (pause_controller(b, 1)) return -1;
    write32(base + 0x24, (uint32_t)b->aperture);
    write32(base + 0x28, UINT32_MAX);
    write32(base + 0x2c, UINT32_MAX);
    write32(base + 0x30, UINT32_MAX);
    write32(base + 0x34, (uint32_t)b->window_mask);
    if (pause_controller(b, 0) || pause_controller(b, 1)) return -1;
    /* Stock configures program opcode 02, one command byte, three address
     * bytes, zero data length, and all single-lane format fields. */
    write32(base + 8, 0x0d);
    write32(base + 0x48, 2);
    write32(base + 0x68, 0);
    write32(base + 0x78, 0);
    write32(base + 0x98, 0);
    return pause_controller(b, 0);
}
int xp5_flash_scan(const struct xp5_flash_boot *b, const uint32_t *expected)
{
    static const uint8_t delays[16] = {8,0,1,2,3,4,5,6,7,9,10,11,12,13,14,15};
    uint32_t previous_mode = 0;
    for (uint32_t divider = 0x1f; divider <= 0xff; divider += 8) {
        if (divider != 0x1f) {
            write32(b->clock, 0x100 | divider);
            write32(b->commit, 2);
            if (wait32(b->commit, UINT32_MAX, 0, (uint32_t)b->poll_limit)) return -1;
        }
        for (uint32_t delay = 0; delay < 16; ++delay) for (uint32_t mode = 0; mode < 2; ++mode) {
            if (pause_controller(b, 1)) return -1;
            /* Preserve stock's one-call delay: configuration consumes the
             * previously stored context flag, then publishes this mode. */
            write32(b->controller + 4, 0x0d | (previous_mode << 12));
            write32(b->controller + 0x44, 0x0b);
            write32(b->controller + 0x64, 0);
            write32(b->controller + 0x74, delays[delay]);
            write32(b->controller + 0x94, 0);
            if (pause_controller(b, 0)) return -1;
            previous_mode = mode;
            uint32_t index;
            for (index = 0; index < 4; ++index)
                if (read32(b->aperture + 4*index) != expected[index]) break;
            if (index == 4) return 0;
        }
    }
    return -1;
}
int xp5_flash_header(const uint32_t *header, uint32_t count, uint32_t polls)
{
    for (uint32_t n = 0; n < count; ++n) {
        uint32_t encoded = header[6+2*n], value = header[7+2*n];
        uintptr_t address = encoded & ~3u;
        switch (encoded & 3) {
        case 0: write32(address, value); break;
        case 1: (void)read32(address); break;
        case 2: if (wait32(address, UINT32_MAX, value, polls)) return -1; break;
        case 3: {
            uint64_t counter, attempts = polls;
            __asm__ volatile("msr pmcr_el0, %0\n\tmsr pmcntenset_el0, %1" : : "r"(7ull), "r"(1ull << 31) : "memory");
            do { __asm__ volatile("mrs %0, pmccntr_el0" : "=r"(counter)); }
            while (counter < value && --attempts);
            __asm__ volatile("msr pmcntenclr_el0, %0\n\tmsr pmcr_el0, %1" : : "r"(1ull << 31), "r"(4ull) : "memory");
            if (counter < value) return -1;
            break;
        }
        }
    }
    return 0;
}
