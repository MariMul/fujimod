"""Nested App80 wire types and the Rust mailbox composition."""

from .._bridge import Handle, buffer_in, buffer_out, call, jsusb, uint, unwrap
from . import Session


class CommandCode(Handle):
    def __init__(self, value):
        self._js = call(jsusb.app80.CommandCode.new, uint(value, 8))

    value = property(lambda self: self._js.value)

    def __eq__(self, other):
        return type(other) is type(self) and self.value == other.value

    def __hash__(self):
        return hash((type(self), self.value))


class Request(Handle):
    def __init__(self, code, arguments):
        self._js = call(
            jsusb.app80.Request.new, unwrap(code, CommandCode), buffer_in(arguments)
        )

    raw = property(lambda self: buffer_out(self._js.raw))


class Mailbox(Handle):
    def __init__(self, *, page, offset, configuration_limit):
        self._js = call(
            jsusb.app80.Mailbox.new, uint(page), uint(offset), uint(configuration_limit)
        )


async def exchange(session, mailbox, request):
    return await session._call(
        jsusb.app80.exchange,
        unwrap(session, Session),
        unwrap(mailbox, Mailbox),
        unwrap(request, Request),
        decode=buffer_out,
    )


# The catalogue uses CommandCode; finish the wire types before importing it.
from ._catalogue import (  # noqa: E402
    RadioAction,
    ChargeAction,
    Command,
    CommandError,
    CommandFailure,
    CommandReply,
    DisplayOptionAction,
    HdmiAction,
    I2cAction,
    Invocation,
    ProfileAction,
    RegisterAction,
    ReplyValue,
    Service,
    SoundAction,
    WordReadResult,
    execute,
    _value_types,
)

__all__ = [
    "DisplayOptionAction",
    "RadioAction",
    "SoundAction",
    "HdmiAction",
    "ChargeAction",
    "RegisterAction",
    "I2cAction",
    "CommandCode",
    "Request",
    "Mailbox",
    "exchange",
    "CommandError",
    "CommandFailure",
    "ProfileAction",
    "Command",
    "WordReadResult",
    "ReplyValue",
    "CommandReply",
    "Invocation",
    "Service",
    "execute",
]

globals().update(_value_types)
__all__ += list(_value_types)
