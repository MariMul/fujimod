#include "sd_pio.h"

#define SDHCI_BLOCK_SIZE_COUNT 0x04u
#define SDHCI_DMA_ADDRESS 0x00u
#define SDHCI_ARGUMENT_LOW 0x08u
#define SDHCI_ARGUMENT_HIGH 0x0au
#define SDHCI_TRANSFER_MODE_COMMAND 0x0cu
#define SDHCI_RESPONSE0 0x10u
#define SDHCI_DATA 0x20u
#define SDHCI_PRESENT_STATE 0x24u
#define SDHCI_HOST_CONTROL 0x28u
#define SDHCI_NORMAL_STATUS 0x30u
#define SDHCI_ERROR_STATUS 0x32u
#define SDHCI_INTERRUPT_ENABLE 0x34u
#define SDHCI_ERROR_ENABLE 0x36u
#define SDHCI_INTERRUPT_SIGNAL_ENABLE 0x38u
#define SDHCI_ERROR_SIGNAL_ENABLE 0x3au

#define PRESENT_COMMAND_INHIBIT (1u << 0)
#define PRESENT_DATA_INHIBIT (1u << 1)
#define PRESENT_WRITE_AVAILABLE (1u << 10)
#define PRESENT_READ_AVAILABLE (1u << 11)

#define STATUS_COMMAND_COMPLETE (1u << 0)
#define STATUS_TRANSFER_COMPLETE (1u << 1)
#define STATUS_ERROR (1u << 15)
#define R1_TRANSFER_ERROR_MASK 0x7ff98008u
#define COMMAND_INTERRUPT_MASK 0xee01u
#define READ_PIO_INTERRUPT_MASK 0xee23u
#define WRITE_PIO_INTERRUPT_MASK 0xee13u
#define ERROR_SIGNAL_MASK 0x17ffu
#define HOST_CONTROL_DMA_MASK 0x0018u
#define SD_BLOCK_SIZE_BOUNDARY_512_KIB 0x7000u

#define READ_PIO_REQUEST 0x123a0036u
#define WRITE_PIO_REQUEST 0x193a0026u
#define SD_BLOCK_SIZE 512u
#define WORDS_PER_BLOCK (SD_BLOCK_SIZE / sizeof(uint32_t))

static volatile uint16_t *reg16(uintptr_t base, uint32_t offset)
{
    return (volatile uint16_t *)(base + offset);
}

static volatile uint32_t *reg32(uintptr_t base, uint32_t offset)
{
    return (volatile uint32_t *)(base + offset);
}

static void configure_interrupts(uintptr_t base, uint16_t mask)
{
    *reg16(base, SDHCI_NORMAL_STATUS) = 0xffffu;
    *reg16(base, SDHCI_ERROR_STATUS) = 0xffffu;
    *reg16(base, SDHCI_INTERRUPT_ENABLE) = mask;
    *reg16(base, SDHCI_ERROR_ENABLE) = 0xffffu;
    *reg16(base, SDHCI_INTERRUPT_SIGNAL_ENABLE) = mask;
    *reg16(base, SDHCI_ERROR_SIGNAL_ENABLE) = ERROR_SIGNAL_MASK;
}

static int poll_interrupt(uintptr_t base, uint16_t wanted,
                          uint32_t poll_limit, uint32_t *response0,
                          uint16_t *error_status)
{
    while (poll_limit-- != 0) {
        uint16_t status = *reg16(base, SDHCI_NORMAL_STATUS);

        if ((status & STATUS_ERROR) != 0) {
            uint16_t errors = *reg16(base, SDHCI_ERROR_STATUS);
            *reg16(base, SDHCI_ERROR_STATUS) = errors;
            *reg16(base, SDHCI_NORMAL_STATUS) = status;
            if (error_status != 0) *error_status = errors;
            return SD_PIO_CONTROLLER_ERROR;
        }
        if ((status & wanted) != 0) {
            *reg16(base, SDHCI_NORMAL_STATUS) =
                wanted == STATUS_COMMAND_COMPLETE ? STATUS_COMMAND_COMPLETE : status;
            if (wanted == STATUS_COMMAND_COMPLETE && response0 != 0)
                *response0 = *reg32(base, SDHCI_RESPONSE0);
            return SD_PIO_OK;
        }
    }

    return SD_PIO_TIMEOUT;
}

