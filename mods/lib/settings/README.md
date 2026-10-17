# Settings schema generator

This component integrates the upstream
[JSON Schema to C generator](https://github.com/badicsalex/json_schema_to_c)
as a pinned Git submodule, including its pinned jsmn tokenizer. It is the
candidate foundation for schema-defined FujiMod settings, recipes and films.
It is **not yet qualified to replace the live settings parsers**. The first
qualification found correctness gaps below; no card format or mod has been
migrated and no compatibility layer was added.

## Build integration

From the FujiMod repository root:

```sh
git submodule update --init --recursive -- mods/lib/settings/upstream
python3 mods/lib/settings/generate.py mods/lib/settings/tests/section.schema.json \
  --out mods/lib/settings/build/example
python3 -m unittest discover -s mods/lib/settings/tests -v
python3 mods/lib/settings/qualify.py
```

The final command currently exits 1: it is the adoption gate, not a smoke
test that treats known defects as acceptable behavior. All generated C,
headers, host executables and reports stay in ignored `build/` directories.

`generate.py` invokes upstream unchanged. It checks both dependency revisions
and refuses modified dependency checkouts. It records generator/tokenizer
revisions, hashes of every loaded schema and hashes of the generated `.c/.h`
in `generation.json`. Local schema references work; additional authorized
schema roots are explicit `--authorized-path` arguments. Python's standard
library is sufficient. The generator and generated runtime retain their
upstream MIT licenses; jsmn has its own MIT notice in the dependency.

Pinned inputs:

- Generator: `570075656a080a6756add4dc209d8f8d5ed4c0da`.
- jsmn: `85695f3d5903b1cd5b4030efe50db3b4f5f3c928`.

## Qualification

`tests/section.schema.json` is a representative **test fixture**, not the
final user-facing settings schema. It exercises booleans, bounded integers,
a string enum, an owned 32-byte string, a nested object and a bounded array.

The host tests compile actual upstream-generated C with address/undefined
behavior sanitizers. They establish:

- Strings are inline `char[33]`; arrays are inline bounded storage.
- Named choices become C enums.
- Omitted values get schema defaults, including an omitted nested object.
- An empty optional array has count zero.
- Bad known-field values, unknown fields and duplicate known fields fail.
- A caller-owned candidate can be published with a struct copy. Overwriting
  the JSON input after parsing does not change its copied strings.
- Generation is reproducible for the same schema and dependency revisions.

The negative qualification checks found these **adoption blockers**:

| Input / requirement | Current upstream behavior |
| --- | --- |
| `{"author":"A\nB"}` | Stores literal backslash+n instead of a newline |
| `{"enabled":truth}` | Accepts it as true |
| `{"hold_ms":0400}` | Accepts the non-JSON leading-zero number |
| `{"enabled":true,}` | Accepts the trailing comma |
| `{} {}` | Accepts the first document without rejecting the second |

These are exposed as five expected-failure regressions against the desired
behavior. `qualification.json` separately says `ready: false`; passing the
smoke/regression test runner does not qualify the parser. The missing-comma
case is already rejected. This list is not an exhaustive JSON conformance
assessment. In particular Unicode length/decoding, numeric overflow and
unknown-section preservation need further qualification.

Relevant upstream implementation locations at the pinned revision:

- `js2c_builtins.h:builtin_parse_string` copies raw token bytes.
- `js2c_builtins.h:builtin_parse_bool` checks only the first character.
- Integer conversion uses `strtoll`/`strtoull`; number conversion uses
  `strtod`. These also need a freestanding target integration.
- `js2c_builtins.h:builtin_parse_json_string` tokenizes with jsmn without
  requiring one complete strict JSON document.
- The root generator's public API is a parser API. There is no generated
  serializer, separate default initializer, transactional publication or
  settings persistence framework.

### Freestanding ARM compile check

The generated fixture was also passed to the current private tree's native
GCC 4.6.4 ARM compiler with `-std=gnu99 -Os -marm -march=armv7-a
-mfloat-abi=soft -ffreestanding -ffunction-sections -fdata-sections`.
Compilation stops at upstream's `#include <inttypes.h>`: that header is not
provided by the freestanding toolchain. This was a compile check only, not
an ARM execution or successful firmware link. The numeric conversion calls
listed above remain dependencies to resolve after the header boundary.

## Intended runtime ownership

Each mod/section owns a bounded generated struct. Decode into a separate
candidate, validate relationships, then publish using that mod's task-safe
application function. Generated parsers return **true on error**, and can
partially modify their output before failing; callers must not decode into
live state. There is no string destructor: these structs own inline arrays,
not individual heap allocations. No pointer into the input is retained for
ordinary string fields. Upstream's optional `raw` reference representation is
not suitable for a live setting after the input buffer is reused.

JSON Schema is the source of definitions, not a document interpreted on the
camera. Build-time per-mod section schemas will compose the shared SETTINGS
file, while AF remains its own experimental file. Recipes and films will
share processor-override rules. Stock settings and stock Q/Fn assignments
remain in stock persistence; only custom-feature overrides belong in SETTINGS.

Before live adoption: fix/requalify the parser issues, supply serialization
and the freestanding numeric dependencies, then integrate section registration,
preservation and card persistence. These are explicit remaining work, not
capabilities provided by the upstream generator.
