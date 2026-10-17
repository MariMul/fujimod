"""Pyodide conversion for Rust-owned service commands and typed replies."""

from dataclasses import dataclass
from types import SimpleNamespace
from _fujimod_js import valueSchemas as _schemas
from .._binding_values import bind_values as _bind_values

from .._bridge import Handle, buffer_in, buffer_out, call, jsusb, snapshot, uint, unwrap
from .._variants import Sum, enum_out, variant
from ..defs import service as definitions
from . import Session
from .app80 import CommandCode

_js = jsusb.app80
_value_types, _result_decoders = _bind_values(
    _schemas.service, _js, "fujimod.usb.app80"
)
_v = SimpleNamespace(**_value_types)


class CommandError(ValueError):
    pass


class CommandFailure(Sum):
    pass


variant(CommandFailure, "Unsupported", [("codec", definitions.Codec)])
variant(CommandFailure, "Ambiguous", [("codec", definitions.Codec)])
variant(CommandFailure, "Unavailable", [("code", int)])
variant(CommandFailure, "Argument", [("kind", _v.ArgumentKind)])
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
            enum_out(value.argument, _v.ArgumentKind, _js.ArgumentKind)
        ),
        kinds.OversizeArguments: CommandFailure.OversizeArguments,
    }[value.kind]()


