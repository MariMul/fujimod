#include "sd_host.h"

#define SDHCI_CLOCK_CONFIGURATION 0x01e0u
#define SDHCI_CLOCK_CONTROL 0x01e4u
#define SDHCI_PRESENT_STATE 0x0024u
#define SDHCI_INTERFACE_CONTROL 0x0028u
#define SDHCI_HANDSHAKE_CONTROL 0x002cu
#define SDHCI_INTERRUPT_ENABLE 0x0034u
#define SDHCI_ERROR_ENABLE 0x0036u
#define SDHCI_INTERRUPT_SIGNAL_ENABLE 0x0038u
#define SDHCI_ERROR_SIGNAL_ENABLE 0x003au
#define SDHCI_SECONDARY_INTERFACE_CONTROL 0x003eu
#define SDHCI_SOFTWARE_RESET 0x002eu
#define SDHCI_TIMEOUT_CONTROL 0x002eu
#define SDHCI_SECOND_HANDSHAKE_MODE 0xfa00u
#define SDHCI_HOST_CONTROL_FOUR_BIT 0x0002u
#define SDHCI_HOST_CONTROL_HIGH_SPEED 0x0004u
#define SDHCI_HOST_CONTROL_DRIVE_FIELDS 0x00c0u
#define SDHCI_COMMAND_INTERRUPT_MASK 0xee01u
#define SDHCI_ERROR_SIGNAL_MASK 0x17ffu

static volatile uint16_t *reg16(uintptr_t base, uint32_t offset)
{
    return (volatile uint16_t *)(base + offset);
}

static volatile uint32_t *reg32(uintptr_t base, uint32_t offset)
{
    return (volatile uint32_t *)(base + offset);
}

static int wait_handshake(uintptr_t host_base,
                          sd_wait_fn handshake_wait,
                          void *delay_context, int acknowledge)
{
    uint16_t status;

    for (uint32_t attempt = 0; attempt < 1000; ++attempt) {
        status = *reg16(host_base, SDHCI_HANDSHAKE_CONTROL);
        if ((status & 2u) != 0) {
            if (acknowledge)
                *reg16(host_base, SDHCI_HANDSHAKE_CONTROL) = status | 4u;
            return SD_HOST_OK;
        }
        handshake_wait(delay_context, 1);
    }

    return SD_HOST_HANDSHAKE_TIMEOUT;
}

static int wait_reset_bit(uintptr_t host_base, uint16_t reset_bit,
                          sd_wait_fn delay_ticks, void *delay_context)
{
    for (uint32_t attempt = 0; attempt < 5; ++attempt) {
        if ((*reg16(host_base, SDHCI_SOFTWARE_RESET) & reset_bit) == 0)
            return SD_HOST_OK;
        delay_ticks(delay_context, 1);
    }

    return SD_HOST_RESET_TIMEOUT;
}

int sd_host_reset_lines(uintptr_t host_base,
                            sd_wait_fn delay_ticks,
                            void *delay_context)
{
    uint16_t value;

    if (host_base == 0 || delay_ticks == 0)
        return SD_HOST_INVALID_ARGUMENT;

    value = *reg16(host_base, SDHCI_SOFTWARE_RESET);
    *reg16(host_base, SDHCI_SOFTWARE_RESET) =
        (uint16_t)((value & 0xfaffu) | 0x0200u);
    if (wait_reset_bit(host_base, 0x0200u, delay_ticks, delay_context) !=
        SD_HOST_OK)
        return SD_HOST_RESET_TIMEOUT;

    value = *reg16(host_base, SDHCI_SOFTWARE_RESET);
    *reg16(host_base, SDHCI_SOFTWARE_RESET) =
        (uint16_t)((value & 0xfcffu) | 0x0400u);
    return wait_reset_bit(host_base, 0x0400u, delay_ticks, delay_context);
}