static int poll_present_state(uintptr_t base, uint32_t mask,
                              uint32_t expected, uint32_t poll_limit,
                              uint16_t *error_status)
{
    while (poll_limit-- != 0) {
        uint16_t status = *reg16(base, SDHCI_NORMAL_STATUS);

        if ((status & STATUS_ERROR) != 0) {
            uint16_t errors = *reg16(base, SDHCI_ERROR_STATUS);
            *reg16(base, SDHCI_ERROR_STATUS) = errors;
            *reg16(base, SDHCI_NORMAL_STATUS) = status;
            if (error_status != 0) *error_status = errors;
            return SD_PIO_CONTROLLER_ERROR;
        }
        if ((*reg32(base, SDHCI_PRESENT_STATE) & mask) == expected)
            return SD_PIO_OK;
    }

    return SD_PIO_TIMEOUT;
}

static uint32_t load_le32(const uint8_t *bytes)
{
    return (uint32_t)bytes[0] |
           ((uint32_t)bytes[1] << 8) |
           ((uint32_t)bytes[2] << 16) |
           ((uint32_t)bytes[3] << 24);
}

static void store_le32(uint8_t *bytes, uint32_t word)
{
    bytes[0] = (uint8_t)word;
    bytes[1] = (uint8_t)(word >> 8);
    bytes[2] = (uint8_t)(word >> 16);
    bytes[3] = (uint8_t)(word >> 24);
}

int sd_command(uintptr_t host_base, uint32_t request_word,
                   uint32_t argument, uint32_t *response,
                   uint32_t response_words, uint32_t poll_limit,
                   uint16_t *error_status)
{
    uint32_t response0 = 0;
    int result;

    if (host_base == 0 || poll_limit == 0 || response_words > 4 ||
        (request_word & 0xffffu) != 0 ||
        (response_words != 0 && response == 0))
        return SD_PIO_INVALID_ARGUMENT;
    if (error_status != 0) *error_status = 0;
    if (response != 0) {
        for (uint32_t i = 0; i < response_words; ++i) response[i] = 0;
    }

    result = poll_present_state(
        host_base, PRESENT_COMMAND_INHIBIT | PRESENT_DATA_INHIBIT, 0,
        poll_limit, error_status);
    if (result != SD_PIO_OK) return result;

    configure_interrupts(host_base, COMMAND_INTERRUPT_MASK);

    *reg16(host_base, SDHCI_ARGUMENT_LOW) = (uint16_t)argument;
    *reg16(host_base, SDHCI_ARGUMENT_HIGH) = (uint16_t)(argument >> 16);
    *reg32(host_base, SDHCI_TRANSFER_MODE_COMMAND) = request_word;

    result = poll_interrupt(host_base, STATUS_COMMAND_COMPLETE, poll_limit,
                            response_words == 0 ? 0 : &response0,
                            error_status);
    if (result != SD_PIO_OK) return result;

    if (response_words != 0) response[0] = response0;
    for (uint32_t i = 1; i < response_words; ++i)
        response[i] = *reg32(host_base, SDHCI_RESPONSE0 + i * sizeof(uint32_t));

    return SD_PIO_OK;
}