def _bank(value):
    if type(value) is not _v.ProfileBank:
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
    [("bank", _v.ProfileBank), ("index", _v.ProfileIndex)],
    create=lambda bank, index: call(
        _js.ProfileAction.read, _bank(bank), unwrap(index, _v.ProfileIndex)
    ),
)
variant(
    ProfileAction,
    "Write",
    [("bank", _v.ProfileBank), ("index", _v.ProfileIndex), ("value", int)],
    create=lambda bank, index, value: call(
        _js.ProfileAction.write,
        _bank(bank),
        unwrap(index, _v.ProfileIndex),
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


def _choice(value, cls, choices):
    if type(value) is not cls:
        raise TypeError(f"expected {cls.__name__}")
    return getattr(choices, value.name)


class I2cAction(Sum, Handle):
    pass


variant(
    I2cAction,
    "Read",
    [("port", _v.I2cPort), ("command", int), ("length", _v.I2cLength)],
    create=lambda port, command, length: call(
        _js.I2cAction.read,
        _choice(port, _v.I2cPort, _js.I2cPort),
        uint(command, 8),
        unwrap(length, _v.I2cLength),
    ),
)
variant(
    I2cAction,
    "Write",
    [("port", _v.I2cPort), ("command", int), ("payload", _v.I2cPayload)],
    create=lambda port, command, payload: call(
        _js.I2cAction.write,
        _choice(port, _v.I2cPort, _js.I2cPort),
        uint(command, 8),
        unwrap(payload, _v.I2cPayload),
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


class DisplayOptionAction(Sum, Handle):
    pass


variant(
    DisplayOptionAction,
    "IdlePins",
    create=lambda: call(_js.DisplayOptionAction.idlePins),
)
variant(
    DisplayOptionAction,
    "ForwardPair",
    [("first", int), ("second", int)],
    create=lambda first, second: call(
        _js.DisplayOptionAction.forwardPair, uint(first, 8), uint(second, 8)
    ),
)


class SoundAction(Sum, Handle):
    pass


variant(
    SoundAction,
    "Clip",
    [("clip", _v.SoundClip), ("format", _v.SoundFormat)],
    create=lambda clip, format: call(
        _js.SoundAction.clip,
        unwrap(clip, _v.SoundClip),
        _choice(format, _v.SoundFormat, _js.SoundFormat),
    ),
)
variant(
    SoundAction,
    "Track",
    [
        ("track", _v.SoundTrack),
        ("operation", _v.SoundOperation),
        ("format", _v.SoundFormat),
    ],
    create=lambda track, operation, format: call(
        _js.SoundAction.track,
        _choice(track, _v.SoundTrack, _js.SoundTrack),
        _choice(operation, _v.SoundOperation, _js.SoundOperation),
        _choice(format, _v.SoundFormat, _js.SoundFormat),
    ),
)
variant(SoundAction, "Noop", create=lambda: call(_js.SoundAction.noop))


class HdmiAction(Sum, Handle):
    pass


variant(HdmiAction, "ResetStub", create=lambda: call(_js.HdmiAction.resetStub))
variant(
    HdmiAction,
    "Packet",
    [("packet", _v.HdmiPacket)],
    create=lambda packet: call(_js.HdmiAction.packet, unwrap(packet, _v.HdmiPacket)),
)
variant(
    HdmiAction,
    "Triplet",
    [("first", _v.HdmiValue), ("second", _v.HdmiValue), ("third", _v.HdmiValue)],
    create=lambda first, second, third: call(
        _js.HdmiAction.triplet,
        unwrap(first, _v.HdmiValue),
        unwrap(second, _v.HdmiValue),
        unwrap(third, _v.HdmiValue),
    ),
)


class ChargeAction(Sum, Handle):
    pass


variant(
    ChargeAction,
    "Control",
    [("enabled", _v.Toggle)],
    create=lambda enabled: call(
        _js.ChargeAction.control, _choice(enabled, _v.Toggle, _js.Toggle)
    ),
)
variant(
    ChargeAction,
    "EnableAndSetFlag",
    [("enabled", _v.Toggle)],
    create=lambda enabled: call(
        _js.ChargeAction.enableAndSetFlag, _choice(enabled, _v.Toggle, _js.Toggle)
    ),
)


class RadioAction(Sum, Handle):
    pass


variant(
    RadioAction,
    "Primary",
    [
        ("mode", _v.RadioMode),
        ("channel", _v.RadioChannel),
        ("power", _v.RadioPower),
        ("option", _v.Toggle),
    ],
    create=lambda mode, channel, power, option: call(
        _js.RadioAction.primary,
        _choice(mode, _v.RadioMode, _js.RadioMode),
        unwrap(channel, _v.RadioChannel),
        unwrap(power, _v.RadioPower),
        _choice(option, _v.Toggle, _js.Toggle),
    ),
)
variant(
    RadioAction,
    "Secondary",
    [("mode", _v.RadioMode), ("channel", _v.RadioChannel)],
    create=lambda mode, channel: call(
        _js.RadioAction.secondary,
        _choice(mode, _v.RadioMode, _js.RadioMode),
        unwrap(channel, _v.RadioChannel),
    ),
)


class Command(Sum, Handle):
    pass


variant(
    Command,
    "WritePins",
    [("bank", _v.PinBank), ("levels", int), ("exclude", int)],
    create=lambda bank, levels, exclude: call(
        _js.Command.writePins,
        unwrap(bank, _v.PinBank),
        uint(levels, 16),
        uint(exclude, 16),
    ),
)
variant(
    Command,
    "ReadPins",
    [("bank", _v.PinBank)],
    create=lambda bank: call(_js.Command.readPins, unwrap(bank, _v.PinBank)),
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
    [("address", _v.WordAddress)],
    create=lambda address: call(_js.Command.readWord, unwrap(address, _v.WordAddress)),
)
variant(
    Command,
    "WriteWord",
    [("address", _v.WordAddress), ("value", int)],
    create=lambda address, value: call(
        _js.Command.writeWord, unwrap(address, _v.WordAddress), uint(value)
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
    [("action", _v.AccumulatorAction)],
    create=lambda action: call(
        _js.Command.accumulator,
        _choice(action, _v.AccumulatorAction, _js.AccumulatorAction),
    ),
)
variant(Command, "PeripheralReport", create=lambda: call(_js.Command.peripheralReport))

variant(
    Command,
    "DisplayOption",
    [("action", DisplayOptionAction)],
    create=lambda action: call(
        _js.Command.displayOption, _owned(action, DisplayOptionAction)
    ),
)
variant(
    Command,
    "DisplayMode",
    [("mode", _v.DisplayMode)],
    create=lambda mode: call(
        _js.Command.displayMode, _choice(mode, _v.DisplayMode, _js.DisplayMode)
    ),
)
variant(
    Command,
    "Sound",
    [("action", SoundAction)],
    create=lambda action: call(_js.Command.sound, _owned(action, SoundAction)),
)
variant(
    Command,
    "Hdmi",
    [("action", HdmiAction)],
    create=lambda action: call(_js.Command.hdmi, _owned(action, HdmiAction)),
)
variant(
    Command,
    "ParameterService",
    [("enabled", _v.Toggle)],
    create=lambda enabled: call(
        _js.Command.parameterService, _choice(enabled, _v.Toggle, _js.Toggle)
    ),
)
variant(
    Command,
    "ChargeControl",
    [("action", ChargeAction)],
    create=lambda action: call(_js.Command.chargeControl, _owned(action, ChargeAction)),
)
variant(
    Command,
    "ChargeStatus",
    [("query", _v.ChargeQuery)],
    create=lambda query: call(
        _js.Command.chargeStatus, _choice(query, _v.ChargeQuery, _js.ChargeQuery)
    ),
)

variant(
    Command,
    "AudioCapture",
    [("count", _v.AudioCount), ("route", _v.AudioRoute)],
    create=lambda count, route: call(
        _js.Command.audioCapture,
        unwrap(count, _v.AudioCount),
        _choice(route, _v.AudioRoute, _js.AudioRoute),
    ),
)
variant(
    Command,
    "AudioPlayback",
    [("mode", _v.AudioPlayback)],
    create=lambda mode: call(
        _js.Command.audioPlayback, _choice(mode, _v.AudioPlayback, _js.AudioPlayback)
    ),
)
variant(
    Command,
    "EraseStorage",
    [("action", _v.StorageErase)],
    create=lambda action: call(
        _js.Command.eraseStorage, _choice(action, _v.StorageErase, _js.StorageErase)
    ),
)
variant(Command, "StorageStatus", create=lambda: call(_js.Command.storageStatus))
variant(Command, "FormatStorage", create=lambda: call(_js.Command.formatStorage))
variant(Command, "DisplaySetup", create=lambda: call(_js.Command.displaySetup))

variant(
    Command,
    "NandReadBlocks",
    [("spare", _v.NandSpare), ("block_group", int)],
    create=lambda spare, block_group: call(
        _js.Command.nandReadBlocks,
        _choice(spare, _v.NandSpare, _js.NandSpare),
        uint(block_group, 8),
    ),
)
variant(Command, "Option41", create=lambda: call(_js.Command.option41))
variant(
    Command,
    "WlanControl",
    [("enabled", _v.Toggle)],
    create=lambda enabled: call(
        _js.Command.wlanControl, _choice(enabled, _v.Toggle, _js.Toggle)
    ),
)
variant(
    Command,
    "RadioControl",
    [("action", RadioAction)],
    create=lambda action: call(_js.Command.radioControl, _owned(action, RadioAction)),
)
variant(
    Command,
    "SignalStrength",
    [("address", _v.PeerAddress)],
    create=lambda address: call(
        _js.Command.signalStrength, unwrap(address, _v.PeerAddress)
    ),
)
variant(Command, "Iperf", create=lambda: call(_js.Command.iperf))


class WordReadResult(Sum):
    pass


variant(WordReadResult, "Value", [("value", int)])
variant(WordReadResult, "Rejected")
variant(WordReadResult, "Unknown", [("raw_status", int)])


class ReplyValue(Sum):
    pass


variant(ReplyValue, "NoPayload")
variant(ReplyValue, "Pins", [("levels", int)])
variant(ReplyValue, "Channel", [("value", int)])
variant(ReplyValue, "ProfileByte", [("value", int)])
variant(ReplyValue, "DownloadStatus", [("raw_status", int)])
variant(ReplyValue, "ConfigurationSaved", [("status", _v.SaveStatus)])
variant(ReplyValue, "ReadWord", [("result", WordReadResult)])
variant(ReplyValue, "WriteWord", [("result", _v.WordWriteResult)])
variant(ReplyValue, "Research")
variant(ReplyValue, "NandRead", [("status", _v.NandReadResult)])
variant(ReplyValue, "Option41", [("state", _v.Option41State)])
variant(ReplyValue, "SignalStrength", [("raw_value", int)])
variant(ReplyValue, "Iperf", [("raw_result", int)])
variant(ReplyValue, "AudioCaptured", [("status", _v.AudioResult)])
variant(ReplyValue, "AudioPlayed", [("status", _v.AudioResult)])
variant(ReplyValue, "StorageErased", [("status", _v.StorageEraseResult)])
variant(ReplyValue, "StorageStatus", [("status", _v.StorageStatus)])
variant(ReplyValue, "ChargeApplied", [("status", _v.ChargeResult)])
variant(ReplyValue, "ChargePresence", [("status", _v.ChargePresence)])
variant(ReplyValue, "ChargeState", [("status", _v.ReportedChargeState)])
variant(ReplyValue, "I2cData", [("data", bytes)])
variant(ReplyValue, "Accumulated", [("values", tuple)])
variant(ReplyValue, "PeripheralReport", [("data", bytes)])


def _word_read(value):
    kinds = _js.WordReadKind
    return {
        kinds.Value: lambda: WordReadResult.Value(value.value),
        kinds.Rejected: WordReadResult.Rejected,
        kinds.Unknown: lambda: WordReadResult.Unknown(value.rawStatus),
    }[value.kind]()


def _value(value):
    kinds = _js.ReplyKind
    return {
        kinds.NandRead: lambda: ReplyValue.NandRead(
            _result_decoders[_v.NandReadResult](value.status)
        ),
        kinds.Option41: lambda: ReplyValue.Option41(
            _result_decoders[_v.Option41State](value.state)
        ),
        kinds.SignalStrength: lambda: ReplyValue.SignalStrength(value.rawValue),
        kinds.Iperf: lambda: ReplyValue.Iperf(value.rawResult),
        kinds.AudioCaptured: lambda: ReplyValue.AudioCaptured(
            _result_decoders[_v.AudioResult](value.status)
        ),
        kinds.AudioPlayed: lambda: ReplyValue.AudioPlayed(
            _result_decoders[_v.AudioResult](value.status)
        ),
        kinds.StorageErased: lambda: ReplyValue.StorageErased(
            _result_decoders[_v.StorageEraseResult](value.status)
        ),
        kinds.StorageStatus: lambda: ReplyValue.StorageStatus(
            _result_decoders[_v.StorageStatus](value.status)
        ),
        kinds.ChargeApplied: lambda: ReplyValue.ChargeApplied(
            _result_decoders[_v.ChargeResult](value.status)
        ),
        kinds.ChargePresence: lambda: ReplyValue.ChargePresence(
            _result_decoders[_v.ChargePresence](value.status)
        ),
        kinds.ChargeState: lambda: ReplyValue.ChargeState(
            _result_decoders[_v.ReportedChargeState](value.status)
        ),
        kinds.NoPayload: ReplyValue.NoPayload,
        kinds.Pins: lambda: ReplyValue.Pins(value.levels),
        kinds.Channel: lambda: ReplyValue.Channel(value.value),
        kinds.ProfileByte: lambda: ReplyValue.ProfileByte(value.value),
        kinds.DownloadStatus: lambda: ReplyValue.DownloadStatus(value.rawStatus),
        kinds.ConfigurationSaved: lambda: ReplyValue.ConfigurationSaved(
            _result_decoders[_v.SaveStatus](value.status)
        ),
        kinds.ReadWord: lambda: ReplyValue.ReadWord(_word_read(value.result)),
        kinds.WriteWord: lambda: ReplyValue.WriteWord(
            _result_decoders[_v.WordWriteResult](value.result)
        ),
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
