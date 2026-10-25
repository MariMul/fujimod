/* Board effects are explicit catalog inputs, applied in their declared order. */
#include "board.h"
int setup_sd_board_initialize(sd_wait_fn delay, void *context, uint32_t startup_ticks)
{
    if (!delay || setup_sd_board.count > 32) return SD_HOST_INVALID_ARGUMENT;
    for (uint32_t i = 0; i < setup_sd_board.count; ++i) {
        const struct setup_sd_operation *op = &setup_sd_board.operations[i];
        volatile uint32_t *address = (volatile uint32_t *)(uintptr_t)op->address;
        if (op->kind == 0) *address = op->value;
        else if (op->kind == 1) *address = (*address & ~op->mask) | (op->value & op->mask);
        else if (op->kind == 2) delay(context, startup_ticks);
        else return SD_HOST_INVALID_ARGUMENT;
    }
    return SD_HOST_OK;
}
uintptr_t setup_sd_board_host(void) { return (uintptr_t)setup_sd_board.host; }
int setup_sd_board_host_initialize(sd_wait_fn delay, void *context)
{
    return sd_host_initialize_registers(setup_sd_board_host(),
        (uint16_t)setup_sd_board.timeout_setting, delay, delay, context);
}
