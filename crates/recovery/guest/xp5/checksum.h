#ifndef RECOVERY_CHECKSUM_H
#define RECOVERY_CHECKSUM_H
/* Start with 0; pass the returned CRC to continue over another chunk. */
unsigned int recovery_crc32_update(unsigned int previous, const void *data, unsigned int size);
unsigned int recovery_crc32(const void *data, unsigned int size);
#endif
