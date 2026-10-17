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
    SoundClip = auto()
    HdmiValue = auto()
    HdmiPacketLength = auto()
    AudioCount = auto()
    RadioChannel = auto()
    RadioPower = auto()
    PeerAddressLength = auto()


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


class Toggle(Enum):
    Disabled = auto()
    Enabled = auto()


class DisplayMode(Enum):
    Option0 = auto()
    Option1 = auto()


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


class SoundClip(Handle):
    def __init__(self, value):
        self._js = call(_js.SoundClip.new, uint(value, 8))

    value = property(lambda self: self._js.value)


class SoundFormat(Enum):
    Format1 = auto()
    Format2 = auto()
    Format3 = auto()


class SoundTrack(Enum):
    Track2 = auto()
    Track3 = auto()
    Track4 = auto()


class SoundOperation(Enum):
    First = auto()
    Second = auto()
    Third = auto()


class SoundAction(Sum, Handle):
    pass


variant(
    SoundAction,
    "Clip",
    [("clip", SoundClip), ("format", SoundFormat)],
    create=lambda clip, format: call(
        _js.SoundAction.clip,
        unwrap(clip, SoundClip),
        _choice(format, SoundFormat, _js.SoundFormat),
    ),
)
variant(
    SoundAction,
    "Track",
    [("track", SoundTrack), ("operation", SoundOperation), ("format", SoundFormat)],
    create=lambda track, operation, format: call(
        _js.SoundAction.track,
        _choice(track, SoundTrack, _js.SoundTrack),
        _choice(operation, SoundOperation, _js.SoundOperation),
        _choice(format, SoundFormat, _js.SoundFormat),
    ),
)
variant(SoundAction, "Noop", create=lambda: call(_js.SoundAction.noop))


class HdmiValue(Handle):
    def __init__(self, value):
        self._js = call(_js.HdmiValue.new, uint(value, 16))

    value = property(lambda self: self._js.value)


class HdmiPacket(Handle):
    def __init__(self, data):
        self._js = call(_js.HdmiPacket.new, buffer_in(data))

    data = property(lambda self: buffer_out(self._js.data))


class HdmiAction(Sum, Handle):
    pass


variant(HdmiAction, "ResetStub", create=lambda: call(_js.HdmiAction.resetStub))
variant(
    HdmiAction,
    "Packet",
    [("packet", HdmiPacket)],
    create=lambda packet: call(_js.HdmiAction.packet, unwrap(packet, HdmiPacket)),
)
variant(
    HdmiAction,
    "Triplet",
    [("first", HdmiValue), ("second", HdmiValue), ("third", HdmiValue)],
    create=lambda first, second, third: call(
        _js.HdmiAction.triplet,
        unwrap(first, HdmiValue),
        unwrap(second, HdmiValue),
        unwrap(third, HdmiValue),
    ),
)


class ChargeAction(Sum, Handle):
    pass


variant(
    ChargeAction,
    "Control",
    [("enabled", Toggle)],
    create=lambda enabled: call(
        _js.ChargeAction.control, _choice(enabled, Toggle, _js.Toggle)
    ),
)
variant(
    ChargeAction,
    "EnableAndSetFlag",
    [("enabled", Toggle)],
    create=lambda enabled: call(
        _js.ChargeAction.enableAndSetFlag, _choice(enabled, Toggle, _js.Toggle)
    ),
)


class ChargeQuery(Enum):
    Presence = auto()
    State = auto()


class AudioCount(Handle):
    def __init__(self, value):
        self._js = call(_js.AudioCount.new, uint(value, 8))

    value = property(lambda self: self._js.value)


class AudioRoute(Enum):
    Request82 = auto()
    Request81 = auto()


class AudioPlayback(Enum):
    AllLanes = auto()
    ZeroSecondLane = auto()
    ZeroFirstLane = auto()


class StorageErase(Enum):
    Kind1Regions = auto()
    Kind0And2Regions = auto()
    Kind0And1And2Regions = auto()
    AuxiliaryRegion = auto()
    ResetProfiles = auto()


class NandSpare(Enum):
    Omit = auto()
    Include = auto()


class RadioMode(Enum):
    Mode0 = auto()
    Mode1 = auto()


class RadioChannel(Handle):
    def __init__(self, value):
        self._js = call(_js.RadioChannel.new, uint(value, 8))

    value = property(lambda self: self._js.value)


class RadioPower(Handle):
    def __init__(self, value):
        self._js = call(_js.RadioPower.new, uint(value, 8))

    value = property(lambda self: self._js.value)


class PeerAddress(Handle):
    def __init__(self, data):
        self._js = call(_js.PeerAddress.new, buffer_in(data))

    data = property(lambda self: buffer_out(self._js.data))


class RadioAction(Sum, Handle):
    pass


