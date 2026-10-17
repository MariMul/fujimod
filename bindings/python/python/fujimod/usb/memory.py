"""Compositions delegated to the Rust memory operations."""

from . import Session, Reader
from .._bridge import jsusb, uint, unwrap, buffer_in, buffer_out, call


async def read_range(session, address, length):
    return await session._call(
        jsusb.memory.readRange,
        unwrap(session, Session),
        uint(address),
        uint(length),
        decode=buffer_out,
    )


async def write_range(session, address, data):
    await session._call(
        jsusb.memory.writeRange,
        unwrap(session, Session),
        uint(address),
        buffer_in(data),
    )


def read_chunks(session, address, length):
    return Reader._wrap(
        call(
            jsusb.memory.readChunks,
            unwrap(session, Session),
            uint(address),
            uint(length),
        )
    )
