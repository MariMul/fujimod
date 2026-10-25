#ifndef RECOVERY_SD_BOARD_H
#define RECOVERY_SD_BOARD_H
#include "sd_host.h"
struct setup_sd_operation {
    uint64_t address;
    uint32_t mask, value, kind, reserved;
};
struct setup_sd_board {
    uint64_t host;
    uint32_t timeout_setting, count;
    struct setup_sd_operation operations[32];
};
extern const struct setup_sd_board setup_sd_board;
#endif
