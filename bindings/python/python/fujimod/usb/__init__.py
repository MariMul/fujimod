"""Awaitable Pyodide API over the same Rust core as native FujiMod."""

from ._errors import (
    UsbError,
    TransferError,
    ProtocolError,
    CompletionUnknownError,
    DeviceCommandError,
    ClosedSessionError,
    SessionBusyError,
)
from ._types import (
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
)
from .._bridge import (
    Handle,
    jsusb,
    call,
    uint,
    enum_in,
    enum_out,
    buffer_in,
    buffer_out,
    snapshot,
    unwrap,
    await_result,
)


class Session(Handle):
    def __init__(self):
        raise TypeError("use await Session.open(...)")

    @classmethod
    async def open(
        cls,
        device,
        interface,
        limits,
        *,
        timeout_ms=5000,
        max_reply_bytes=8388608,
        trace=None,
    ):
        options = call(
            jsusb.TransferOptions.new, uint(timeout_ms), uint(max_reply_bytes)
        )
        try:
            if trace is not None:
                call(options.setTrace, unwrap(trace, tracing.Trace))
            promise = call(
                jsusb.Session.open,
                unwrap(device, Device),
                unwrap(interface, InterfaceSelection),
                unwrap(limits, FirmwareLimits),
                options,
            )
            # The owner exists only once C0 completes. Consume a cancelled
            # open's outcome and abandon it without any cleanup request.
            import asyncio
            from .._bridge import begin, wait_pending, Success

            pending = begin(promise)
            try:
                return cls._wrap(await wait_pending(pending))
            except asyncio.CancelledError:
                outcome = pending.result()
                if isinstance(outcome, Success):
                    outcome.value.abandon()
                    outcome.value.free()
                raise
        finally:
            options.free()

    state = property(lambda self: enum_out(self._js.state, SessionState))
    token = property(lambda self: self._js.sessionId)
    packet_bytes = property(lambda self: self._js.packetBytes)

    async def _call(self, method, *args, decode=lambda v: v):
        return decode(await await_result(call(method, *args), self._js.cancelPending))

    async def ping(self):
        return await self._call(self._js.ping, decode=buffer_out)

    async def close(self):
        return await self._call(self._js.close, decode=buffer_out)

    def abandon(self):
        if self.state in (SessionState.Closed, SessionState.Abandoned):
            raise ClosedSessionError("session is no longer available")
        call(self._js.abandon)

    async def device_info(self, selector):
        return await self._call(
            self._js.deviceInfo,
            enum_in(selector, InfoSelector),
            decode=lambda v: snapshot(v, lambda r: r.text),
        )

    async def read_memory(self, address, length):
        return await self._call(
            self._js.readMemory, uint(address), uint(length), decode=buffer_out
        )

    async def write_memory(self, address, data):
        await self._call(self._js.writeMemory, uint(address), buffer_in(data))

    async def configuration_geometry(self, view):
        return await self._call(
            self._js.configurationGeometry,
            enum_in(view, ConfigurationView),
            decode=ConfigurationGeometry._from_js,
        )

    async def read_configuration(self, view, offset, length, *, page=0):
        return await self._call(
            self._js.readConfiguration,
            enum_in(view, ConfigurationView),
            uint(page),
            uint(offset),
            uint(length),
            decode=buffer_out,
        )

    async def write_configuration(self, view, offset, data, *, page=0):
        await self._call(
            self._js.writeConfiguration,
            enum_in(view, ConfigurationView),
            uint(page),
            uint(offset),
            buffer_in(data),
        )

    async def diagnostic_query(self, tag):
        return await self._call(
            self._js.diagnosticQuery,
            unwrap(tag, DiagnosticTag),
            decode=DiagnosticReply._from_js,
        )

    async def prepare_data_read(self, mode):
        return await self._call(
            self._js.prepareDataRead,
            enum_in(mode, DataReadMode),
            decode=DiagnosticReply._from_js,
        )

    async def prepare_data_write(self, length, mode):
        return await self._call(
            self._js.prepareDataWrite,
            uint(length),
            enum_in(mode, DataWriteMode),
            decode=DiagnosticReply._from_js,
        )

    async def read_data(self, offset, length):
        return await self._call(
            self._js.readData, uint(offset), uint(length), decode=buffer_out
        )

    async def write_data(self, data):
        await self._call(self._js.writeData, buffer_in(data))

    async def empty_response(self):
        return await self._call(self._js.emptyResponse, decode=buffer_out)

    async def prepare_capabilities(self):
        return await self._call(
            self._js.prepareCapabilities, decode=CapabilitiesReply._from_js
        )

    async def firmware_download_status(self):
        return await self._call(
            self._js.firmwareDownloadStatus, decode=FirmwareDownloadReply._from_js
        )

    async def clear_firmware_download_status(self):
        return await self._call(
            self._js.clearFirmwareDownloadStatus, decode=FirmwareDownloadReply._from_js
        )

    async def send_key(self, name, state):
        if not isinstance(name, str):
            raise TypeError("expected event name text")
        return await self._call(
            self._js.sendKey, name, enum_in(state, KeyState), decode=buffer_out
        )

    async def request_mode(self, mode):
        return await self._call(
            self._js.requestMode, enum_in(mode, CameraMode), decode=buffer_out
        )

    async def command_event(self, argument):
        if not isinstance(argument, str):
            raise TypeError("expected command event text")
        return await self._call(self._js.commandEvent, argument, decode=buffer_out)

    async def submit_console(self, data):
        await self._call(self._js.submitConsole, buffer_in(data))

    async def read_log(self, capacity):
        return await self._call(
            self._js.readLog, uint(capacity), decode=LogReply._from_js
        )

    async def poll_console(self, capacity):
        return await self._call(
            self._js.pollConsole, uint(capacity), decode=ConsoleReply._from_js
        )

    async def research_command(self, request):
        return await self._call(
            self._js.researchCommand,
            unwrap(request, research.Request),
            decode=research.Reply._from_js,
        )


async def discover(filter):
    return [
        Device._wrap(v)
        for v in await await_result(call(jsusb.discover, enum_in(filter, DeviceFilter)))
    ]


async def request_device(filter):
    """Browser picker: invoke directly from a user gesture."""
    value = await await_result(call(jsusb.requestDevice, enum_in(filter, DeviceFilter)))
    return None if value is None else Device._wrap(value)


async def interfaces(device):
    return [
        InterfaceSelection._from_js(v)
        for v in await await_result(call(unwrap(device, Device).interfaces))
    ]


from ._reader import Reader
from . import app80, data, memory, research, tracing

__all__ = [
    "Session",
    "Reader",
    "discover",
    "request_device",
    "interfaces",
    "memory",
    "data",
    "app80",
    "research",
    "tracing",
    "UsbError",
    "TransferError",
    "ProtocolError",
    "CompletionUnknownError",
    "DeviceCommandError",
    "ClosedSessionError",
    "SessionBusyError",
    "InfoSelector",
    "ConfigurationView",
    "KeyState",
    "CameraMode",
    "DataReadMode",
    "DataWriteMode",
    "OffsetMode",
    "DeviceFilter",
    "SessionState",
    "CommandStatus",
    "ConsoleProgress",
    "FirmwareDownloadState",
    "FirmwareLimits",
    "InterfaceSelection",
    "Device",
    "DiagnosticTag",
    "DataWindow",
    "DiagnosticReply",
    "FirmwareDownloadReply",
    "ConfigurationGeometry",
    "CapabilitiesReply",
    "LogReply",
    "ConsoleReply",
]
