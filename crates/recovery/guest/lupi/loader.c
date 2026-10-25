/* SRAM-resident second read for a ROM whose first file read is u16-sized. */
typedef unsigned int u32;
typedef unsigned short u16;
typedef unsigned char u8;
struct config { u32 open, read, filesystem, size, crc32; };
extern const volatile struct config lupi_config;
static void stop(void) { for (;;) __asm__ volatile("wfi"); }
void lupi_main(void) {
    const volatile struct config *c = &lupi_config;
    int (*open)(const char *) = (int (*)(const char *))c->open;
    int (*read)(u8 *, u16, u16 *) = (int (*)(u8 *, u16, u16 *))c->read;
    u32 at = 0, crc = ~0u;
    if (open("ffimage.bin")) stop();
    u32 fs = *(volatile u32 *)c->filesystem;
    if (!fs || *(volatile u32 *)(fs + 0x20) != c->size) stop();
    while (at < c->size) {
        u32 count = c->size - at;
        u16 actual = 0;
        if (count > 0x8000) count = 0x8000;
        u8 *dest = (u8 *)(0x40000000u + at);
        if (read(dest, (u16)count, &actual) || actual != count) stop();
        for (u32 i = 0; i < count; ++i) {
            crc ^= dest[i];
            for (u32 bit = 0; bit < 8; ++bit)
                crc = (crc >> 1) ^ ((0u - (crc & 1)) & 0xedb88320u);
        }
        at += count;
    }
    if (~crc != c->crc32) stop();
    __asm__ volatile("mov r0, #0\n"
                     "mcr p15, 0, r0, c7, c5, 0\n"
                     "dsb\n"
                     "isb\n"
                     "bx r0" ::: "r0", "memory");
    stop();
}
