#ifndef XP5_FLASH_BOOT_H
#define XP5_FLASH_BOOT_H
#include <stdint.h>
struct xp5_flash_boot {
    uint64_t controller, aperture, window_mask, clock, commit, poll_limit;
};
int xp5_flash_initialize(const struct xp5_flash_boot *binding);
int xp5_flash_scan(const struct xp5_flash_boot *binding, const uint32_t *expected);
int xp5_flash_header(const uint32_t *header, uint32_t count, uint32_t polls);
#endif
