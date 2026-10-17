"""Pyodide conversion for Rust-owned service commands and typed replies."""

from dataclasses import dataclass
from enum import Enum, auto

from .._bridge import Handle, buffer_in, buffer_out, call, jsusb, snapshot, uint, unwrap
from .._variants import Sum, enum_out, variant
from ..defs import service as definitions
from . import Session
from .app80 import CommandCode

_js = jsusb.app80


class CommandError(ValueError):
    pass


class ArgumentKind(Enum):
    PinBank = auto()
    ProfileIndex = auto()
    WordAlignment = auto()
    WordAddress = auto()
    I2cLength = auto()


class CommandFailure(Sum):
    pass


variant(CommandFailure, "Unsupported", [("codec", definitions.Codec)])
variant(CommandFailure, "Ambiguous", [("codec", definitions.Codec)])
variant(CommandFailure, "Unavailable", [("code", int)])
variant(CommandFailure, "Argument", [("kind", ArgumentKind)])
variant(CommandFailure, "OversizeArguments")


def _command_failure(value):
    from _fujimod_js import defs

    kinds = _js.CommandFailureKind
    return {
        kinds.Unsupported: lambda: CommandFailure.Unsupported(
            enum_out(value.codec, definitions.Codec, defs.service.Codec)
        ),
        kinds.Ambiguous: lambda: CommandFailure.Ambiguous(
            enum_out(value.codec, definitions.Codec, defs.service.Codec)
        ),
        kinds.Unavailable: lambda: CommandFailure.Unavailable(value.code),
        kinds.Argument: lambda: CommandFailure.Argument(
            enum_out(value.argument, ArgumentKind, _js.ArgumentKind)
        ),
        kinds.OversizeArguments: CommandFailure.OversizeArguments,
    }[value.kind]()


class PinBank(Handle):
    def __init__(self, value):
        self._js = call(_js.PinBank.new, uint(value, 8))

    value = property(lambda self: self._js.value)


class ProfileIndex(Handle):
    def __init__(self, value):
        self._js = call(_js.ProfileIndex.new, uint(value, 8))

    value = property(lambda self: self._js.value)


class WordAddress(Handle):
    def __init__(self, value):
        self._js = call(_js.WordAddress.new, uint(value))

    value = property(lambda self: self._js.value)


class ProfileBank(Enum):
    First = auto()
    Second = auto()


def _bank(value):
    if type(value) is not ProfileBank:
        raise TypeError("expected ProfileBank")
    return getattr(_js.ProfileBank, value.name)


def _owned(value, cls):
    if not isinstance(value, cls):
        raise TypeError(f"expected {cls.__name__} variant")
    return value._js


class ProfileAction(Sum, Handle):
    pass


variant(
    ProfileAction,
    "Read",
    [("bank", ProfileBank), ("index", ProfileIndex)],
    create=lambda bank, index: call(
        _js.ProfileAction.read, _bank(bank), unwrap(index, ProfileIndex)
    ),
)
variant(
    ProfileAction,
    "Write",
    [("bank", ProfileBank), ("index", ProfileIndex), ("value", int)],
    create=lambda bank, index, value: call(
        _js.ProfileAction.write,
        _bank(bank),
        unwrap(index, ProfileIndex),
        uint(value, 8),
    ),
)
variant(ProfileAction, "Reset", create=lambda: call(_js.ProfileAction.reset))


class RegisterAction(Sum, Handle):
    pass


variant(
    RegisterAction,
    "AudioValue",
    [("value", int)],
    create=lambda value: call(_js.RegisterAction.audioValue, uint(value, 8)),
)
variant(
    RegisterAction,
    "DisplayFirst",
    [("selector", int), ("value", int)],
    create=lambda selector, value: call(
        _js.RegisterAction.displayFirst, uint(selector, 8), uint(value, 16)
    ),
)
variant(
    RegisterAction,
    "DisplayStub",
    [("selector", int), ("value", int)],
    create=lambda selector, value: call(
        _js.RegisterAction.displayStub, uint(selector, 8), uint(value, 16)
    ),
)
variant(
    RegisterAction,
    "DisplaySecond",
    [("selector", int), ("value", int)],
    create=lambda selector, value: call(
        _js.RegisterAction.displaySecond, uint(selector, 8), uint(value, 16)
    ),
)
variant(
    RegisterAction,
    "PowerWrite",
    [("register", int), ("value", int)],
    create=lambda register, value: call(
        _js.RegisterAction.powerWrite, uint(register, 16), uint(value, 8)
    ),
)
variant(
    RegisterAction,
    "PositionPair",
    [("first", int), ("second", int)],
    create=lambda first, second: call(
        _js.RegisterAction.positionPair, uint(first, 16), uint(second, 16)
    ),
)
variant(RegisterAction, "NoopFirst", create=lambda: call(_js.RegisterAction.noopFirst))
variant(
    RegisterAction, "NoopSecond", create=lambda: call(_js.RegisterAction.noopSecond)
)


