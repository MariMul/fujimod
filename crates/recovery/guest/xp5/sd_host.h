#ifndef SD_HOST_H
#define SD_HOST_H

#include <stdint.h>

enum sd_host_result {
    SD_HOST_OK = 0,
    SD_HOST_INVALID_ARGUMENT = -1,
    SD_HOST_CLOCK_TIMEOUT = -2,
    SD_HOST_HANDSHAKE_TIMEOUT = -3,
    SD_HOST_RESET_TIMEOUT = -4,
};

typedef void (*sd_wait_fn)(void *context, uint32_t value);

/*
 * Apply the controller-register portion of stock sd_controller_initialize().
 * Board power and routing must already be prepared. The delay callback uses
 * stock routine's tick unit.
 */
int sd_host_initialize_registers(uintptr_t host_base,
                                     uint16_t timeout_setting,
                                     sd_wait_fn delay_ticks,
                                     sd_wait_fn handshake_wait,
                                     void *delay_context);

/* Reset the command and data lines as the stock recovery helper does. */
int sd_host_reset_lines(uintptr_t host_base,
                            sd_wait_fn delay_ticks,
                            void *delay_context);

#endif
