# Shared settings framework: design and implementation status

The checked-in generated files currently implement one qualification fixture,
not the shared SETTINGS framework. In particular there is no mod section
registry, document merger, JSON writer or card persistence integration here.
The existing live settings implementation is unchanged. This document records
the intended responsibilities so build integration is not mistaken for a
finished framework.

| Concern | Implemented now | Required before migration |
| --- | --- | --- |
| Bounded structs, enum decoding, defaults | Generated fixture and host tests | Real per-mod schemas and target compilation |
| Generated sources usable without dependencies | Committed C/header; separate CI regeneration check | Apply this policy to every real schema |
| Shared document with contributed sections | No | Section registry, schema composition, preservation and tests |
| Safe publication on decode error | Host fixture keeps its live sentinel | Per-section publication through each mod's task-safe application function |
| Detailed parse errors | Upstream returns a boolean, true on error | Framework error context and improved parser diagnostics |
| JSON serialization | No upstream serializer; none added here | Schema-derived writers and round-trip tests |
| Card persistence | No new path | Verified storage operation sequence and failure handling |

## Section composition

Each mod should contribute its own section schema and generated bounded C
type. A build manifest should assign each section a unique top-level JSON
name and identify its owner and parser/writer. Duplicate registrations must
fail the build. The build should produce a complete editor-validation schema
whose root `properties` reference those section schemas. Composing that
schema does not require one permanent C struct containing every mod's state.

At runtime the framework should parse the document envelope, locate each
registered section and decode it into that section's candidate struct. The
owner then validates relationships and publishes it using its task-safe
application function. Framework code should handle document I/O once; mods
should not each rewrite the shared file independently.

Sections belonging to absent mods must survive saves, so a build with fewer
mods does not erase their configuration. The document envelope must preserve
unregistered sections while each registered parser can validate its own
section strictly. The current fixture rejects unknown properties and does
not provide this envelope or preservation behavior.

This shared document contains ordinary settings, recipe slot assignments
and only custom-feature Q/Fn overrides. Stock assignments continue through
stock persistence. AF remains a separate experimental file.

## Defaults and overrides

Missing fields use documented schema defaults. This does not make malformed
or out-of-range supplied fields valid. For recipes and films, choose the
requested variant, merge the matching processor override recursively into
its base object, replacing supplied arrays as whole values, then apply
defaults and validate. Applying defaults to the override before merging
would incorrectly replace unspecified base values. No such override merger
exists in the current fixture.

## Errors

Upstream returns `true` on error and can partially change its output. The
fixture initializes a separate candidate, calls the parser and copies it to
live storage only on success. Strings are owned inline, so rejecting a
candidate or replacing live state does not require freeing strings.

The framework still needs explicit outcomes for I/O failure, malformed JSON,
schema/range failure, unsupported values and output-capacity exhaustion. It
should report the operation and section; a property path or input offset
requires additional parser support because the generated boolean alone
cannot supply one.

Malformed document syntax must prevent any publication from that document.
Within a syntactically valid envelope, an invalid registered section should
retain its previous live state while independent valid sections can load.
On initial load without previous state, its documented defaults remain in
effect. Invalid stored sections must not be silently replaced with defaults
by an unrelated section's save. These are required framework semantics,
not behaviors demonstrated by the one-section fixture.

## Writing JSON and saving

The selected upstream tool generates parsers, not serializers. We need
schema-derived serialization beside each generated parser so property
names, enum strings and bounds come from the same source. The framework
must compose registered sections and preserved sections into one document.
Each writer must escape JSON strings, emit schema enum names rather than
C enum ordinals, and fail explicitly when an output buffer is too small or
a struct contains an invalid value. It must not produce a truncated document
and call that success.

Before any new card write path is enabled, require parse/write/parse tests
for defaults, every enum, strings and escapes, nested objects and arrays,
and preservation tests across builds with different registered sections.
The current parser's known string and strict-JSON defects must be resolved
first; checking generated output into Git does not resolve them.

Serialization must finish successfully before replacing the saved document.
The storage integration needs a verified write/replace protocol using the
camera's available file operations, explicit handling of short writes and
no-card/full/write-protected failures, and retention of dirty state after a
failed save. We have not established an atomic replacement protocol here
and should not promise one based on desktop filesystem behavior.
