#include "fat32.h"

static uint16_t le16(const uint8_t *p)
{
    return (uint16_t)p[0] | (uint16_t)p[1] << 8;
}

static uint32_t le32(const uint8_t *p)
{
    return (uint32_t)le16(p) | (uint32_t)le16(p + 2) << 16;
}

static int cluster_valid(const struct fat32_volume *v, uint32_t cluster)
{
    return cluster >= 2 && cluster - 2 < v->clusters;
}

static int blocks(const struct fat32_volume *v, uint32_t lba, uint32_t count, uint8_t *buffer)
{
    if (!count || lba < v->partition || (uint64_t)lba + count > (uint64_t)v->partition + v->sectors)
        return FAT32_INVALID;
    return v->read(v->context, lba, count, buffer) == 0 ? FAT32_OK : FAT32_IO;
}

static int next_cluster(const struct fat32_volume *v, uint32_t current, uint32_t *next, uint8_t *scratch)
{
    if (!cluster_valid(v, current) || current / 128 >= v->fat_sectors)
        return FAT32_CHAIN;
    int result = blocks(v, v->fat + current / 128, 1, scratch);
    if (result) return result;
    *next = le32(scratch + (current % 128) * 4) & 0x0fffffffu;
    if (*next >= 0x0ffffff8u) return FAT32_NOT_FOUND;
    return cluster_valid(v, *next) && *next < 0x0ffffff0u ? FAT32_OK : FAT32_CHAIN;
}

int fat32_mount(struct fat32_volume *v, fat32_read_fn read, void *context,
                uint32_t card_sectors, uint8_t scratch[512])
{
    if (!v || !read || !scratch || !card_sectors) return FAT32_INVALID;
    if (read(context, 0, 1, scratch)) return FAT32_IO;
    if (le16(scratch + 510) != 0xaa55) return FAT32_INVALID;
    uint32_t partition = 0, limit = card_sectors;
    /* A BPB with 512-byte sectors identifies a volume directly at LBA zero. */
    if (le16(scratch + 11) != 512) {
        unsigned found = 0;
        for (unsigned i = 0; i < 4; ++i) {
            const uint8_t *p = scratch + 446 + i * 16;
            if (p[4] != 0x0b && p[4] != 0x0c) continue;
            if (++found != 1) return FAT32_INVALID;
            partition = le32(p + 8);
            limit = le32(p + 12);
        }
        if (!found || !partition || !limit || (uint64_t)partition + limit > card_sectors)
            return FAT32_INVALID;
        if (read(context, partition, 1, scratch)) return FAT32_IO;
    }
    if (le16(scratch + 510) != 0xaa55 || le16(scratch + 11) != 512
            || le16(scratch + 17) || le16(scratch + 22) || le16(scratch + 42))
        return FAT32_INVALID;
    uint32_t spc = scratch[13], reserved = le16(scratch + 14), fats = scratch[16];
    uint32_t total = le16(scratch + 19), fat_size = le32(scratch + 36);
    if (!total) total = le32(scratch + 32);
    if (!spc || spc > 128 || (spc & (spc - 1)) || !reserved || !fats || fats > 2
            || !fat_size || !total || total > limit)
        return FAT32_INVALID;
    uint64_t data_offset = (uint64_t)reserved + (uint64_t)fats * fat_size;
    if (data_offset >= total) return FAT32_INVALID;
    uint32_t clusters = (total - (uint32_t)data_offset) / spc;
    if (clusters < 65525 || clusters > 0x0fffffeeu || (uint64_t)clusters + 2 > (uint64_t)fat_size * 128)
        return FAT32_INVALID;
    uint32_t root = le32(scratch + 44) & 0x0fffffffu;
    uint16_t flags = le16(scratch + 40);
    uint32_t selected_fat = flags & 0x80 ? flags & 15 : 0;
    if (selected_fat >= fats || root < 2 || root - 2 >= clusters)
        return FAT32_INVALID;
    v->read = read;
    v->context = context;
    v->card_sectors = card_sectors;
    v->partition = partition;
    v->sectors = total;
    v->fat = partition + reserved + selected_fat * fat_size;
    v->fat_sectors = fat_size;
    v->data = partition + (uint32_t)data_offset;
    v->clusters = clusters;
    v->root = root;
    v->sectors_per_cluster = spc;
    return FAT32_OK;
}

