# Camera remote control

`fujimod-remote` resolves immutable menu, option and physical-control references
against catalog facts and live memory. Navigation and selection use native
inputs. Reads return typed snapshots without advancing a board.

`Remote` has four asynchronous, local-future primitives: `set_input`, `read`,
`frame` and `wait`. The `operations` module composes navigation, selection,
presses, turns, predicate waits and screenshots over those primitives. Predicate
waits retain the store/device reads made by their predicate. Callers supply
absolute deadlines in the connection's clock and own their assertions.

Build `Facts` with `Facts::from_selection(&selection, manifest)`. A reference is
resolved on every use; duplicate or missing rows and option names produce an
error with the candidates found. Stock menu IDs are root indices or catalog
roles. Use `via` to identify a shared submenu occurrence. Mod rows and stable
option identities come from the installed build's menu manifest.

`BoardRemote::new(session, facts, options)` borrows or owns an in-process
`Session`. Its checked physical panel and contact acknowledgment driver must
already be connected. Continuations use exact store/device watches and yield
between bounded instruction windows. Checkpoints, restoring, scene input and
explicit clock advancement are board-only methods. Frame observations use the
selected scanout and a completed-frame counter from the board's device graph.

`Ff80Remote::new(session, facts, options)` borrows or owns an existing USB
session. Options supply firmware transport limits and polling cadence. It uses
range reads and explicit debug command bindings; native scanner names are never
converted into debug command names. An uncertain or failed command is propagated
without replay. Its screenshot completion witness is a newly published
framebuffer pointer, so an unchanged pointer can reach the deadline; passive
`frame()` can still capture the current buffer. Physical identity, permission
and session entry/exit belong to the caller.

Python exposes the same futures through `fujimod.remote.Camera` and
`camera.blocking`. `menu(id, key=..., via=...)`, `option(item, name)` and
`control(id)` create references without a connection. Async operations yield to
Python's event loop on the owning thread. Overlapping operations refuse, and
canceling an operation releases its camera lease. A USB camera leases its
existing session until `camera.close()`.

```python
from fujimod.remote import menu, option

quality = menu("shooting", key=61)
await camera.navigate(quality, deadline=deadline)
await camera.select(option(quality, "NORMAL"), deadline=deadline)
current = camera.blocking.read(quality)
capture = await camera.screenshot(deadline=deadline)
capture.frame.save("quality.png")
```

The `testing` feature supplies a synthetic transport for binding tests. Unit and
USB replay tests use synthetic facts and memory only.
