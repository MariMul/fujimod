/* Card-backed 16 KiB logical firmware reader using the immutable ROM file ABI. */
typedef unsigned int u32;
typedef unsigned short u16;
typedef unsigned char u8;
struct firmware_config { u32 open, read, filesystem, size, capacity; };
extern const volatile struct firmware_config lupi_firmware_config;
static volatile u32 busy;
static u8 discard[512];

static int transfer(u8 *output, u32 count) {
    int (*read)(u8 *,u16,u16 *) = (int (*)(u8 *,u16,u16 *))lupi_firmware_config.read;
    u16 actual = 0;
    if (read(output,(u16)count,&actual) || actual != count) return -255;
    return 0;
}

int lupi_firmware_indexed(u32 block, u32 count, u8 *output,
                           void (*progress)(u32), u32 selector) {
    const volatile struct firmware_config *c = &lupi_firmware_config;
    int (*open)(const char *) = (int (*)(const char *))c->open;
    u32 offset, bytes, at, status, saved, fs, completed = 0;
    if (selector > 1 || block >= c->capacity / 0x4000u ||
        count > c->capacity / 0x4000u - block ||
        count > (~(u32)output) / 0x4000u) return -30;
    if (!count) return 0;
    if (__sync_lock_test_and_set(&busy,1)) return -16;
    __asm__ volatile("mrs %0, cpsr\ncpsid i" : "=r"(saved) :: "memory");
    status = (u32)-255;
    if (open("ffstore.bin")) goto finish;
    fs = *(volatile u32 *)c->filesystem;
    if (!fs || *(volatile u32 *)(fs+0x20) != c->size) goto finish;
    offset = block * 0x4000u;
    bytes = count * 0x4000u;
    at = 0;
    while (at < offset && at < c->size) {
        u32 chunk = offset-at;
        if (chunk > sizeof(discard)) chunk = sizeof(discard);
        if (chunk > c->size-at) chunk = c->size-at;
        if (transfer(discard,chunk)) goto finish;
        at += chunk;
    }
    while (completed < bytes) {
        u32 chunk = bytes-completed;
        u32 present = offset+completed < c->size ? c->size-offset-completed : 0;
        if (chunk > 512) chunk = 512;
        if (present > chunk) present = chunk;
        if (present && transfer(output+completed,present)) goto finish;
        for (u32 i=present; i<chunk; ++i) output[completed+i] = 255;
        completed += chunk;
        if (progress) progress(completed);
    }
    status = 0;
finish:
    __sync_lock_release(&busy);
    __asm__ volatile("msr cpsr_c, %0" :: "r"(saved) : "memory");
    return (int)status;
}
