/* Initial XP4 SD program. Addresses and firmware data are generated bindings. */
#include "ffinit.h"

volatile struct ffinit_progress ffinit_progress;

static ff_u32 word(const ff_u8 *p)
{
    return (ff_u32)p[0] | ((ff_u32)p[1] << 8) |
           ((ff_u32)p[2] << 16) | ((ff_u32)p[3] << 24);
}

static void fail(ff_u32 error) __attribute__((noreturn));
static void fail(ff_u32 error)
{
    ffinit_progress.error = error;
    ffinit_progress.stage = 0xffffffffu;
    ffinit_halt();
}

/* Topaz header entries encode operation in the address's low two bits. */
void ffinit_prepare(void)
{
    ff_u32 i, count = word(ffinit_boot_header + 0x14);
    ffinit_progress.stage = 1;
    ffinit_progress.error = 0;
    ffinit_progress.loaded = 0;
    ffinit_progress.crc32 = 0;
    if (count > 0xfdu || !ffinit_config.setup_data)
        fail(0x100);
    for (i = 0; i < count; ++i) {
        const ff_u8 *row = ffinit_boot_header + 0x18 + i * 8;
        ff_u32 encoded = word(row), value = word(row + 4);
        volatile ff_u32 *address = (volatile ff_u32 *)(encoded & ~3u);
        switch (encoded & 3u) {
        case 0:
            *address = value;
            break;
        case 1:
            (void)*address;
            break;
        case 2:
            while (*address != value) {}
            break;
        case 3:
            ffinit_counter_start();
            while (ffinit_counter_read() < value) {}
            ffinit_counter_stop();
            break;
        }
    }
    ffinit_progress.stage = 2;
    ffinit_resume_setup();
}

ff_u32 __attribute__((noinline)) ffinit_crc32(const volatile ff_u8 *bytes, ff_u32 size)
{
    ff_u32 crc = 0xffffffffu;
    while (size--)
        crc = (crc >> 8) ^ ffinit_crc_table[(crc ^ *bytes++) & 0xffu];
    return ~crc;
}

void ffinit_load(void)
{
    ff_u8 *workspace = (ff_u8 *)ffinit_config.workspace;
    ff_u8 *file = workspace + 0x240;
    ff_u32 *actual = (ff_u32 *)(workspace + 0x290);
    ff_u32 crc;
    int result;
    ffinit_progress.stage = 3;
    result = ffinit_config.mount(workspace, "SD40_0", 1);
    if (result)
        fail(0x200u | (ff_u32)result);
    result = ffinit_config.open(file, "ffboot.bin", 1);
    if (result)
        fail(0x300u | (ff_u32)result);
    if (word(file + 0x10) != ffinit_config.payload_size || word(file + 0x14))
        fail(0x400);
    ffinit_progress.stage = 4;
    *actual = 0;
    result = ffinit_config.read(file, (void *)ffinit_config.payload_address,
                               ffinit_config.payload_size, actual);
    ffinit_progress.loaded = *actual;
    if (result || *actual != ffinit_config.payload_size)
        fail(0x500u | (ff_u32)result);
    result = ffinit_config.close(file);
    if (result)
        fail(0x600u | (ff_u32)result);
    ffinit_progress.stage = 5;
    crc = ffinit_crc32((const volatile ff_u8 *)ffinit_config.payload_address,
                      ffinit_config.payload_size);
    ffinit_progress.crc32 = crc;
    if (crc != ffinit_config.payload_crc32)
        fail(0x700);
    ffinit_progress.stage = 6;
    ffinit_enter_ram();
}
