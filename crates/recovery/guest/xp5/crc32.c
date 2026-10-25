/* CRC32/IEEE reflected polynomial; constants below are arithmetic, not firmware addresses. */
#include "checksum.h"
unsigned int recovery_crc32_update(unsigned int previous, const void *data, unsigned int size)
{
    const unsigned char *p = data;
    static const unsigned int table[16] = {0,498536548u,997073096u,651767980u,
        1994146192u,1802195444u,1303535960u,1342533948u,3988292384u,4027552580u,
        3604390888u,3412177804u,2607071920u,2262029012u,2685067896u,3183342108u};
    unsigned int crc = ~previous;
    while (size--) {
        crc ^= *p++;
        crc = (crc >> 4) ^ table[crc & 15];
        crc = (crc >> 4) ^ table[crc & 15];
    }
    return ~crc;
}

unsigned int recovery_crc32(const void *data, unsigned int size)
{
    return recovery_crc32_update(0, data, size);
}
