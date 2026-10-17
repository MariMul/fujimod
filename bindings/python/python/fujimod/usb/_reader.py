"""Python async iterator lifetime over Rust's owned range reader."""

from .._bridge import call, buffer_out, begin, wait_pending, settled
from ._errors import SessionBusyError


class Reader:
    def __init__(self):
        raise TypeError("use memory.read_chunks() or data.read_chunks()")

    @classmethod
    def _wrap(cls, reader):
        obj = object.__new__(cls)
        obj._js = reader
        obj._pending = None
        return obj

    bytes_completed = property(lambda self: self._js.bytesCompleted)

    def __aiter__(self):
        return self

    async def __anext__(self):
        if self._pending is not None:
            raise SessionBusyError("range reader already has a pending chunk")
        # The JS facade turns its Rust reader's optional chunk into IteratorResult.
        pending = begin(call(self._js.next))
        self._pending = pending
        try:
            value = await wait_pending(pending, self._js.cancelPending)
            if value.done:
                raise StopAsyncIteration
            return buffer_out(value.value)
        finally:
            self._pending = None

    async def aclose(self):
        pending = self._pending
        if pending is not None:
            call(self._js.cancelPending)
            await settled(pending)
        else:
            call(self._js.close)

    async def __aenter__(self):
        return self

    async def __aexit__(self, *_):
        await self.aclose()
        return False

    def __del__(self):
        value = getattr(self, "_js", None)
        if value is not None:
            # No implicit camera I/O, including when iteration ends early.
            value.cancelPending()


Reader.__module__ = "fujimod.usb"
