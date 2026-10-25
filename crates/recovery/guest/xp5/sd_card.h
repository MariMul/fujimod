#ifndef SD_CARD_H
#define SD_CARD_H

#include "sd_host.h"

struct sd_card {
    uint32_t ocr;
    uint16_t rca;
    uint8_t ocr_ccs;
    uint8_t v2_compatible;
    uint64_t capacity_bytes;
    uint64_t sector_count;
};

/*
 * Initialize an SD v1/v2 card through CMD16, then select four-bit bus mode
 * with ACMD6. Host clock, board power, and pin routing must already be
 * initialized. The delay callback supplies the stock 10-tick wait used
 * between ACMD41 readiness attempts.
 *
 * This path does not issue MMC CMD1 or negotiate optional speed modes.
 */
int sd_card_initialize(uintptr_t host_base, uint32_t poll_limit,
                           sd_wait_fn delay_ticks, void *delay_context,
                           struct sd_card *card);

/* Poll CMD13 until the card's Ready-for-Data status bit is set. */
int sd_card_wait_ready(uintptr_t host_base,
                           const struct sd_card *card,
                           uint32_t poll_limit,
                           sd_wait_fn delay_ticks, void *delay_context);

/* Poll CMD13 for transfer state AND Ready-for-Data, with a caller bound. */
int sd_card_wait_transfer_state(uintptr_t host_base, const struct sd_card *card,
                                uint32_t poll_limit, uint32_t attempts,
                                sd_wait_fn delay_ticks, void *delay_context);

/* Capacity-checked block wrappers; writes wait for CMD13 Ready-for-Data. */
int sd_card_read_blocks(uintptr_t host_base,
                            const struct sd_card *card, uint32_t lba,
                            uint32_t sector_count, uint8_t *buffer,
                            uint32_t poll_limit, sd_wait_fn delay_ticks,
                            void *delay_context, uint32_t *response0,
                            uint16_t *error_status);

int sd_card_write_blocks(uintptr_t host_base,
                             const struct sd_card *card, uint32_t lba,
                             uint32_t sector_count, const uint8_t *buffer,
                             uint32_t poll_limit, sd_wait_fn delay_ticks,
                             void *delay_context, uint32_t *response0,
                             uint16_t *error_status);

#endif
