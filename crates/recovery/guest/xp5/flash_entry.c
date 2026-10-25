#include <stdint.h>
#include <stddef.h>
#include "flash_boot.h"


struct xp5_flash_configuration {
    uint64_t abi;
    struct xp5_flash_boot flash;
    uint64_t setup, original_word, branch_word;
    uint64_t pio, clock, clock_mask, reset, reset_mask;
    uint32_t green, red, pin, count;
    uint32_t pins[12];
    const uint32_t *header;
    uint64_t success_hold_units;
};
_Static_assert(sizeof(struct xp5_flash_configuration)==200 && offsetof(struct xp5_flash_configuration,header)==184
    && offsetof(struct xp5_flash_configuration,success_hold_units)==192,"flash configuration ABI");
volatile struct xp5_flash_configuration xp5_flash_configuration = {.abi = 1};
volatile uint64_t xp5_flash_progress[4];
/* The located setup timer initializes its counter and accepts microseconds. */
extern void boot_setup_delay(uint32_t units);
static void write32(uintptr_t address, uint32_t value) { *(volatile uint32_t *)address = value; }
static void lamp_pin(uintptr_t pio, uint32_t port, uint32_t pin, int high)
{
    write32(pio+port*64, (1u<<(pin+16)) | (high?1u<<pin:0));
}
static void prepare_lamp(uintptr_t pio, uint32_t port, uint32_t pin)
{
    uintptr_t base=pio+port*64;
    write32(pio+0x500, (1u<<(port+16)) | (1u<<port));
    lamp_pin(pio,port,pin,0);
    write32(base+12,3u<<(pin*2+16));
    write32(base+8,1u<<(pin+16));
    write32(base+4,(1u<<(pin+16))|(1u<<pin));
}
static void show(int green, int red)
{
    const volatile struct xp5_flash_configuration *b=&xp5_flash_configuration;
    lamp_pin(b->pio,b->green,b->pin,green);
    lamp_pin(b->pio,b->red,b->pin,red);
}
static void delay(void) { for (volatile unsigned n=0;n<1000000;n++) __asm__ volatile("nop"); }
static void fail(unsigned count) __attribute__((noreturn));
static void fail(unsigned count)
{
    xp5_flash_progress[1]=count;
    for (;;) {
        for (unsigned n=0;n<count;n++) {show(0,1);delay();show(0,0);delay();}
        delay();delay();delay();
    }
}
void xp5_flash_prepare(void)
{
    uint64_t el, sctlr;
    __asm__ volatile("mrs %0, CurrentEL" : "=r"(el));
    if (el!=12) for (;;) __asm__ volatile("wfi");
    __asm__ volatile("mrs %0, sctlr_el3" : "=r"(sctlr));
    if (sctlr & 0x1005) for (;;) __asm__ volatile("wfi");
    xp5_flash_progress[0]=1;
    xp5_flash_progress[2]=el;
    xp5_flash_progress[3]=sctlr;
    const struct xp5_flash_configuration *b=(const struct xp5_flash_configuration *)&xp5_flash_configuration;
    if (b->abi!=1 || !b->flash.poll_limit || b->flash.poll_limit>UINT32_MAX || b->count>253
            || b->green>=16 || b->red>=16 || b->green==3 || b->red==3
            || b->green==b->red || b->pin>=8 || !b->header)
        for (;;) __asm__ volatile("wfi");
    *(volatile uint32_t *)b->clock |= (uint32_t)b->clock_mask;
    *(volatile uint32_t *)b->reset |= (uint32_t)b->reset_mask;
    prepare_lamp(b->pio,b->green,b->pin);
    prepare_lamp(b->pio,b->red,b->pin);
    show(1,1);
    if (xp5_flash_initialize(&b->flash)) fail(1);
    for (unsigned n=0;n<6;n++) write32(b->pins[2*n],b->pins[2*n+1]);
    xp5_flash_progress[0]=2;
    if (xp5_flash_scan(&b->flash,b->header)) fail(1);
    show(0,0);
    xp5_flash_progress[0]=3;
    /* Preserve the complete input header in the XP5 boot-service area. */
    for (unsigned n=0;n<0x800/4;n++) write32(b->setup+0x3a800+4u*n,b->header[n]);
    if (xp5_flash_header(b->header,b->count,(uint32_t)b->flash.poll_limit)) fail(2);
    uint64_t hold=b->success_hold_units;
    if (hold) {
        show(1,0);
        while (hold) {
            /* Keep each counter interval below its 32-bit wrap period. */
            uint32_t units=hold>1000000?1000000:(uint32_t)hold;
            boot_setup_delay(units);
            hold-=units;
        }
        show(0,0);
    }
    if (*(volatile uint32_t *)b->setup != b->branch_word) fail(2);
    write32(b->setup,(uint32_t)b->original_word);
    __asm__ volatile("dsb sy\n\tic iallu\n\tdsb sy\n\tisb" : : : "memory");
    xp5_flash_progress[0]=4;
}