int fat32_open_root(const struct fat32_volume *v, const char name[11],
                    struct fat32_file *file, uint8_t scratch[512])
{
    if (!v || !name || !file || !scratch) return FAT32_INVALID;
    uint32_t cluster = v->root;
    for (uint32_t visited = 0; visited < v->clusters; ++visited) {
        if (!cluster_valid(v, cluster)) return FAT32_CHAIN;
        uint32_t lba = v->data + (cluster - 2) * v->sectors_per_cluster;
        for (uint32_t sector = 0; sector < v->sectors_per_cluster; ++sector) {
            int result = blocks(v, lba + sector, 1, scratch);
            if (result) return result;
            for (unsigned offset = 0; offset < 512; offset += 32) {
                const uint8_t *p = scratch + offset;
                if (!p[0]) return FAT32_NOT_FOUND;
                if (p[0] == 0xe5 || p[11] & 0x18 || p[11] == 0x0f) continue;
                unsigned n = 0;
                while (n < 11 && p[n] == (uint8_t)name[n]) ++n;
                if (n != 11) continue;
                uint32_t first = ((uint32_t)le16(p + 20) << 16 | le16(p + 26)) & 0x0fffffffu;
                uint32_t size = le32(p + 28);
                if (!size || !cluster_valid(v, first)) return FAT32_INVALID;
                file->size = size;
                file->position = 0;
                file->cluster = first;
                file->sector_in_cluster = 0;
                file->clusters_seen = 1;
                return FAT32_OK;
            }
        }
        int result = next_cluster(v, cluster, &cluster, scratch);
        if (result) return result;
    }
    return FAT32_CHAIN;
}

int fat32_next_extent(const struct fat32_volume *v, struct fat32_file *file, uint32_t max_sectors,
                      uint32_t *lba, uint32_t *sectors, uint8_t scratch[512])
{
    if (!v || !file || !lba || !sectors || !scratch || !max_sectors
            || file->position > file->size || (file->position != file->size && file->position % 512))
        return FAT32_INVALID;
    *sectors = 0;
    if (file->position == file->size) return FAT32_OK;
    if (file->sector_in_cluster == v->sectors_per_cluster) {
        if (file->clusters_seen >= v->clusters) return FAT32_CHAIN;
        int result = next_cluster(v, file->cluster, &file->cluster, scratch);
        if (result) return result == FAT32_NOT_FOUND ? FAT32_CHAIN : result;
        ++file->clusters_seen;
        file->sector_in_cluster = 0;
    }
    if (!cluster_valid(v, file->cluster)) return FAT32_CHAIN;
    uint32_t first = v->data + (file->cluster - 2) * v->sectors_per_cluster + file->sector_in_cluster;
    uint32_t remaining = file->size - file->position;
    uint32_t count = v->sectors_per_cluster - file->sector_in_cluster;
    if (count > max_sectors) count = max_sectors;
    if (count > (remaining + 511) / 512) count = (remaining + 511) / 512;
    if (first < v->data || (uint64_t)first + count > (uint64_t)v->partition + v->sectors)
        return FAT32_CHAIN;
    uint32_t bytes = count * 512 < remaining ? count * 512 : remaining;
    file->position += bytes;
    file->sector_in_cluster += count;
    *lba = first;
    *sectors = count;
    return FAT32_OK;
}

int fat32_read(const struct fat32_volume *v, struct fat32_file *file,
               uint8_t *buffer, size_t capacity, size_t *received, uint8_t scratch[512])
{
    if (!v || !file || !buffer || !received || !scratch || capacity < 512 || file->position > file->size)
        return FAT32_INVALID;
    *received = 0;
    while (file->position < file->size && capacity >= 512) {
        uint32_t start = file->position, remaining = file->size - start, lba, sectors;
        /* Whole sectors go straight to the buffer; a final partial sector goes alone through scratch. */
        uint32_t limit = remaining >= 512 ? remaining / 512 : 1;
        if (limit > capacity / 512) limit = (uint32_t)(capacity / 512);
        int result = fat32_next_extent(v, file, limit, &lba, &sectors, scratch);
        if (result) return result;
        uint32_t bytes = file->position - start;
        if (bytes % 512) {
            result = blocks(v, lba, 1, scratch);
            if (result) return result;
            for (uint32_t i = 0; i < bytes; ++i) buffer[i] = scratch[i];
        } else {
            result = blocks(v, lba, sectors, buffer);
            if (result) return result;
        }
        buffer += bytes;
        capacity -= bytes;
        *received += bytes;
    }
    return FAT32_OK;
}