static int transfer_blocks(uintptr_t host_base, uint32_t ocr_ccs,
                           uint32_t lba, uint32_t sector_count,
                           const uint8_t *write_buffer,
                           uint8_t *read_buffer, uint32_t poll_limit,
                           uint32_t *response0, uint16_t *error_status)
{
    int write_to_card = write_buffer != 0;
    uint32_t command_argument;
    uint32_t request;
    uint32_t present_ready;
    uint32_t command_response = 0;
    uint32_t last_lba;
    int result;

    if (host_base == 0 || ocr_ccs > 1 ||
        (write_buffer == 0 && read_buffer == 0) ||
        (write_buffer != 0 && read_buffer != 0) || poll_limit == 0 ||
        sector_count == 0 || sector_count > 0xffffu)
        return SD_PIO_INVALID_ARGUMENT;
    last_lba = lba + sector_count - 1u;
    if (last_lba < lba ||
        (ocr_ccs == 0 && last_lba > (UINT32_MAX >> 9)))
        return SD_PIO_ADDRESS_RANGE;

    command_argument = ocr_ccs != 0 ? lba : lba << 9;
    request = write_to_card ? WRITE_PIO_REQUEST : READ_PIO_REQUEST;
    present_ready = write_to_card ? PRESENT_WRITE_AVAILABLE
                                  : PRESENT_READ_AVAILABLE;
    if (response0 != 0) *response0 = 0;
    if (error_status != 0) *error_status = 0;

    result = poll_present_state(
        host_base, PRESENT_COMMAND_INHIBIT | PRESENT_DATA_INHIBIT, 0,
        poll_limit, error_status);
    if (result != SD_PIO_OK) return result;

    /* Clear stale W1C status before starting this request. */
    configure_interrupts(host_base, write_to_card ? WRITE_PIO_INTERRUPT_MASK
                                                  : READ_PIO_INTERRUPT_MASK);

    *reg32(host_base, SDHCI_DMA_ADDRESS) = 0;
    uint16_t host_control = *reg16(host_base, SDHCI_HOST_CONTROL);
    *reg16(host_base, SDHCI_HOST_CONTROL) =
        (uint16_t)(host_control & (uint16_t)~HOST_CONTROL_DMA_MASK);
    *reg16(host_base, SDHCI_BLOCK_SIZE_COUNT) =
        (uint16_t)((*reg16(host_base, SDHCI_BLOCK_SIZE_COUNT) & 0xf000u) |
                   SD_BLOCK_SIZE_BOUNDARY_512_KIB | SD_BLOCK_SIZE);
    *reg16(host_base, SDHCI_BLOCK_SIZE_COUNT + 2u) = (uint16_t)sector_count;
    *reg16(host_base, SDHCI_ARGUMENT_LOW) = (uint16_t)command_argument;
    *reg16(host_base, SDHCI_ARGUMENT_HIGH) =
        (uint16_t)(command_argument >> 16);
    *reg32(host_base, SDHCI_TRANSFER_MODE_COMMAND) = request;

    result = poll_interrupt(host_base, STATUS_COMMAND_COMPLETE, poll_limit,
                            &command_response, error_status);
    if (result != SD_PIO_OK) return result;
    if (response0 != 0) *response0 = command_response;
    if ((command_response & R1_TRANSFER_ERROR_MASK) != 0)
        return SD_PIO_CARD_ERROR;

    for (uint32_t sector = 0; sector < sector_count; ++sector) {
        const uint8_t *source_sector =
            write_buffer == 0 ? 0 : write_buffer + sector * SD_BLOCK_SIZE;
        uint8_t *destination_sector =
            read_buffer == 0 ? 0 : read_buffer + sector * SD_BLOCK_SIZE;

        result = poll_present_state(host_base, present_ready, present_ready,
                                    poll_limit, error_status);
        if (result != SD_PIO_OK) return result;

        for (uint32_t word_index = 0; word_index < WORDS_PER_BLOCK;
             ++word_index) {
            uint32_t byte_offset = word_index * sizeof(uint32_t);

            if (write_to_card) {
                *reg32(host_base, SDHCI_DATA) =
                    load_le32(source_sector + byte_offset);
            } else {
                store_le32(destination_sector + byte_offset,
                           *reg32(host_base, SDHCI_DATA));
            }
        }
    }

    result = poll_interrupt(host_base, STATUS_TRANSFER_COMPLETE, poll_limit,
                            0, error_status);
    if (result != SD_PIO_OK) return result;

    return SD_PIO_OK;
}

int sd_pio_read(uintptr_t host_base, uint32_t ocr_ccs,
                    uint32_t lba, uint32_t sector_count, uint8_t *buffer,
                    uint32_t poll_limit, uint32_t *response0,
                    uint16_t *error_status)
{
    return transfer_blocks(host_base, ocr_ccs, lba, sector_count, 0, buffer,
                           poll_limit, response0, error_status);
}

int sd_pio_write(uintptr_t host_base, uint32_t ocr_ccs,
                     uint32_t lba, uint32_t sector_count,
                     const uint8_t *buffer, uint32_t poll_limit,
                     uint32_t *response0, uint16_t *error_status)
{
    return transfer_blocks(host_base, ocr_ccs, lba, sector_count, buffer, 0,
                           poll_limit, response0, error_status);
}
