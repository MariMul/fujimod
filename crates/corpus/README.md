# fujimod-corpus

Generic ARM32 (A32) and AArch64 decoding, function features, combined firmware
families, and locator queries. Boundaries, mapped bytes, pointer windows, names,
types, readable C, notes, and external reference information are caller inputs.
This crate contains no camera maps or firmware-derived fixtures.

`Index::add_unit(UnitInput, Vec<Region>)` accepts any boundary provider; all byte
ranges are half-open. The JSON input contract is
[`schema/unit.schema.json`](schema/unit.schema.json). Regions are passed separately
as addresses and byte arrays. Overlapping mappings require distinct units.
Unmapped function bodies and duplicate units are errors. ARM32 means A32;
Thumb boundaries require a future explicit instruction-set selector.

`Index::build()` indexes all supplied units together. Unique normalized code
(at least four instructions per function) seeds families across firmwares.
Duplicated code waits for aligned call-site evidence. Calls resolve first in
the caller's unit, then at a unique address in its firmware. Ambiguous targets
remain unresolved. A family has at most one member per firmware; conflicting
proposals are left unmatched. This deliberately prefers incomplete families to
silently merging two different functions.

Matching retains branch shapes, conditions, field offsets and constants, but
removes register allocation, branch destinations, literal addresses, stack
slots and split address immediates. A32 token spelling and no-junk token-diff
alignment preserve the earlier clone pairer's behavior. Features include
normalized SHA-256, instruction trigrams, referenced strings, constants, data
references and call edges. Rare strings plus high trigram agreement and shared
constants supply additional seeds. Cross-ISA feature seeds require two shared
strings and two nontrivial constants; these are heuristic call-site-grade
matches, never equal-code evidence. Weak/dissimilar call-site candidates are
retained as evidence links, but never merge families or propagate names/calls.

`pairs(reference, other)` projects the combined families into `identical`,
`same-code`, `call-site`, and `weak` evidence. `identical` means equal normalized
code with consistently resolved callees; it does **not** mean byte equality.
`same-code` has equal normalized code but incomplete callee evidence.

Locators can find by name/address, select family members in a firmware, follow
calls and data references (including reverse references), read family
annotations, and search mapped units with byte masks. `names` retains aliases
and donor provenance; callers can require equal normalized code. Metadata
annotations use explicit `kind`, `text`, and `source` fields, so names/types,
readable C, research notes, and locator reports remain private caller data.

`save`/`open` use a versioned internal binary cache, not a portable interchange
contract. Keep populated caches private. Rebuilding/addition changes numeric
function/family ids; persist `(firmware, unit, address)` keys in locator reports.
Rebuild after adding units, or `merge` indexes and rebuild automatically.

Python uses `from fujimod.corpus import Index`, with the same operations and
JSON-compatible dictionaries. Build the extension through the existing
`bindings/python` package. Tests use only synthetic listings and instructions;
all test output goes under `target/corpus-tests`.
