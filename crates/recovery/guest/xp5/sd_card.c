#include "sd_card.h"

#include "sd_pio.h"

#define CMD0_REQUEST 0x00000000u
#define CMD8_REQUEST 0x081a0000u
#define CMD55_REQUEST 0x371a0000u
#define ACMD41_REQUEST 0x29020000u
#define ACMD6_REQUEST 0x061a0000u
#define CMD2_REQUEST 0x02090000u
#define CMD3_REQUEST 0x031a0000u
#define CMD9_REQUEST 0x09090000u
#define CMD7_REQUEST 0x071b0000u
#define CMD13_REQUEST 0x0d1a0000u
#define CMD16_REQUEST 0x101a0000u

#define SDHCI_ERROR_COMMAND_TIMEOUT 0x0001u
#define OCR_READY (1u << 31)
#define OCR_CCS (1u << 30)
#define OCR_VOLTAGE_MASK 0x00ffff00u
#define OCR_SD_V2_REQUEST 0x51000000u
#define OCR_SD_V1_REQUEST 0x01000000u
#define OCR_MINIMUM_VOLTAGE_WINDOW (1u << 21)
#define R1_ERROR_MASK 0x7ff98008u
#define R1_ILLEGAL_COMMAND (1u << 22)
#define R1_READY_FOR_DATA (1u << 8)
#define R6_ERROR_MASK 0x0000e000u
#define ACMD41_ATTEMPTS 300u
#define CARD_STATUS_ATTEMPTS 0x32u

static int command(uintptr_t host_base, uint32_t request, uint32_t argument,
                   uint32_t response[4], uint32_t response_words,
                   uint32_t poll_limit, uint16_t *error_status)
{
    return sd_command(host_base, request, argument, response,
                          response_words, poll_limit, error_status);
}

static int command_app(uintptr_t host_base, uint32_t argument,
                       uint32_t poll_limit, uint32_t *response,
                       uint16_t *error_status)
{
    uint32_t response_words[4] = {0, 0, 0, 0};
    int result = command(host_base, CMD55_REQUEST, 0, response_words, 1,
                         poll_limit, error_status);
    if (result != SD_PIO_OK) return result;
    if ((response_words[0] & R1_ERROR_MASK) != 0)
        return SD_PIO_CARD_ERROR;

    result = command(host_base, ACMD41_REQUEST, argument, response_words, 1,
                     poll_limit, error_status);
    if (result == SD_PIO_OK) *response = response_words[0];
    return result;
}

static int wait_for_ocr_ready(uintptr_t host_base, uint32_t ocr_request,
                              uint32_t poll_limit,
                              sd_wait_fn delay_ticks,
                              void *delay_context, uint32_t *ocr)
{
    for (uint32_t attempt = 0; attempt < ACMD41_ATTEMPTS; ++attempt) {
        uint32_t response = 0;
        uint16_t error_status = 0;
        int result = command_app(host_base, ocr_request, poll_limit,
                                 &response, &error_status);
        if (result != SD_PIO_OK) return result;
        if ((response & OCR_READY) != 0) {
            *ocr = response;
            return SD_PIO_OK;
        }
        if (attempt + 1u != ACMD41_ATTEMPTS)
            delay_ticks(delay_context, 10);
    }

    return SD_PIO_TIMEOUT;
}

static int parse_csd_capacity(const uint32_t response[4],
                              uint64_t *capacity_bytes)
{
    uint8_t raw[16];
    for (uint32_t i = 0; i < 4; ++i) {
        raw[i * 4u] = (uint8_t)response[i];
        raw[i * 4u + 1u] = (uint8_t)(response[i] >> 8);
        raw[i * 4u + 2u] = (uint8_t)(response[i] >> 16);
        raw[i * 4u + 3u] = (uint8_t)(response[i] >> 24);
    }

    uint32_t structure = raw[14] >> 6;
    if (structure == 0) {
        uint32_t read_block_length = ((raw[2] & 3u) << 2) | (raw[1] >> 6);
        uint32_t size_multiplier = ((raw[5] & 3u) << 1) | (raw[4] >> 7);
        uint32_t block_length;
        if (read_block_length == 0) return SD_PIO_CARD_ERROR;
        if (read_block_length > 11)
            block_length = 0x800u;
        else
            block_length = 1u << read_block_length;

        uint32_t card_size = (raw[6] >> 6) | ((uint32_t)raw[7] << 2) |
                             ((uint32_t)(raw[8] & 3u) << 10);
        uint64_t block_count = (uint64_t)(card_size + 1u) <<
                               (size_multiplier + 2u);
        *capacity_bytes = block_count * block_length;
    } else if (structure == 1) {
        uint32_t card_size = (uint32_t)raw[5] |
                             ((uint32_t)raw[6] << 8) |
                             ((uint32_t)(raw[7] & 0x3fu) << 16);
        *capacity_bytes = ((uint64_t)card_size + 1u) * 0x80000u;
    } else {
        return SD_PIO_CARD_ERROR;
    }

    if (*capacity_bytes == 0 || (*capacity_bytes & 0x1ffu) != 0)
        return SD_PIO_CARD_ERROR;
    return SD_PIO_OK;
}

