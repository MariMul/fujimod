#ifndef AARCH64_BOOT_H
#define AARCH64_BOOT_H
#include <stdint.h>
#include <stddef.h>
extern volatile uint32_t a64_boot_ready;
void a64_boot_zero_gate(void);
/* A physical destination of zero is valid; copying uses explicit A64 stores. */
void a64_copy_to_physical(uintptr_t destination, const void *source, size_t count);
#endif