class I2cLength(Handle):
    def __init__(self, value):
        self._js = call(_js.I2cLength.new, uint(value, 8))

    value = property(lambda self: self._js.value)


class I2cPayload(Handle):
    def __init__(self, data):
        self._js = call(_js.I2cPayload.new, buffer_in(data))

    data = property(lambda self: buffer_out(self._js.data))


class I2cPort(Enum):
    Port0 = auto()
    Port1 = auto()
    Port2 = auto()
    Port4 = auto()
    Port5 = auto()


def _choice(value, cls, choices):
    if type(value) is not cls:
        raise TypeError(f"expected {cls.__name__}")
    return getattr(choices, value.name)


class I2cAction(Sum, Handle):
    pass


variant(
    I2cAction,
    "Read",
    [("port", I2cPort), ("command", int), ("length", I2cLength)],
    create=lambda port, command, length: call(
        _js.I2cAction.read,
        _choice(port, I2cPort, _js.I2cPort),
        uint(command, 8),
        unwrap(length, I2cLength),
    ),
)
variant(
    I2cAction,
    "Write",
    [("port", I2cPort), ("command", int), ("payload", I2cPayload)],
    create=lambda port, command, payload: call(
        _js.I2cAction.write,
        _choice(port, I2cPort, _js.I2cPort),
        uint(command, 8),
        unwrap(payload, I2cPayload),
    ),
)
variant(
    I2cAction,
    "WriteSingle",
    [("command", int), ("value", int)],
    create=lambda command, value: call(
        _js.I2cAction.writeSingle, uint(command, 8), uint(value, 8)
    ),
)


class AccumulatorAction(Enum):
    Start = auto()
    StopAndRead = auto()


class Command(Sum, Handle):
    pass


variant(
    Command,
    "WritePins",
    [("bank", PinBank), ("levels", int), ("exclude", int)],
    create=lambda bank, levels, exclude: call(
        _js.Command.writePins,
        unwrap(bank, PinBank),
        uint(levels, 16),
        uint(exclude, 16),
    ),
)
variant(
    Command,
    "ReadPins",
    [("bank", PinBank)],
    create=lambda bank: call(_js.Command.readPins, unwrap(bank, PinBank)),
)
variant(
    Command,
    "ReadChannel",
    [("channel", int)],
    create=lambda channel: call(_js.Command.readChannel, uint(channel, 8)),
)
variant(
    Command,
    "WriteChannel",
    [("channel", int), ("value", int)],
    create=lambda channel, value: call(
        _js.Command.writeChannel, uint(channel, 8), uint(value, 16)
    ),
)
variant(
    Command,
    "ProfileByte",
    [("action", ProfileAction)],
    create=lambda action: call(_js.Command.profileByte, _owned(action, ProfileAction)),
)
variant(Command, "DownloadStatus", create=lambda: call(_js.Command.downloadStatus))
variant(
    Command,
    "CopyConfiguration",
    [("page", int)],
    create=lambda page: call(_js.Command.copyConfiguration, uint(page, 8)),
)
variant(
    Command, "FlushConfiguration", create=lambda: call(_js.Command.flushConfiguration)
)
variant(
    Command,
    "ReadWord",
    [("address", WordAddress)],
    create=lambda address: call(_js.Command.readWord, unwrap(address, WordAddress)),
)
variant(
    Command,
    "WriteWord",
    [("address", WordAddress), ("value", int)],
    create=lambda address, value: call(
        _js.Command.writeWord, unwrap(address, WordAddress), uint(value)
    ),
)


variant(
    Command,
    "RegisterControl",
    [("action", RegisterAction)],
    create=lambda action: call(
        _js.Command.registerControl, _owned(action, RegisterAction)
    ),
)
variant(
    Command,
    "I2c",
    [("action", I2cAction)],
    create=lambda action: call(_js.Command.i2c, _owned(action, I2cAction)),
)
variant(
    Command,
    "Accumulator",
    [("action", AccumulatorAction)],
    create=lambda action: call(
        _js.Command.accumulator,
        _choice(action, AccumulatorAction, _js.AccumulatorAction),
    ),
)
variant(Command, "PeripheralReport", create=lambda: call(_js.Command.peripheralReport))


class SaveStatus(Sum):
    pass


variant(SaveStatus, "Success")
variant(SaveStatus, "Failed")
variant(SaveStatus, "Unknown", [("raw_status", int)])


