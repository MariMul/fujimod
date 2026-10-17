"""Typed Python values. Wire encoding and decoding remain in Rust."""

from dataclasses import dataclass, field
from enum import Enum, auto
from ._bridge import Handle, jsusb, call, uint, enum_in, enum_out, buffer_out, snapshot


class InfoSelector(Enum):
    Vendor = auto()
    Manufacturer = auto()
    Product = auto()
    InternalVersion = auto()
    DeviceType = auto()
    Serial = auto()
    Framework = auto()


class ConfigurationView(Enum):
    Storage = auto()
    Ram = auto()


class KeyState(Enum):
    On = auto()
    Off = auto()


class CameraMode(Enum):
    Record = auto()
    Playback = auto()


class DataReadMode(Enum):
    Pattern = auto()
    Echo = auto()


class DataWriteMode(Enum):
    Preserve = auto()
    FillOnes = auto()
    Echo = auto()


class OffsetMode(Enum):
    Checked = auto()
    Wrapping = auto()


class DeviceFilter(Enum):
    Fujifilm = auto()
    Diagnostic = auto()


class SessionState(Enum):
    Ready = auto()
    InFlight = auto()
    CompletionUnknown = auto()
    Closed = auto()
    Abandoned = auto()
    ReadingRange = auto()


class CommandStatus(Enum):
    Success = auto()
    DeviceError = auto()


class ConsoleProgress(Enum):
    Complete = auto()
    Pending = auto()
    Unknown = auto()


class FirmwareDownloadState(Enum):
    Normal = auto()
    Writing = auto()
    Success = auto()
    Failed = auto()
    Unknown = auto()


@dataclass(frozen=True, kw_only=True)
class FirmwareLimits(Handle):
    memory_read: int
    memory_write: int
    configuration: int
    console: int
    _js: object = field(init=False, repr=False, compare=False)

    def __post_init__(self):
        object.__setattr__(
            self,
            "_js",
            call(
                jsusb.FirmwareLimits.new,
                *(
                    uint(n)
                    for n in (
                        self.memory_read,
                        self.memory_write,
                        self.configuration,
                        self.console,
                    )
                ),
            ),
        )


@dataclass(frozen=True, kw_only=True)
class InterfaceSelection(Handle):
    configuration: int
    interface: int
    alternate: int
    _js: object = field(init=False, repr=False, compare=False)

    def __post_init__(self):
        object.__setattr__(
            self,
            "_js",
            call(
                jsusb.InterfaceSelection.new,
                *(
                    uint(n, 8)
                    for n in (self.configuration, self.interface, self.alternate)
                ),
            ),
        )

    @classmethod
    def _from_js(cls, item):
        return snapshot(
            item,
            lambda v: cls(
                configuration=v.configuration,
                interface=v.interface,
                alternate=v.alternate,
            ),
        )


class Device(Handle):
    def __init__(self):
        raise TypeError("use discover() or request_device()")

    vendor_id = property(lambda self: self._js.vendorId)
    product_id = property(lambda self: self._js.productId)
    # WebUSB does not expose OS bus/address/port identifiers.
    bus_id = property(lambda self: None)
    address = property(lambda self: None)
    ports = property(lambda self: None)
    manufacturer = property(lambda self: self._js.manufacturer)
    product = property(lambda self: self._js.productName)
    serial = property(lambda self: self._js.serialNumber)


class DiagnosticTag(Handle):
    def __init__(self):
        raise TypeError("use bulk_source() or research(value)")

    @classmethod
    def bulk_source(cls):
        return cls._wrap(call(jsusb.DiagnosticTag.bulkSource))

    @classmethod
    def research(cls, value):
        return cls._wrap(call(jsusb.DiagnosticTag.research, uint(value)))

    value = property(lambda self: self._js.value)


class DataWindow(Handle):
    def __init__(self, base, address, length, capacity, offset_mode):
        self._js = call(
            jsusb.DataWindow.new,
            uint(base),
            uint(address),
            uint(length),
            uint(capacity),
            enum_in(offset_mode, OffsetMode),
        )

    capacity = property(lambda self: self._js.capacity)


@dataclass(frozen=True)
class DiagnosticReply:
    status: CommandStatus
    primary: int
    secondary: int
    _fields: tuple
    raw: bytes
    fields = property(lambda self: list(self._fields))

    @classmethod
    def _from_js(cls, value):
        return snapshot(
            value,
            lambda v: cls(
                enum_out(v.status, CommandStatus),
                v.primary,
                v.secondary,
                tuple(v.fields.to_py()),
                buffer_out(v.raw),
            ),
        )


@dataclass(frozen=True)
class FirmwareDownloadReply:
    state: FirmwareDownloadState
    failure_code: int | None
    text: str
    raw: bytes

    @classmethod
    def _from_js(cls, value):
        return snapshot(
            value,
            lambda v: cls(
                enum_out(v.state, FirmwareDownloadState),
                v.failureCode,
                v.text,
                buffer_out(v.raw),
            ),
        )


@dataclass(frozen=True)
class ConfigurationGeometry:
    capacity: int
    page_bytes: int
    raw: bytes

    @classmethod
    def _from_js(cls, value):
        return snapshot(
            value, lambda v: cls(v.capacity, v.pageBytes, buffer_out(v.raw))
        )


@dataclass(frozen=True)
class CapabilitiesReply:
    byte_length: int
    raw: bytes

    @classmethod
    def _from_js(cls, value):
        return snapshot(value, lambda v: cls(v.byteLength, buffer_out(v.raw)))


@dataclass(frozen=True)
class LogReply:
    status: int
    data: bytes

    @classmethod
    def _from_js(cls, value):
        return snapshot(value, lambda v: cls(v.status, buffer_out(v.data)))


@dataclass(frozen=True)
class ConsoleReply:
    progress: ConsoleProgress
    unknown_progress: int | None
    status: int
    output: bytes

    @classmethod
    def _from_js(cls, value):
        return snapshot(
            value,
            lambda v: cls(
                enum_out(v.progress, ConsoleProgress),
                v.unknownProgress,
                v.status,
                buffer_out(v.output),
            ),
        )


# Public values use the same qualified names as the native binding.
for _value in (
    InfoSelector,
    ConfigurationView,
    KeyState,
    CameraMode,
    DataReadMode,
    DataWriteMode,
    OffsetMode,
    DeviceFilter,
    SessionState,
    CommandStatus,
    ConsoleProgress,
    FirmwareDownloadState,
    FirmwareLimits,
    InterfaceSelection,
    Device,
    DiagnosticTag,
    DataWindow,
    DiagnosticReply,
    FirmwareDownloadReply,
    ConfigurationGeometry,
    CapabilitiesReply,
    LogReply,
    ConsoleReply,
):
    _value.__module__ = "fujimod.usb"
