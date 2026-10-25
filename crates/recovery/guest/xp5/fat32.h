#ifndef FUJI_FAT32_H
#define FUJI_FAT32_H

#include <stdint.h>
#include <stddef.h>

typedef int (*fat32_read_fn)(void *context, uint32_t lba, uint32_t sectors, uint8_t *buffer);

enum fat32_result {
    FAT32_OK = 0, FAT32_IO = -1, FAT32_INVALID = -2,
    FAT32_NOT_FOUND = -3, FAT32_CHAIN = -4,
};

struct fat32_volume {
    fat32_read_fn read;
    void *context;
    uint32_t card_sectors, partition, sectors, fat, fat_sectors;
    uint32_t data, clusters, root, sectors_per_cluster;
};

struct fat32_file {
    uint32_t size, position, cluster, sector_in_cluster, clusters_seen;
};

int fat32_mount(struct fat32_volume *volume, fat32_read_fn read, void *context,
                uint32_t card_sectors, uint8_t scratch[512]);
int fat32_open_root(const struct fat32_volume *volume, const char name[11],
                    struct fat32_file *file, uint8_t scratch[512]);
int fat32_read(const struct fat32_volume *volume, struct fat32_file *file,
               uint8_t *buffer, size_t capacity, size_t *received, uint8_t scratch[512]);
/* Map the file's next sectors, at most max_sectors within one cluster, to card
 * LBAs and advance past them. A final partial sector counts as one sector.
 * Returns FAT32_OK with *sectors zero at end of file. */
int fat32_next_extent(const struct fat32_volume *volume, struct fat32_file *file, uint32_t max_sectors,
                      uint32_t *lba, uint32_t *sectors, uint8_t scratch[512]);

#endif
