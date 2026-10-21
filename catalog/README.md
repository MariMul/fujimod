# Reference catalog

The catalog records reusable hardware and firmware facts, keyed by identity.
Each fact has one owner. Consumers resolve references by ID; documents never
store another document's file path. Input file locations belong to callers.

| Layer | Location | Owns |
| --- | --- | --- |
| Processor | `processors/<id>/` | CPU/reset architecture, address/block maps, interrupt lines, clock domains and processor interconnects |
| Body | `bodies/<id>/` | Processor ID, NAND geometry, sensor, lens, controls/pins, display and physical wiring to processor blocks |
| Firmware | `firmware/<body>/<version>/` | Body/version identity, update and ROM hashes, installation/decoding layout and generated facts consumed by published tools |
| Run configuration | `../configurations/emulator/<id>/` | Body/firmware selection, start inputs, execution mode, scene and small named overlays |

Processor and body records describe the hardware independently of an emulator.
Body records refer to processor block IDs instead of copying block maps or IRQs.
Firmware records refer to their body ID instead of repeating physical wiring.
Reusable component records remain in the body layer and are referenced by
component ID, so identical lens, sensor and control facts are not copied between
bodies. Firmware identity includes its body and version; existing explicit
firmware IDs remain selectors, never file locations.

Run configurations are executable preparation policy outside the reference
catalog. A configuration selects immutable reference identities and explicit
external inputs. REC with USB, service sessions and SD boot are small typed
overlays on a base configuration. An overlay changes declared connection/start
inputs or selects an implementation; it does not duplicate a body or firmware,
rewrite reference facts, or introduce a general patch/workflow language.

Firmware update files, real boot ROM dumps and optional persistent images are
supplied by explicit caller paths and verified against the selected identities.
No reference record locates or downloads those inputs. A real ROM reset entry
remains distinct from a loader or SD entry.

Processor, body, component and firmware records have separate JSON Schemas with
typed fields and closed objects. Run configurations and overlays have their own
schemas alongside their implementation. Schema IDs are stable URIs; CI resolves
them to the schemas in the same checkout, validates every document, and checks
identity uniqueness and cross-layer references without network resolution.
Device/model option variants are checked by their implementation schemas and
Rust constructors; an untyped object is not a substitute for a schema.

Catalog resolution is shared by Rust and the language bindings. Applications
select IDs and supply inputs; they do not reconstruct paths or maintain parallel
copies of facts. Changes migrate all consumers together and remove superseded
layouts, selectors and duplicated mode records.
