"""Language views of the shared Rust service definition, without USB commands."""

from enum import Enum, auto

from _fujimod_js import defs as _jsdefs

from .._bridge import Handle, buffer_out, call, uint, unwrap
from .._variants import Sum, enum_out, variant
from . import FirmwareIdentity

_js = _jsdefs.service


class Codec(Enum):
    Research = auto()
    WritePins = auto()
    ReadPins = auto()
    ReadChannel = auto()
    WriteChannel = auto()
    ProfileByte = auto()
    DownloadStatus = auto()
    CopyConfiguration = auto()
    FlushConfiguration = auto()
    MemoryWord = auto()
    RegisterControl = auto()
    I2c = auto()
    Accumulator = auto()
    PeripheralReport = auto()
    DisplayOption = auto()
    DisplayMode = auto()
    Sound = auto()
    Hdmi = auto()
    ParameterService = auto()
    ChargeControl = auto()
    ChargeStatus = auto()


class ServiceModes(Handle):
    def __init__(self, mask):
        self._js = call(_js.ServiceModes.new, uint(mask))

    mask = property(lambda self: self._js.mask)

    def __eq__(self, other):
        return type(other) is ServiceModes and self.mask == other.mask


class ModeAvailability(Sum):
    pass


variant(ModeAvailability, "Modes", [("modes", ServiceModes)])
variant(ModeAvailability, "Disabled")
variant(ModeAvailability, "Unreviewed")


class ModeTransition(Sum):
    pass


variant(ModeTransition, "Keep")
variant(ModeTransition, "Set", [("modes", ServiceModes)])


class Entry(Handle):
    def __init__(self):
        raise TypeError("entries come from a loaded definition")

    code = property(lambda self: self._js.code)
    label = property(lambda self: self._js.label)
    codec = property(lambda self: enum_out(self._js.codec, Codec, _js.Codec))
    slots = property(lambda self: tuple(buffer_out(self._js.slots)))
    firmware_kind = property(lambda self: self._js.firmwareKind)
    handler = property(lambda self: int(self._js.handler))
    parameter_table = property(lambda self: int(self._js.parameterTable))

    @property
    def enabled_modes(self):
        value = self._js.enabledModes
        kinds = _js.ModeAvailabilityKind
        return {
            kinds.Modes: lambda: ModeAvailability.Modes(
                ServiceModes._wrap(value.modes)
            ),
            kinds.Disabled: ModeAvailability.Disabled,
            kinds.Unreviewed: ModeAvailability.Unreviewed,
        }[value.kind]()

    @property
    def transition(self):
        value = self._js.transition
        return {
            _js.ModeTransitionKind.Keep: ModeTransition.Keep,
            _js.ModeTransitionKind.Set: lambda: ModeTransition.Set(
                ServiceModes._wrap(value.modes)
            ),
        }[value.kind]()


class Mailbox(Handle):
    def __init__(self):
        raise TypeError("mailbox metadata comes from a loaded definition")

    page = property(lambda self: self._js.page)
    offset = property(lambda self: self._js.offset)
    configuration_limit = property(lambda self: self._js.configurationLimit)


class Definition(Handle):
    def __init__(self):
        raise TypeError("use Definition.from_json")

    @classmethod
    def from_json(cls, json, expected):
        if type(json) is not str:
            raise TypeError("definition JSON must be a string")
        return cls._wrap(
            call(_js.Definition.fromJson, json, unwrap(expected, FirmwareIdentity))
        )

    identity = property(lambda self: FirmwareIdentity._wrap(self._js.identity))
    model = property(lambda self: self._js.model)
    version = property(lambda self: self._js.version)
    mailbox = property(lambda self: Mailbox._wrap(self._js.mailbox))
    entries = property(lambda self: [Entry._wrap(value) for value in self._js.entries])

    def select(self, code, modes):
        value = call(self._js.select, uint(code, 8), unwrap(modes, ServiceModes))
        return None if value is None else Entry._wrap(value)
