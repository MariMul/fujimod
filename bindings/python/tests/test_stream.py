"""Exercise the production Rust session and Python ownership through replay."""
import gc
import struct
import pytest

from fujimod import usb
from fujimod.usb import blocking

testing = pytest.importorskip("fujimod.usb.testing", reason="build bindings with --features testing")
E = testing.Exchange
TOKEN = 0x1234
LIMITS = usb.FirmwareLimits(memory_read=8, memory_write=8, configuration=256, console=256)


def control(kind, size, *, token=TOKEN, offset=0):
    return struct.pack(">BBIIH", kind, 0, offset, size, token)


def opened(steps=(), *, packet_bytes=4):
    return testing.session([
        E.control(control(0xC0, 8, token=0)),
        E.read(8, struct.pack("<HH", TOKEN, 0) + struct.pack(">I", packet_bytes)),
        *steps,
    ], LIMITS)


def memory_read(address, data):
    request = struct.pack("<9I", 0x200001, 0, 0, 0, 2, 0, 0, address, len(data))
    return [E.control(control(0xD0, len(request))), E.write(request),
            E.control(control(0xD1, 36 + len(data))), E.read(36 + len(data), request + data)]


def test_stream_exclusively_owns_session_and_returns_exact_tail():
    session, script = opened([*memory_read(0x1000, b"abcd"), *memory_read(0x1004, b"ef"),
                             E.control(control(0xC4, 2)), E.read(2, TOKEN.to_bytes(2, "little"))])
    with blocking.memory.read_chunks(session, 0x1000, 6) as reader:
        assert session.state == usb.SessionState.ReadingRange
        with pytest.raises(usb.SessionBusyError):
            session.ping()
        with pytest.raises(usb.SessionBusyError):
            session.close()
        assert next(reader) == b"abcd"
        assert reader.bytes_completed == 4
        assert next(reader) == b"ef"
        assert reader.bytes_completed == 6
        with pytest.raises(StopIteration):
            next(reader)
    assert session.state == usb.SessionState.Ready
    assert session.close() == TOKEN.to_bytes(2, "little")
    assert session.state == usb.SessionState.Closed
    assert script.remaining == 0


def test_early_close_restores_session_without_issuing_extra_commands():
    session, script = opened(memory_read(0x1000, b"abcd"))
    reader = blocking.memory.read_chunks(session, 0x1000, 8)
    assert next(reader) == b"abcd"
    reader.close()
    assert session.state == usb.SessionState.Ready
    assert reader.bytes_completed == 4
    assert list(reader) == []
    assert script.remaining == 0


def test_dropping_reader_returns_session_ownership_without_io():
    session, script = opened()
    reader = blocking.memory.read_chunks(session, 0x1000, 8)
    del reader
    gc.collect()
    assert session.state == usb.SessionState.Ready
    assert script.remaining == 0


def test_context_exception_returns_session_without_io():
    session, script = opened()
    with pytest.raises(RuntimeError, match="consumer stopped"):
        with blocking.memory.read_chunks(session, 0x1000, 8):
            raise RuntimeError("consumer stopped")
    assert session.state == usb.SessionState.Ready
    assert script.remaining == 0


def test_stream_failure_keeps_delivered_bytes_and_fences_session():
    second = memory_read(0x1004, b"efgh")
    second[-1] = E.disconnected(40, b"\x01\x00")
    session, script = opened([*memory_read(0x1000, b"abcd"), *second])
    sink = bytearray()
    reader = blocking.memory.read_chunks(session, 0x1000, 8)
    sink.extend(next(reader))
    with pytest.raises(usb.TransferError) as failure:
        next(reader)
    assert sink == b"abcd"
    assert reader.bytes_completed == 4
    assert failure.value.partial == b"\x01\x00"
    assert failure.value.transferred == 2
    assert session.state == usb.SessionState.CompletionUnknown
    with pytest.raises(usb.CompletionUnknownError):
        session.ping()
    reader.close()
    with pytest.raises(usb.CompletionUnknownError):
        blocking.memory.read_chunks(session,0x1000,4)
    assert script.remaining == 0


def test_invalid_range_and_oversize_direct_read_reject_before_io():
    session, script = opened()
    with pytest.raises(ValueError):
        blocking.memory.read_chunks(session, 0xFFFFFFFF, 2)
    with pytest.raises(ValueError):
        session.read_memory(0x1000, 6)  # negotiated limit 4; direct command never splits
    assert session.state == usb.SessionState.Ready
    assert script.remaining == 0


def test_selector_and_mode_types_cannot_be_substituted():
    session, script = opened()
    with pytest.raises(TypeError):
        session.device_info("Product")
    with pytest.raises(TypeError):
        session.device_info(4)
    with pytest.raises(TypeError):
        session.prepare_data_write(4, usb.DataReadMode.Pattern)
    assert session.state == usb.SessionState.Ready
    assert script.remaining == 0


def test_camera_error_is_structured_and_does_not_wait_for_completion():
    steps = memory_read(0x1000, b"abcd")
    reply = bytearray(struct.pack("<5I", 0x200001, 1, 0xFFFF, 0, 2))
    steps[-1] = E.read(40, reply)
    session, script = opened(steps)
    with pytest.raises(usb.DeviceCommandError) as failure:
        session.read_memory(0x1000, 4)
    assert (failure.value.command, failure.value.primary, failure.value.secondary) == (0x200001, 1, 0xFFFF)
    assert session.state == usb.SessionState.Ready
    assert script.remaining == 0


@pytest.mark.parametrize('steps,error', [
    ([E.control_disconnected(control(0xC4, 2))], usb.TransferError),
    ([E.control(control(0xC4, 2)), E.disconnected(2, b'\x12')], usb.TransferError),
    ([E.control(control(0xC4, 2)), E.read(2, b'\x12')], usb.ProtocolError),
])
def test_failed_close_is_unknown_and_cannot_be_retried_or_used(steps,error):
    session, script = opened(steps)
    with pytest.raises(error):
        session.close()
    assert session.state == usb.SessionState.CompletionUnknown
    for operation in (session.close, session.ping,
                      lambda: session.read_memory(0x1000, 4),
                      lambda: blocking.memory.read_chunks(session, 0x1000, 4)):
        with pytest.raises(usb.CompletionUnknownError):
            operation()
    assert session.state == usb.SessionState.CompletionUnknown
    assert script.remaining == 0


def test_close_after_incomplete_command_cannot_claim_closed_or_send_c4():
    session, script = opened([E.control(control(0xC2, 2)), E.disconnected(2, b'')])
    with pytest.raises(usb.TransferError):
        session.ping()
    with pytest.raises(usb.CompletionUnknownError):
        session.close()
    assert session.state == usb.SessionState.CompletionUnknown
    assert script.remaining == 0


def test_completed_close_preserves_opaque_reply_and_rejects_further_io():
    session, script = opened([E.control(control(0xC4, 2)), E.read(2, b'\xfe\xff')])
    assert session.close() == b'\xfe\xff'
    assert session.state == usb.SessionState.Closed
    for operation in (session.close, session.ping,
                      lambda: blocking.memory.read_chunks(session, 0x1000, 4)):
        with pytest.raises(usb.ClosedSessionError):
            operation()
    assert script.remaining == 0
