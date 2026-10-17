# Upstream dependencies

Pinned third-party source checkouts live here, separate from FujiMod-owned
code. Initialize only the dependencies needed for the work at hand.

| Checkout | Used by | Needed when |
| --- | --- | --- |
| `json-schema-to-c` | `mods/lib/settings/generate.py` | Regenerating or verifying committed settings C; includes its own pinned jsmn dependency |

Normal builds and host settings tests use checked-in generated C and do not
require these checkouts. FujiMod keeps its own dependencies so it can build
independently of the private X70 View workspace and that workspace's `repos/`.
