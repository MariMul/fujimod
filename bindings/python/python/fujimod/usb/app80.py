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
    AccumulatorAction,
    ArgumentKind,
    AudioCount,
    AudioRoute,
    AudioPlayback,
    AudioResult,
    StorageErase,
    StorageEraseResult,
    StorageStatus,
    ChargeAction,
    ChargePresence,
    ChargeQuery,
    ChargeResult,
    Command,
    CommandError,
    CommandFailure,
    CommandReply,
    DisplayMode,
    DisplayOptionAction,
    HdmiAction,
    HdmiPacket,
    HdmiValue,
    I2cAction,
    I2cLength,
    I2cPayload,
    I2cPort,
    Invocation,
    PinBank,
    ProfileAction,
    ProfileBank,
    ProfileIndex,
    RegisterAction,
    ReplyValue,
    ReportedChargeState,
    SaveStatus,
    Service,
    SoundAction,
    SoundClip,
    SoundFormat,
    SoundOperation,
    SoundTrack,
    Toggle,
    WordAddress,
    WordReadResult,
    WordWriteResult,
    execute,
)

__all__ = [
    "Toggle",
    "DisplayMode",
    "DisplayOptionAction",
    "AudioCount",
    "AudioRoute",
    "AudioPlayback",
    "AudioResult",
    "StorageErase",
    "StorageEraseResult",
    "StorageStatus",
    "SoundClip",
    "SoundFormat",
    "SoundTrack",
    "SoundOperation",
    "SoundAction",
    "HdmiValue",
    "HdmiPacket",
    "HdmiAction",
    "ChargeAction",
    "ChargeQuery",
    "ChargeResult",
    "ChargePresence",
    "ReportedChargeState",
    "RegisterAction",
    "I2cLength",
    "I2cPayload",
    "I2cPort",
    "I2cAction",
    "AccumulatorAction",
    "CommandCode",
    "Request",
    "Mailbox",
    "exchange",
    "CommandError",
    "CommandFailure",
    "ArgumentKind",
    "PinBank",
    "ProfileIndex",
    "WordAddress",
    "ProfileBank",
    "ProfileAction",
    "Command",
    "SaveStatus",
    "WordReadResult",
    "WordWriteResult",
    "ReplyValue",
    "CommandReply",
    "Invocation",
    "Service",
    "execute",
]
