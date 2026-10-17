"""Explicit research envelopes, encoded and checked by Rust."""

from dataclasses import dataclass
from . import CommandStatus
from ._bridge import (
    Handle,
    jsusb,
    uint,
    unwrap,
    buffer_in,
    buffer_out,
    enum_out,
    call,
    snapshot,
)


class CommandCode(Handle):
    def __init__(self, value):
        self._js = call(jsusb.research.CommandCode.new, uint(value))

    value = property(lambda self: self._js.value)

    def __eq__(self, other):
        return type(other) is type(self) and self.value == other.value

    def __hash__(self):
        return hash((type(self), self.value))


class Request(Handle):
    def __init__(self, code, body, *, reply_capacity):
        self._js = call(
            jsusb.research.Request.new,
            unwrap(code, CommandCode),
            buffer_in(body),
            uint(reply_capacity),
        )

    code = property(lambda self: CommandCode._wrap(self._js.code))
    body = property(lambda self: buffer_out(self._js.body))
    reply_capacity = property(lambda self: self._js.replyCapacity)


@dataclass(frozen=True)
class Reply:
    code: CommandCode
    status: CommandStatus
    primary: int
    secondary: int
    body: bytes
    raw: bytes

    @classmethod
    def _from_js(cls, value):
        return snapshot(
            value,
            lambda v: cls(
                CommandCode._wrap(v.code),
                enum_out(v.status, CommandStatus),
                v.primary,
                v.secondary,
                buffer_out(v.body),
                buffer_out(v.raw),
            ),
        )