variant(
    RadioAction,
    "Primary",
    [
        ("mode", RadioMode),
        ("channel", RadioChannel),
        ("power", RadioPower),
        ("option", Toggle),
    ],
    create=lambda mode, channel, power, option: call(
        _js.RadioAction.primary,
        _choice(mode, RadioMode, _js.RadioMode),
        unwrap(channel, RadioChannel),
        unwrap(power, RadioPower),
        _choice(option, Toggle, _js.Toggle),
    ),
)
variant(
    RadioAction,
    "Secondary",
    [("mode", RadioMode), ("channel", RadioChannel)],
    create=lambda mode, channel: call(
        _js.RadioAction.secondary,
        _choice(mode, RadioMode, _js.RadioMode),
        unwrap(channel, RadioChannel),
    ),
)


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
    [("mode", DisplayMode)],
    create=lambda mode: call(
        _js.Command.displayMode, _choice(mode, DisplayMode, _js.DisplayMode)
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
    [("enabled", Toggle)],
    create=lambda enabled: call(
        _js.Command.parameterService, _choice(enabled, Toggle, _js.Toggle)
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
    [("query", ChargeQuery)],
    create=lambda query: call(
        _js.Command.chargeStatus, _choice(query, ChargeQuery, _js.ChargeQuery)
    ),
)


variant(
    Command,
    "AudioCapture",
    [("count", AudioCount), ("route", AudioRoute)],
    create=lambda count, route: call(
        _js.Command.audioCapture,
        unwrap(count, AudioCount),
        _choice(route, AudioRoute, _js.AudioRoute),
    ),
)
variant(
    Command,
    "AudioPlayback",
    [("mode", AudioPlayback)],
    create=lambda mode: call(
        _js.Command.audioPlayback, _choice(mode, AudioPlayback, _js.AudioPlayback)
    ),
)
variant(
    Command,
    "EraseStorage",
    [("action", StorageErase)],
    create=lambda action: call(
        _js.Command.eraseStorage, _choice(action, StorageErase, _js.StorageErase)
    ),
)
variant(Command, "StorageStatus", create=lambda: call(_js.Command.storageStatus))
variant(Command, "FormatStorage", create=lambda: call(_js.Command.formatStorage))
variant(Command, "DisplaySetup", create=lambda: call(_js.Command.displaySetup))


variant(
    Command,
    "NandReadBlocks",
    [("spare", NandSpare), ("block_group", int)],
    create=lambda spare, block_group: call(
        _js.Command.nandReadBlocks,
        _choice(spare, NandSpare, _js.NandSpare),
        uint(block_group, 8),
    ),
)
variant(Command, "Option41", create=lambda: call(_js.Command.option41))
variant(
    Command,
    "WlanControl",
    [("enabled", Toggle)],
    create=lambda enabled: call(
        _js.Command.wlanControl, _choice(enabled, Toggle, _js.Toggle)
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
    [("address", PeerAddress)],
    create=lambda address: call(
        _js.Command.signalStrength, unwrap(address, PeerAddress)
    ),
)
variant(Command, "Iperf", create=lambda: call(_js.Command.iperf))


class NandReadResult(Sum):
    pass


variant(NandReadResult, "Success")
variant(NandReadResult, "RejectedOrWaitError")
variant(NandReadResult, "OtherError")
variant(NandReadResult, "Unknown", [("raw_status", int)])


class Option41State(Sum):
    pass


variant(Option41State, "Zero")
variant(Option41State, "NonZero")
variant(Option41State, "Unknown", [("raw_status", int)])


class AudioResult(Sum):
    pass


variant(AudioResult, "Success")
variant(AudioResult, "Failed")
variant(AudioResult, "Unknown", [("raw_status", int)])


class StorageEraseResult(Sum):
    pass


variant(StorageEraseResult, "Success")
variant(StorageEraseResult, "Failed")
variant(StorageEraseResult, "Unknown", [("raw_status", int)])


class StorageStatus(Sum):
    pass


variant(StorageStatus, "Zero")
variant(StorageStatus, "Value4")
variant(StorageStatus, "Value8")
variant(StorageStatus, "Value16")
variant(StorageStatus, "Value32")
variant(StorageStatus, "Unknown", [("raw_status", int)])


class ChargeResult(Sum):
    pass


variant(ChargeResult, "Success")
variant(ChargeResult, "Failed")
variant(ChargeResult, "Unknown", [("raw_status", int)])


class ChargePresence(Sum):
    pass


variant(ChargePresence, "Failed")
variant(ChargePresence, "Absent")
variant(ChargePresence, "Present")
variant(ChargePresence, "Unknown", [("raw_status", int)])


class ReportedChargeState(Sum):
    pass


variant(ReportedChargeState, "Failed")
variant(ReportedChargeState, "Value0")
variant(ReportedChargeState, "Values2To5")
variant(ReportedChargeState, "Value6")
variant(ReportedChargeState, "OtherValue")
variant(ReportedChargeState, "Unknown", [("raw_status", int)])


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
variant(ReplyValue, "NandRead", [("status", NandReadResult)])
variant(ReplyValue, "Option41", [("state", Option41State)])
variant(ReplyValue, "SignalStrength", [("raw_value", int)])
variant(ReplyValue, "Iperf", [("raw_result", int)])
variant(ReplyValue, "AudioCaptured", [("status", AudioResult)])
variant(ReplyValue, "AudioPlayed", [("status", AudioResult)])
variant(ReplyValue, "StorageErased", [("status", StorageEraseResult)])
variant(ReplyValue, "StorageStatus", [("status", StorageStatus)])
variant(ReplyValue, "ChargeApplied", [("status", ChargeResult)])
variant(ReplyValue, "ChargePresence", [("status", ChargePresence)])
variant(ReplyValue, "ChargeState", [("status", ReportedChargeState)])
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


def _charge_result(value):
    kinds = _js.ChargeResultKind
    return {
        kinds.Success: ChargeResult.Success,
        kinds.Failed: ChargeResult.Failed,
        kinds.Unknown: lambda: ChargeResult.Unknown(value.rawStatus),
    }[value.kind]()


def _charge_presence(value):
    kinds = _js.ChargePresenceKind
    return {
        kinds.Failed: ChargePresence.Failed,
        kinds.Absent: ChargePresence.Absent,
        kinds.Present: ChargePresence.Present,
        kinds.Unknown: lambda: ChargePresence.Unknown(value.rawStatus),
    }[value.kind]()


def _charge_state(value):
    kinds = _js.ReportedChargeStateKind
    return {
        kinds.Failed: ReportedChargeState.Failed,
        kinds.Value0: ReportedChargeState.Value0,
        kinds.Values2To5: ReportedChargeState.Values2To5,
        kinds.Value6: ReportedChargeState.Value6,
        kinds.OtherValue: ReportedChargeState.OtherValue,
        kinds.Unknown: lambda: ReportedChargeState.Unknown(value.rawStatus),
    }[value.kind]()


def _audio_result(value):
    kinds = _js.AudioResultKind
    return {
        kinds.Success: AudioResult.Success,
        kinds.Failed: AudioResult.Failed,
        kinds.Unknown: lambda: AudioResult.Unknown(value.rawStatus),
    }[value.kind]()


def _storage_erase_result(value):
    kinds = _js.StorageEraseResultKind
    return {
        kinds.Success: StorageEraseResult.Success,
        kinds.Failed: StorageEraseResult.Failed,
        kinds.Unknown: lambda: StorageEraseResult.Unknown(value.rawStatus),
    }[value.kind]()


def _storage_status(value):
    kinds = _js.StorageStatusKind
    return {
        kinds.Zero: StorageStatus.Zero,
        kinds.Value4: StorageStatus.Value4,
        kinds.Value8: StorageStatus.Value8,
        kinds.Value16: StorageStatus.Value16,
        kinds.Value32: StorageStatus.Value32,
        kinds.Unknown: lambda: StorageStatus.Unknown(value.rawStatus),
    }[value.kind]()


def _nand_read(value):
    kinds = _js.NandReadResultKind
    return {
        kinds.Success: NandReadResult.Success,
        kinds.RejectedOrWaitError: NandReadResult.RejectedOrWaitError,
        kinds.OtherError: NandReadResult.OtherError,
        kinds.Unknown: lambda: NandReadResult.Unknown(value.rawStatus),
    }[value.kind]()


def _option_41(value):
    kinds = _js.Option41StateKind
    return {
        kinds.Zero: Option41State.Zero,
        kinds.NonZero: Option41State.NonZero,
        kinds.Unknown: lambda: Option41State.Unknown(value.rawStatus),
    }[value.kind]()


def _value(value):
    kinds = _js.ReplyKind
    return {
        kinds.NandRead: lambda: ReplyValue.NandRead(_nand_read(value.status)),
        kinds.Option41: lambda: ReplyValue.Option41(_option_41(value.state)),
        kinds.SignalStrength: lambda: ReplyValue.SignalStrength(value.rawValue),
        kinds.Iperf: lambda: ReplyValue.Iperf(value.rawResult),
        kinds.AudioCaptured: lambda: ReplyValue.AudioCaptured(
            _audio_result(value.status)
        ),
        kinds.AudioPlayed: lambda: ReplyValue.AudioPlayed(_audio_result(value.status)),
        kinds.StorageErased: lambda: ReplyValue.StorageErased(
            _storage_erase_result(value.status)
        ),
        kinds.StorageStatus: lambda: ReplyValue.StorageStatus(
            _storage_status(value.status)
        ),
        kinds.ChargeApplied: lambda: ReplyValue.ChargeApplied(
            _charge_result(value.status)
        ),
        kinds.ChargePresence: lambda: ReplyValue.ChargePresence(
            _charge_presence(value.status)
        ),
        kinds.ChargeState: lambda: ReplyValue.ChargeState(_charge_state(value.status)),
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