int sd_card_initialize(uintptr_t host_base, uint32_t poll_limit,
                           sd_wait_fn delay_ticks, void *delay_context,
                           struct sd_card *card)
{
    uint32_t response[4] = {0, 0, 0, 0};
    uint16_t error_status = 0;
    int v2_compatible = 0;
    int result;

    if (host_base == 0 || poll_limit == 0 || delay_ticks == 0 || card == 0)
        return SD_PIO_INVALID_ARGUMENT;

    card->ocr = 0;
    card->rca = 0;
    card->ocr_ccs = 0;
    card->v2_compatible = 0;
    card->capacity_bytes = 0;
    card->sector_count = 0;

    result = command(host_base, CMD0_REQUEST, 0, response, 0, poll_limit,
                     &error_status);
    if (result != SD_PIO_OK) return result;

    result = command(host_base, CMD8_REQUEST, 0x000001aau, response, 1,
                     poll_limit, &error_status);
    if (result == SD_PIO_CONTROLLER_ERROR &&
        (error_status & SDHCI_ERROR_COMMAND_TIMEOUT) != 0) {
        int reset_result = sd_host_reset_lines(
            host_base, delay_ticks, delay_context);
        if (reset_result != SD_HOST_OK) return reset_result;
    } else if (result != SD_PIO_OK) {
        return result;
    } else {
        if ((response[0] & 0x00000fffu) == 0x000001aau) {
            v2_compatible = 1;
        } else if ((response[0] & R1_ERROR_MASK) != R1_ILLEGAL_COMMAND) {
            return SD_PIO_CARD_ERROR;
        }
    }

    uint32_t probe_ocr = 0;
    result = command_app(host_base, 0, poll_limit, &probe_ocr,
                         &error_status);
    if (result != SD_PIO_OK) return result;
    if ((probe_ocr & OCR_MINIMUM_VOLTAGE_WINDOW) == 0)
        return SD_PIO_CARD_ERROR;

    uint32_t ocr_request = probe_ocr & OCR_VOLTAGE_MASK;
    ocr_request |= v2_compatible ? OCR_SD_V2_REQUEST : OCR_SD_V1_REQUEST;

    result = wait_for_ocr_ready(host_base, ocr_request, poll_limit,
                                delay_ticks, delay_context, &card->ocr);
    if (result != SD_PIO_OK) return result;

    result = command(host_base, CMD2_REQUEST, 0, response, 4, poll_limit,
                     &error_status);
    if (result != SD_PIO_OK) return result;

    result = command(host_base, CMD3_REQUEST, 0, response, 1, poll_limit,
                     &error_status);
    if (result != SD_PIO_OK) return result;
    if ((response[0] & R6_ERROR_MASK) != 0)
        return SD_PIO_CARD_ERROR;
    card->rca = (uint16_t)(response[0] >> 16);
    if (card->rca == 0) return SD_PIO_CARD_ERROR;

    result = command(host_base, CMD9_REQUEST, (uint32_t)card->rca << 16,
                     response, 4, poll_limit, &error_status);
    if (result != SD_PIO_OK) return result;
    result = parse_csd_capacity(response, &card->capacity_bytes);
    if (result != SD_PIO_OK) return result;
    card->sector_count = card->capacity_bytes >> 9;

    result = command(host_base, CMD7_REQUEST, (uint32_t)card->rca << 16,
                     response, 1, poll_limit, &error_status);
    if (result != SD_PIO_OK) return result;
    if ((response[0] & R1_ERROR_MASK) != 0)
        return SD_PIO_CARD_ERROR;

    result = command(host_base, CMD16_REQUEST, 512, response, 1,
                     poll_limit, &error_status);
    if (result != SD_PIO_OK) return result;
    if ((response[0] & R1_ERROR_MASK) != 0)
        return SD_PIO_CARD_ERROR;

    result = command(host_base, CMD55_REQUEST,
                     (uint32_t)card->rca << 16, response, 1, poll_limit,
                     &error_status);
    if (result != SD_PIO_OK) return result;
    if ((response[0] & R1_ERROR_MASK) != 0)
        return SD_PIO_CARD_ERROR;

    result = command(host_base, ACMD6_REQUEST, 2, response, 1, poll_limit,
                     &error_status);
    if (result != SD_PIO_OK) return result;
    if ((response[0] & R1_ERROR_MASK) != 0)
        return SD_PIO_CARD_ERROR;

