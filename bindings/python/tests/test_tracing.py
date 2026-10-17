"""Trace the production Rust session over synthetic USB only."""
import pytest
from fujimod import usb
from fujimod.usb import blocking, tracing
from test_stream import E, control, LIMITS, memory_read


def opened(steps=(),*,trace):
    return usb.testing.session([
        E.control(control(0xc0,8,token=0)),E.read(8,b'\x34\x12\x00\x00\x00\x00\x01\x00'),*steps,
    ],LIMITS,trace=trace)


def test_lifecycle_trace_is_typed_and_survives_session_close():
    trace=tracing.Trace(max_events=32,max_bytes=4096)
    session,script=opened([E.control(control(0xc4,2)),E.read(2,b'\x34\x12')],trace=trace)
    session.close()
    batch=trace.drain()
    assert batch.dropped_events == batch.dropped_bytes == 0
    events=batch.events
    assert len(events) == 8
    assert [e.sequence for e in events] == list(range(8))
    assert all(events[i].transfer==events[i+1].transfer for i in range(0,8,2))
    assert events[0].stage == tracing.Stage.Control
    assert events[0].phase == tracing.Phase.Submitted
    assert events[0].request_kind == tracing.RequestKind.Open
    assert events[0].endpoint == 0
    assert events[0].requested == 12
    assert events[0].timestamp > 0
    assert events[-1].stage == tracing.Stage.BulkIn
    assert events[-1].transferred == 2
    assert events[-1].data == b'\x34\x12'
    with pytest.raises(AttributeError): events[0].sequence=3
    assert trace.drain().events == []
    assert script.remaining == 0


def test_failed_transfer_keeps_partial_bytes_and_fencing():
    trace=tracing.Trace(32,4096)
    session,script=opened([E.control(control(0xc2,2)),E.disconnected(2,b'\x34')],trace=trace)
    with pytest.raises(usb.TransferError): session.ping()
    event=trace.drain().events[-1]
    assert event.phase == tracing.Phase.Failed
    assert event.failure_kind == tracing.FailureKind.Transfer
    assert event.data == b'\x34'
    assert event.transferred == 1
    assert session.state == usb.SessionState.CompletionUnknown
    assert script.remaining == 0


def test_open_failure_can_be_drained_without_a_session():
    trace=tracing.Trace(32,4096)
    with pytest.raises(usb.TransferError):
        usb.testing.session([E.control_disconnected(control(0xc0,8,token=0))],LIMITS,trace=trace)
    events=trace.drain().events
    assert len(events) == 2
    assert events[-1].phase == tracing.Phase.Failed
    assert events[0].data == control(0xc0,8,token=0)


def test_trace_overflow_reports_loss_without_changing_camera_exchange():
    trace=tracing.Trace(max_events=2,max_bytes=4)
    session,script=opened(trace=trace)
    batch=trace.drain()
    assert batch.dropped_events == 2
    assert batch.dropped_bytes == 20  # full 12-byte request and 8-byte response
    assert len(batch.events) == 2
    assert all(e.data==b'' for e in batch.events)
    assert session.state == usb.SessionState.Ready
    assert trace.drain().dropped_events == 0
    assert script.remaining == 0


def test_trace_can_be_drained_while_range_reader_owns_session():
    trace=tracing.Trace(32,4096)
    session,script=opened(memory_read(0x1000,b'abcd'),trace=trace)
    with blocking.memory.read_chunks(session,0x1000,4) as reader:
        assert session.state == usb.SessionState.ReadingRange
        assert len(trace.drain().events)==4
        assert next(reader)==b'abcd'
        events=trace.drain().events
        assert events[2].stage == tracing.Stage.BulkOut
        assert events[2].data[:4] == b'\x01\x00\x20\x00'
        assert events[-1].data.endswith(b'abcd')
    assert session.state == usb.SessionState.Ready
    assert script.remaining == 0


def test_invalid_trace_bounds_reject_before_allocating_or_opening():
    for events,bytes_ in [(0,1),(1,0)]:
        with pytest.raises(ValueError): tracing.Trace(events,bytes_)


def test_completed_usb_transfer_does_not_certify_a_valid_protocol_reply():
    trace=tracing.Trace(32,4096)
    session,script=opened([E.control(control(0xc2,2)),E.read(2,b'\x34')],trace=trace)
    with pytest.raises(usb.ProtocolError): session.ping()
    event=trace.drain().events[-1]
    assert event.phase == tracing.Phase.Completed
    assert event.data == b'\x34'
    assert session.state == usb.SessionState.CompletionUnknown
    assert script.remaining == 0
