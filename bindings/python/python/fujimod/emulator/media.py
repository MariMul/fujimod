"""Create external card inputs through the Rust storage library."""
import json
from fujimod._native.emulator import CardMedium


def format_card(files=(), *, sectors=0x800000, partition_start=8192,
                sectors_per_cluster=64, label='CAMERA', read_only=(), max_dirty_sectors=1 << 21):
    return CardMedium.formatted(json.dumps(dict(sectors=sectors,
        max_dirty_sectors=max_dirty_sectors, partition_start=partition_start,
        sectors_per_cluster=sectors_per_cluster, label=label, read_only_files=list(read_only))), list(files))


def load_card_image(path, *, sectors=0x1000000, max_dirty_sectors=1 << 21):
    """Copy a bounded external raw image into a new, detached Rust medium."""
    from pathlib import Path
    import hashlib
    path = Path(path)
    size = path.stat().st_size
    if not size or size % 512 or size > sectors * 512:
        raise ValueError('Raw card image must contain whole sectors within capacity')
    card = CardMedium.blank(json.dumps(dict(sectors=sectors, max_dirty_sectors=max_dirty_sectors)))
    digest = hashlib.sha256()
    count = 0
    offset = 0
    with path.open('rb') as source:
        while block := source.read(1 << 20):
            digest.update(block)
            if any(block):
                card.write_sectors(offset // 512, block)
                count += sum(any(block[i:i+512]) for i in range(0,len(block),512))
            offset += len(block)
    if offset != size:
        raise ValueError('Raw card image changed during import')
    return card, dict(path=str(path), bytes=size, sectors=size//512,
                      nonzero_sectors=count, sha256=digest.hexdigest())