    card->ocr_ccs = (uint8_t)((card->ocr & OCR_CCS) != 0);
    card->v2_compatible = (uint8_t)v2_compatible;
    return SD_PIO_OK;
}

int sd_card_wait_ready(uintptr_t host_base,
                           const struct sd_card *card,
                           uint32_t poll_limit,
                           sd_wait_fn delay_ticks, void *delay_context)
{
    if (host_base == 0 || card == 0 || card->rca == 0 || poll_limit == 0 ||
        delay_ticks == 0)
        return SD_PIO_INVALID_ARGUMENT;

    for (uint32_t attempt = 0; attempt < CARD_STATUS_ATTEMPTS; ++attempt) {
        uint32_t response[4] = {0, 0, 0, 0};
        uint16_t error_status = 0;
        int result = command(host_base, CMD13_REQUEST,
                             (uint32_t)card->rca << 16, response, 1,
                             poll_limit, &error_status);
        if (result != SD_PIO_OK) return result;
        if ((response[0] & R1_ERROR_MASK) != 0)
            return SD_PIO_CARD_ERROR;
        if ((response[0] & R1_READY_FOR_DATA) != 0)
            return SD_PIO_OK;
        if (attempt + 1u != CARD_STATUS_ATTEMPTS)
            delay_ticks(delay_context, 1);
    }

    return SD_PIO_TIMEOUT;
}

/* The caller chooses the duration: one delay unit between status polls.
 * Ready-for-Data alone can be asserted while the card is still programming. */
int sd_card_wait_transfer_state(uintptr_t host_base, const struct sd_card *card,
                                uint32_t poll_limit, uint32_t attempts,
                                sd_wait_fn delay_ticks, void *delay_context)
{
    if (!host_base || !card || !card->rca || !poll_limit || !attempts || !delay_ticks)
        return SD_PIO_INVALID_ARGUMENT;
    for (uint32_t i = 0; i < attempts; ++i) {
        uint32_t response[4] = {0, 0, 0, 0};
        uint16_t error = 0;
        int result = command(host_base, CMD13_REQUEST, (uint32_t)card->rca << 16,
                             response, 1, poll_limit, &error);
        if (result) return result;
        if (response[0] & R1_ERROR_MASK) return SD_PIO_CARD_ERROR;
        if (((response[0] >> 9) & 15u) == 4u && (response[0] & R1_READY_FOR_DATA))
            return SD_PIO_OK;
        if (i + 1u < attempts) delay_ticks(delay_context, 1);
    }
    return SD_PIO_TIMEOUT;
}

static int check_card_range(const struct sd_card *card, uint32_t lba,
                            uint32_t sector_count)
{
    if (card == 0 || sector_count == 0 || lba >= card->sector_count ||
        (uint64_t)sector_count > card->sector_count - lba)
        return SD_PIO_ADDRESS_RANGE;
    return SD_PIO_OK;
}

int sd_card_read_blocks(uintptr_t host_base,
                            const struct sd_card *card, uint32_t lba,
                            uint32_t sector_count, uint8_t *buffer,
                            uint32_t poll_limit, sd_wait_fn delay_ticks,
                            void *delay_context, uint32_t *response0,
                            uint16_t *error_status)
{
    if (host_base == 0 || card == 0 || card->rca == 0 || poll_limit == 0 ||
        delay_ticks == 0)
        return SD_PIO_INVALID_ARGUMENT;
    int result = check_card_range(card, lba, sector_count);
    if (result != SD_PIO_OK) return result;
    result = sd_pio_read(host_base, card->ocr_ccs, lba, sector_count,
                             buffer, poll_limit, response0, error_status);
    if (result != SD_PIO_OK) return result;
    return sd_card_wait_ready(host_base, card, poll_limit, delay_ticks,
                                  delay_context);
}

int sd_card_write_blocks(uintptr_t host_base,
                             const struct sd_card *card, uint32_t lba,
                             uint32_t sector_count, const uint8_t *buffer,
                             uint32_t poll_limit, sd_wait_fn delay_ticks,
                             void *delay_context, uint32_t *response0,
                             uint16_t *error_status)
{
    if (host_base == 0 || card == 0 || card->rca == 0 || poll_limit == 0 ||
        delay_ticks == 0)
        return SD_PIO_INVALID_ARGUMENT;
    int result = check_card_range(card, lba, sector_count);
    if (result != SD_PIO_OK) return result;
    result = sd_pio_write(host_base, card->ocr_ccs, lba, sector_count,
                              buffer, poll_limit, response0, error_status);
    if (result != SD_PIO_OK) return result;
    return sd_card_wait_ready(host_base, card, poll_limit, delay_ticks,
                                  delay_context);
}
