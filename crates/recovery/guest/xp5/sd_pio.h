#ifndef SD_PIO_H
#define SD_PIO_H

#include <stdint.h>

enum sd_pio_result {
    SD_PIO_OK = 0,
    SD_PIO_INVALID_ARGUMENT = -1,
    SD_PIO_ADDRESS_RANGE = -2,
    SD_PIO_TIMEOUT = -3,
    SD_PIO_CONTROLLER_ERROR = -4,
    SD_PIO_CARD_ERROR = -5,
};

/* Issue a command-only request word and return 0..4 raw response registers. */
int sd_command(uintptr_t host_base, uint32_t request_word,
                   uint32_t argument, uint32_t *response,
                   uint32_t response_words, uint32_t poll_limit,
                   uint16_t *error_status);

/*
 * Transfer 1..65535 sectors through the XM5 controller's PIO data port.
 *
 * Preconditions: controller 0 is initialized; the card is selected and ready
 * for 512-byte block transfers; the OCR CCS value comes from card init; and
 * buffer addresses are valid CPU-readable/writable memory. poll_limit bounds
 * each individual status wait, rather than measuring elapsed time.
 *
 * response0 and error_status are optional outputs. The stock R1 error mask is
 * checked before moving data. A successful write means the host completed the
 * transfer; callers that need confirmed card-program completion must issue
 * and validate the appropriate card-status command.
 */
int sd_pio_read(uintptr_t host_base, uint32_t ocr_ccs,
                    uint32_t lba, uint32_t sector_count, uint8_t *buffer,
                    uint32_t poll_limit, uint32_t *response0,
                    uint16_t *error_status);

int sd_pio_write(uintptr_t host_base, uint32_t ocr_ccs,
                     uint32_t lba, uint32_t sector_count,
                     const uint8_t *buffer, uint32_t poll_limit,
                     uint32_t *response0, uint16_t *error_status);

#endif
