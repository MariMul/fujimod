# Shared FujiMod definitions

`fujimod-defs` loads validated firmware data without USB or emulator dependencies.
The facade exposes it as `fujimod::defs`; enabling `usb` also exposes `defs`.
Definitions stay in JSON rather than becoming another set of Rust constants.
Callers supply the selected firmware ID and SHA-256 when loading a definition.
Matching those values qualifies the data; identifying a connected camera and
checking execution eligibility remain responsibilities of its workflow.

The first schema is `service::Definition` for App80. It contains firmware
identity, an explicit ff80 mailbox location/limit, and the ordered service table.
Each entry preserves its command byte, label, callback-kind word, handler and
parameter-table addresses, mode availability and table-declared transition.
Duplicate command bytes remain separate rows in firmware order. A mode rule
can be reviewed, disabled, or explicitly unreviewed. Unreviewed rows are visible
but cannot be selected for execution through this catalogue.

`Codec` selects an implemented argument/result layout. Its slot array gives
zero-based positions within the 32-byte argument mailbox; invalid, overlapping
or incomplete layouts are rejected. Labels are display text and never select
behavior. `Research` retains a command whose argument semantics have not been
qualified for a typed codec. Unknown callback-kind words remain data and never
become host completion instructions.

The JSON schema is version 1. It requires:

- `firmware`: `id`, `model`, `version`, `sha256`;
- `mailbox`: `page`, `offset`, `configuration_limit`;
- `entries`: `code`, `label`, `codec`, `slots`, `enabled_modes`, `next_mode`,
  `firmware_kind`, `handler`, `parameter_table`;
- optional `source`: `format` (`dispatch24` or `declarations12`), `records`,
  `table`, `table_sha256`, `dispatcher`.

`enabled_modes` is a nonzero mask, zero for a disabled row, or null for an
unreviewed rule. The public API represents those cases with `ModeAvailability`.
`next_mode` is the firmware table's pre-dispatch update, represented by
`ModeTransition`; handlers can have additional effects. All numeric JSON fields
are unsigned quantities. No body, command number, mailbox or firmware is chosen
by default. Private extensions can supply this same schema to USB tooling,
emulators and future SDK generation.
