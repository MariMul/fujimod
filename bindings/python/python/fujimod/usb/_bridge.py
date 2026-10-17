"""Language conversions only. Every USB operation executes in the Rust module."""

import asyncio
from dataclasses import dataclass
from js import Uint8Array
from pyodide.ffi import JsException, to_js
from _fujimod_js import usb as jsusb
from ._errors import (
    UsbError,
    TransferError,
    ProtocolError,
    CompletionUnknownError,
    DeviceCommandError,
    ClosedSessionError,
    SessionBusyError,
)


def uint(value, bits=32):
    if type(value) is not int:
        raise TypeError("expected an integer quantity")
    if not 0 <= value < (1 << bits):
        raise OverflowError(f"quantity must fit u{bits}")
    return value


def buffer_in(value):
    # Copy before yielding: JS/Rust must never retain a Python-memory view.
    return Uint8Array.new(to_js(memoryview(value).cast("B"), create_pyproxies=False))


def buffer_out(value):
    return bytes(value.to_py())


def enum_in(value, cls, group=None):
    if type(value) is not cls:
        raise TypeError(f"expected {cls.__name__}")
    return getattr(getattr(jsusb, group or cls.__name__), value.name)


def enum_out(value, cls, group=None):
    names = getattr(jsusb, group or cls.__name__)
    return {getattr(names, member.name): member for member in cls}[value]


def convert_error(value):
    kind = getattr(value, "kind", None)
    kinds = jsusb.ErrorKind
    cls = {
        kinds.Transfer: TransferError,
        kinds.Protocol: ProtocolError,
        kinds.DeviceStatus: DeviceCommandError,
        kinds.CompletionUnknown: CompletionUnknownError,
        kinds.InvalidArgument: ValueError,
        kinds.Busy: SessionBusyError,
        kinds.Closed: ClosedSessionError,
        kinds.Abandoned: ClosedSessionError,
        kinds.Cancelled: ClosedSessionError,
    }.get(kind, UsbError)
    error = cls(str(getattr(value, "message", value)))
    for name in (
        "command",
        "primary",
        "secondary",
        "transferred",
        "expected",
        "actual",
        "available",
        "declared",
        "requested",
        "limit",
        "reserved",
    ):
        field = getattr(value, name, None)
        if field is not None:
            setattr(error, name, field)
    partial = getattr(value, "partial", None)
    if partial is not None:
        error.partial = buffer_out(partial)
    if cls is TransferError:
        if not hasattr(error, "partial"):
            error.partial = b""
        if not hasattr(error, "transferred"):
            error.transferred = None
    cause = getattr(value, "cause", None)
    if cause is not None:
        error.transfer_error = convert_error(cause)
    return error


def call(function, *args):
    try:
        return function(*args)
    except JsException as error:
        raise convert_error(error.js_error) from error


async def settled(future):
    """Finish cancellation cleanup even when the caller is cancelled again."""
    while not future.done():
        try:
            await asyncio.shield(future)
        except asyncio.CancelledError:
            continue
        except Exception:
            break
    # Retrieve the outcome so a rejected JS Promise never becomes unhandled.
    if not future.cancelled():
        try:
            future.result()
        except Exception:
            pass


@dataclass(frozen=True)
class Success:
    value: object


@dataclass(frozen=True)
class Failure:
    error: Exception


def begin(promise):
    async def capture():
        try:
            return Success(await promise)
        except Exception as error:
            return Failure(error)

    # A shielded inner task always completes with an explicit outcome, avoiding
    # asyncio's exception-on-cancel logging while we consume cancellation.
    return asyncio.create_task(capture())


async def wait_pending(future, cancel=None):
    try:
        outcome = await asyncio.shield(future)
    except asyncio.CancelledError:
        if cancel is not None:
            cancel()
        await settled(future)
        raise
    if isinstance(outcome, Failure):
        if isinstance(outcome.error, JsException):
            raise convert_error(outcome.error.js_error) from outcome.error
        raise outcome.error
    return outcome.value


async def await_result(promise, cancel=None):
    return await wait_pending(begin(promise), cancel)


class Handle:
    """An owned wasm-bindgen object. Destruction releases memory, never USB I/O."""

    @classmethod
    def _wrap(cls, value):
        obj = object.__new__(cls)
        obj._js = value
        return obj

    def __del__(self):
        value = getattr(self, "_js", None)
        if value is not None:
            value.free()


def snapshot(value, convert):
    try:
        return convert(value)
    finally:
        value.free()


def unwrap(value, cls):
    if type(value) is not cls:
        raise TypeError(f"expected {cls.__name__}")
    return value._js
