#include "sd_host.h"
#include "sd_card.h"
#include "fat32.h"
#include "checksum.h"
#include "aarch64_boot.h"

extern const uint32_t boot_setup_initialize_call;
extern const uint32_t boot_setup_ram_entry_load;
extern void boot_setup_initialize(void);
extern void boot_setup_delay(uint32_t units);
int setup_sd_board_initialize(sd_wait_fn delay, void *context, uint32_t milliseconds);
uintptr_t setup_sd_board_host(void);
int setup_sd_board_host_initialize(sd_wait_fn delay, void *context);

struct setup_sd_request { uint64_t abi, bytes, crc32, startup_ms, poll_limit; };
volatile struct setup_sd_request setup_sd_request = { .abi = 1 };
struct setup_sd_progress {
    uint64_t magic, stage, error, loaded, crc32, stock_delay_scale, current_el, sctlr;
} __attribute__((aligned(64)));
volatile struct setup_sd_progress setup_sd_progress;
static uint8_t bounce[16384] __attribute__((aligned(64)));
static uint8_t scratch[512] __attribute__((aligned(64)));
static struct sd_card card;
static struct fat32_volume volume;
static struct fat32_file file;

_Static_assert(sizeof(struct setup_sd_request) == 40, "setup SD request ABI");
_Static_assert(sizeof(struct setup_sd_progress) == 64, "setup SD progress ABI");

static void fail(int error) __attribute__((noreturn));
static void fail(int error)
{
    setup_sd_progress.error = (uint64_t)(int64_t)error;
    __asm__ volatile("dsb sy" : : : "memory");
    for (;;) __asm__ volatile("wfi");
}

static void delay(void *context, uint32_t milliseconds)
{
    (void)context;
    /* Reuse the setup timer's 48-cycle unit instead of assuming CNTFRQ/CNTPCT
     * were configured by the ROM. The boot callbacks request at most 1000 ms. */
    boot_setup_delay(milliseconds * 1000u);
}

static int read_blocks(void *context, uint32_t lba, uint32_t sectors, uint8_t *buffer)
{
    (void)context;
    return sd_card_read_blocks(setup_sd_board_host(), &card, lba, sectors, buffer,
                              (uint32_t)setup_sd_request.poll_limit, delay, 0, 0, 0);
}

void setup_sd_initialize(void)
{
    setup_sd_progress.magic = (uint64_t)'S' | (uint64_t)'D' << 8 |
                              (uint64_t)'B' << 16 | (uint64_t)'T' << 24;
    setup_sd_progress.stage = 1;
    if (setup_sd_request.abi != 1 || !setup_sd_request.bytes
            || setup_sd_request.bytes >= (1ull << 30) || setup_sd_request.crc32 > UINT32_MAX
            || !setup_sd_request.startup_ms || setup_sd_request.startup_ms > 1000
            || !setup_sd_request.poll_limit || setup_sd_request.poll_limit > UINT32_MAX)
        fail(-100);
    boot_setup_initialize();
    setup_sd_progress.stage = 2;
    uint64_t el, sctlr;
    __asm__ volatile("mrs %0, CurrentEL" : "=r"(el));
    setup_sd_progress.current_el = el;
    if (el == 12) __asm__ volatile("mrs %0, sctlr_el3" : "=r"(sctlr));
    else if (el == 8) __asm__ volatile("mrs %0, sctlr_el2" : "=r"(sctlr));
    else if (el == 4) __asm__ volatile("mrs %0, sctlr_el1" : "=r"(sctlr));
    else fail(-101);
    setup_sd_progress.sctlr = sctlr;
    /* This first stage uses physical addresses with MMU and caches off. */
    if (sctlr & ((1u << 12) | (1u << 2) | 1u)) fail(-102);
    setup_sd_progress.stock_delay_scale = 1000;
    int result = setup_sd_board_initialize(delay, 0, (uint32_t)setup_sd_request.startup_ms);
    if (result) fail(-200 + result);
    result = setup_sd_board_host_initialize(delay, 0);
    if (result) fail(-300 + result);
    setup_sd_progress.stage = 3;
    result = sd_card_initialize(setup_sd_board_host(), (uint32_t)setup_sd_request.poll_limit, delay, 0, &card);
    if (result) fail(-400 + result);
    setup_sd_progress.stage = 4;
    if (card.sector_count > UINT32_MAX) fail(-500);
    result = fat32_mount(&volume, read_blocks, 0, (uint32_t)card.sector_count, scratch);
    if (result) fail(-500 + result);
    setup_sd_progress.stage = 5;
    result = fat32_open_root(&volume, "FFBOOT  BIN", &file, scratch);
    if (result) fail(-600 + result);
    if (file.size != setup_sd_request.bytes) fail(-610);
    setup_sd_progress.stage = 6;
    while (setup_sd_progress.loaded < setup_sd_request.bytes) {
        size_t received = 0;
        result = fat32_read(&volume, &file, bounce, sizeof(bounce), &received, scratch);
        if (result) fail(-700 + result);
        if (!received || received > setup_sd_request.bytes - setup_sd_progress.loaded) fail(-710);
        setup_sd_progress.crc32 = recovery_crc32_update((uint32_t)setup_sd_progress.crc32, bounce, received);
        a64_copy_to_physical((uintptr_t)setup_sd_progress.loaded, bounce, received);
        setup_sd_progress.loaded += received;
    }
    setup_sd_progress.stage = 7;
    if (setup_sd_progress.crc32 != setup_sd_request.crc32) fail(-800);
    __asm__ volatile("dsb sy\n\tisb" : : : "memory");
    setup_sd_progress.stage = 8;
    __atomic_store_n(&a64_boot_ready, 1, __ATOMIC_RELEASE);
    __asm__ volatile("sev" : : : "memory");
}
