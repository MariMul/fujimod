#ifndef FFINIT_H
#define FFINIT_H

typedef unsigned char ff_u8;
typedef unsigned int ff_u32;

struct ffinit_config {
    ff_u32 payload_address;
    ff_u32 payload_size;
    ff_u32 payload_crc32;
    ff_u32 workspace;
    int (*mount)(void *, const char *, ff_u32);
    int (*open)(void *, const char *, ff_u32);
    int (*read)(void *, void *, ff_u32, ff_u32 *);
    int (*close)(void *);
    const ff_u8 *setup_data;
};

struct ffinit_progress {
    ff_u32 stage;
    ff_u32 error;
    ff_u32 loaded;
    ff_u32 crc32;
};

extern const struct ffinit_config ffinit_config;
extern const ff_u8 ffinit_boot_header[];
extern const ff_u8 ffinit_setup_data[];
extern const ff_u32 ffinit_crc_table[256];
extern volatile struct ffinit_progress ffinit_progress;

void ffinit_entry(void);
void ffinit_dram_ready(void);
void ffinit_prepare(void) __attribute__((noreturn));
void ffinit_load(void) __attribute__((noreturn));
void ffinit_resume_setup(void) __attribute__((noreturn));
void ffinit_enter_ram(void) __attribute__((noreturn));
void ffinit_halt(void) __attribute__((noreturn));
void ffinit_counter_start(void);
ff_u32 ffinit_counter_read(void);
void ffinit_counter_stop(void);
ff_u32 ffinit_crc32(const volatile ff_u8 *, ff_u32);

#endif