class WordReadResult(Sum):
    pass


variant(WordReadResult, "Value", [("value", int)])
variant(WordReadResult, "Rejected")
variant(WordReadResult, "Unknown", [("raw_status", int)])


class WordWriteResult(Sum):
    pass


variant(WordWriteResult, "Confirmed")
variant(WordWriteResult, "Rejected")
variant(WordWriteResult, "Unknown", [("raw_status", int)])


class ReplyValue(Sum):
    pass


variant(ReplyValue, "NoPayload")
variant(ReplyValue, "Pins", [("levels", int)])
variant(ReplyValue, "Channel", [("value", int)])
variant(ReplyValue, "ProfileByte", [("value", int)])
variant(ReplyValue, "DownloadStatus", [("raw_status", int)])
variant(ReplyValue, "ConfigurationSaved", [("status", SaveStatus)])
variant(ReplyValue, "ReadWord", [("result", WordReadResult)])
variant(ReplyValue, "WriteWord", [("result", WordWriteResult)])
variant(ReplyValue, "Research")
variant(ReplyValue, "I2cData", [("data", bytes)])
variant(ReplyValue, "Accumulated", [("values", tuple)])
variant(ReplyValue, "PeripheralReport", [("data", bytes)])


def _save(value):
    kinds = _js.SaveStatusKind
    return {
        kinds.Success: SaveStatus.Success,
        kinds.Failed: SaveStatus.Failed,
        kinds.Unknown: lambda: SaveStatus.Unknown(value.rawStatus),
    }[value.kind]()


def _word_read(value):
    kinds = _js.WordReadKind
    return {
        kinds.Value: lambda: WordReadResult.Value(value.value),
        kinds.Rejected: WordReadResult.Rejected,
        kinds.Unknown: lambda: WordReadResult.Unknown(value.rawStatus),
    }[value.kind]()


def _word_write(value):
    kinds = _js.WordWriteKind
    return {
        kinds.Confirmed: WordWriteResult.Confirmed,
        kinds.Rejected: WordWriteResult.Rejected,
        kinds.Unknown: lambda: WordWriteResult.Unknown(value.rawStatus),
    }[value.kind]()


def _value(value):
    kinds = _js.ReplyKind
    return {
        kinds.NoPayload: ReplyValue.NoPayload,
        kinds.Pins: lambda: ReplyValue.Pins(value.levels),
        kinds.Channel: lambda: ReplyValue.Channel(value.value),
        kinds.ProfileByte: lambda: ReplyValue.ProfileByte(value.value),
        kinds.DownloadStatus: lambda: ReplyValue.DownloadStatus(value.rawStatus),
        kinds.ConfigurationSaved: lambda: ReplyValue.ConfigurationSaved(
            _save(value.status)
        ),
        kinds.ReadWord: lambda: ReplyValue.ReadWord(_word_read(value.result)),
        kinds.WriteWord: lambda: ReplyValue.WriteWord(_word_write(value.result)),
        kinds.Research: ReplyValue.Research,
        kinds.I2cData: lambda: ReplyValue.I2cData(buffer_out(value.data)),
        kinds.Accumulated: lambda: ReplyValue.Accumulated(tuple(value.values)),
        kinds.PeripheralReport: lambda: ReplyValue.PeripheralReport(
            buffer_out(value.data)
        ),
    }[value.kind]()


@dataclass(frozen=True)
class CommandReply:
    value: ReplyValue
    raw: bytes

    @classmethod
    def _from_js(cls, value):
        return snapshot(
            value, lambda reply: cls(_value(reply.value), buffer_out(reply.raw))
        )


class Invocation(Handle):
    def __init__(self):
        raise TypeError("invocations come from Service.prepare")

    raw = property(lambda self: buffer_out(self._js.raw))
    configuration_limit = property(lambda self: self._js.configurationLimit)

    def decode(self, raw):
        return CommandReply._from_js(call(self._js.decode, buffer_in(raw)))


class Service(Handle):
    def __init__(self, definition):
        self._js = call(_js.Service.new, unwrap(definition, definitions.Definition))

    def prepare(self, modes, command):
        return Invocation._wrap(
            call(
                self._js.prepare,
                unwrap(modes, definitions.ServiceModes),
                _owned(command, Command),
            )
        )

    def prepare_research(self, code, modes, arguments):
        return Invocation._wrap(
            call(
                self._js.prepareResearch,
                unwrap(code, CommandCode),
                unwrap(modes, definitions.ServiceModes),
                buffer_in(arguments),
            )
        )


async def execute(session, invocation):
    return await session._call(
        _js.execute,
        unwrap(session, Session),
        unwrap(invocation, Invocation),
        decode=CommandReply._from_js,
    )
