"""Compositions delegated to the Rust diagnostic-data operations."""

from dataclasses import dataclass
from . import Session, Reader, DataWindow, DataWriteMode, DiagnosticReply
from .._bridge import (
    jsusb,
    uint,
    unwrap,
    enum_in,
    buffer_in,
    buffer_out,
    call,
    snapshot,
)


@dataclass(frozen=True)
class UploadReply:
    setup: DiagnosticReply
    acknowledgement: bytes
    bytes_written: int

    @classmethod
    def _from_js(cls, value):
        return snapshot(
            value,
            lambda v: cls(
                DiagnosticReply._from_js(v.setup),
                buffer_out(v.acknowledgement),
                v.bytesWritten,
            ),
        )


async def upload(session, data, *, piece_bytes, mode):
    return await session._call(
        jsusb.data.upload,
        unwrap(session, Session),
        buffer_in(data),
        uint(piece_bytes),
        enum_in(mode, DataWriteMode),
        decode=UploadReply._from_js,
    )


def read_chunks(session, window, address, length):
    return Reader._wrap(
        call(
            jsusb.data.readChunks,
            unwrap(session, Session),
            unwrap(window, DataWindow),
            uint(address),
            uint(length),
        )
    )