int sd_host_initialize_registers(uintptr_t host_base,
                                     uint16_t timeout_setting,
                                     sd_wait_fn delay_ticks,
                                     sd_wait_fn handshake_wait,
                                     void *delay_context)
{
    if (host_base == 0 || timeout_setting > 15 || delay_ticks == 0 || handshake_wait == 0)
        return SD_HOST_INVALID_ARGUMENT;

    uint32_t value = *reg32(host_base, SDHCI_CLOCK_CONFIGURATION);
    *reg32(host_base, SDHCI_CLOCK_CONFIGURATION) = value | 0x00010000u;

    value = *reg32(host_base, SDHCI_CLOCK_CONTROL);
    *reg32(host_base, SDHCI_CLOCK_CONTROL) = value | 0x00ff0101u;

    value = *reg32(host_base, SDHCI_CLOCK_CONFIGURATION);
    *reg32(host_base, SDHCI_CLOCK_CONFIGURATION) =
        (value & 0xfffff800u) | (value & 0x000001ffu) | 0x00000400u;

    value = *reg32(host_base, SDHCI_CLOCK_CONFIGURATION);
    *reg32(host_base, SDHCI_CLOCK_CONFIGURATION) = value & 0xfffeffffu;

    uint32_t attempt;
    for (attempt = 0; attempt < 33; ++attempt) {
        if ((*reg32(host_base, SDHCI_PRESENT_STATE) & 0x00010000u) != 0)
            break;
        delay_ticks(delay_context, 30);
    }
    if (attempt == 33) return SD_HOST_CLOCK_TIMEOUT;

    value = *reg16(host_base, SDHCI_HANDSHAKE_CONTROL);
    *reg16(host_base, SDHCI_HANDSHAKE_CONTROL) = (uint16_t)(value | 1u);
    if (wait_handshake(host_base, handshake_wait, delay_context, 0) !=
        SD_HOST_OK)
        return SD_HOST_HANDSHAKE_TIMEOUT;

    uint16_t interface = *reg16(host_base, SDHCI_INTERFACE_CONTROL);
    /* Stock preserves the inherited width and speed bits. */
    *reg16(host_base, SDHCI_INTERFACE_CONTROL) =
        (uint16_t)((interface & 0xf000u) |
                   (interface & 0x01ffu) |
                   0x0100u | (7u << 9));

    uint16_t secondary = *reg16(host_base, SDHCI_SECONDARY_INTERFACE_CONTROL);
    *reg16(host_base, SDHCI_SECONDARY_INTERFACE_CONTROL) =
        (uint16_t)(secondary & 0xfff8u);
    delay_ticks(delay_context, 5);

    *reg16(host_base, SDHCI_HANDSHAKE_CONTROL) = 1u;
    *reg16(host_base, SDHCI_HANDSHAKE_CONTROL) =
        SDHCI_SECOND_HANDSHAKE_MODE | 1u;
    if (wait_handshake(host_base, handshake_wait, delay_context, 1) !=
        SD_HOST_OK)
        return SD_HOST_HANDSHAKE_TIMEOUT;

    uint16_t timeout_control = *reg16(host_base, SDHCI_TIMEOUT_CONTROL);
    *reg16(host_base, SDHCI_TIMEOUT_CONTROL) =
        (uint16_t)((timeout_control & 0xfff0u) |
                   timeout_setting);
    interface = *reg16(host_base, SDHCI_INTERFACE_CONTROL);
    *reg16(host_base, SDHCI_INTERFACE_CONTROL) =
        (uint16_t)(interface | SDHCI_HOST_CONTROL_FOUR_BIT |
                   SDHCI_HOST_CONTROL_DRIVE_FIELDS);

    /* Stock 0x00934980's command-mode interrupt setup. */
    *reg16(host_base, SDHCI_INTERRUPT_ENABLE) =
        SDHCI_COMMAND_INTERRUPT_MASK;
    *reg16(host_base, SDHCI_ERROR_ENABLE) = 0xffffu;
    *reg16(host_base, SDHCI_INTERRUPT_SIGNAL_ENABLE) =
        SDHCI_COMMAND_INTERRUPT_MASK;
    *reg16(host_base, SDHCI_ERROR_SIGNAL_ENABLE) = SDHCI_ERROR_SIGNAL_MASK;
    return SD_HOST_OK;
}
