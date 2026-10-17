"""Executed in real Pyodide by pyodide.mjs; USB is replaced before import."""

import asyncio
from fujimod import usb
from fujimod.usb import memory, data, research, app80, tracing
from _fixture import state, log


async def check():
    assert usb.InfoSelector.Product != 4
    assert usb.DataWriteMode.Echo != usb.DataReadMode.Echo
    limits = usb.FirmwareLimits(
        memory_read=256, memory_write=256, configuration=256, console=256
    )
    devices = await usb.discover(usb.DeviceFilter.Diagnostic)
    assert len(devices) == 1 and devices[0].vendor_id == 0x04CB
    assert devices[0].bus_id is None and devices[0].ports is None
    device = await usb.request_device(usb.DeviceFilter.Diagnostic)
    choices = await usb.interfaces(device)
    assert len(choices) == 1 and (
        choices[0].configuration,
        choices[0].interface,
        choices[0].alternate,
    ) == (1, 2, 3)
    trace = tracing.Trace(128, 8192)
    session = await usb.Session.open(device, choices[0], limits, trace=trace)
    assert session.state == usb.SessionState.Ready and session.token == 0x1234
    assert await session.device_info(usb.InfoSelector.Product) == "SYNTHETIC"
    assert await session.read_data(123, 5) == bytes([123, 124, 125, 126, 127])
    try:
        await session.device_info("Product")
    except TypeError:
        pass
    else:
        raise AssertionError("string selector accepted")
    try:
        await session.read_data(0, 1.0)
    except TypeError:
        pass
    else:
        raise AssertionError("fractional Python quantity accepted")
    request = research.Request(
        research.CommandCode(0xDEADBEEF), bytes(16), reply_capacity=32
    )
    assert request.code == research.CommandCode(0xDEADBEEF) and request.body == bytes(
        16
    )
    assert app80.Request(app80.CommandCode(2), b"hi").raw == b"\x02hi" + bytes(30)
    window = usb.DataWindow(0x1000, 0x1000, 32, 4, usb.OffsetMode.Checked)
    async with data.read_chunks(session, window, 0x1000, 6) as reader:
        assert session.state == usb.SessionState.ReadingRange
        assert [chunk async for chunk in reader] == [bytes(range(4)), b"\x04\x05"]
        assert reader.bytes_completed == 6
    mutable = bytearray(b"abc")
    result = await data.upload(
        session, memoryview(mutable), piece_bytes=2, mode=usb.DataWriteMode.Preserve
    )
    assert result.bytes_written == 3 and result.acknowledgement == b"\x01\x02\x03\x04"
    assert result.setup.status == usb.CommandStatus.Success
    assert result.setup.fields == [0, 0, 0, 0]
    assert await session.close() == b"\xaa\xbb"
    assert session.state == usb.SessionState.Closed
    batch = trace.drain()
    assert batch.dropped_events == 0 and batch.dropped_bytes == 0
    assert any(
        e.stage == tracing.Stage.BulkIn and e.endpoint == 0x82 for e in batch.events
    )
    assert all(
        type(e.sequence) is int and e.timestamp > 1_500_000_000 for e in batch.events
    )
    try:
        await session.ping()
    except usb.ClosedSessionError:
        pass
    else:
        raise AssertionError("closed session accepted request")
    second = await usb.Session.open(device, choices[0], limits)
    state.hangControl = 0xC2
    task = asyncio.create_task(second.ping())
    while not any(entry[0] == "control" and entry[1] == 0xC2 for entry in log):
        await asyncio.sleep(0)
    task.cancel()
    try:
        await task
    except asyncio.CancelledError:
        pass
    else:
        raise AssertionError("cancel did not propagate")
    assert second.state == usb.SessionState.CompletionUnknown
    try:
        await second.close()
    except usb.CompletionUnknownError:
        pass
    else:
        raise AssertionError("cancelled exchange allowed cleanup")
    second.abandon()
    assert second.state == usb.SessionState.Abandoned
    assert sum(entry[0] == "control" and entry[1] == 0xC4 for entry in log) == 1
    return True


async def check_pending_reader():
    state.hangControl = None
    device = (await usb.discover(usb.DeviceFilter.Diagnostic))[0]
    interface = (await usb.interfaces(device))[0]
    limits = usb.FirmwareLimits(
        memory_read=256, memory_write=256, configuration=256, console=256
    )
    session = await usb.Session.open(device, interface, limits)
    state.hangControl = 0xE1
    reader = data.read_chunks(
        session,
        usb.DataWindow(0x1000, 0x1000, 16, 4, usb.OffsetMode.Checked),
        0x1000,
        8,
    )
    closed = object()

    async def read_one():
        try:
            return await anext(reader)
        except usb.ClosedSessionError:
            return closed

    pending = asyncio.create_task(read_one())
    while state.kind != 0xE1:
        await asyncio.sleep(0)
    await asyncio.gather(reader.aclose(), reader.aclose())
    assert await pending is closed
    assert session.state == usb.SessionState.CompletionUnknown
    assert reader.bytes_completed == 0
    assert [chunk async for chunk in reader] == []
    session.abandon()
    state.hangControl = None
    trace = tracing.Trace(64, 8192)
    session = await usb.Session.open(
        device, interface, limits, timeout_ms=10, trace=trace
    )
    state.hangControl = 0xC2
    try:
        await session.ping()
    except usb.TransferError as error:
        assert error.partial == b"" and error.transferred is None
    else:
        raise AssertionError("hung control completed")
    assert session.state == usb.SessionState.CompletionUnknown
    failures = [e for e in trace.drain().events if e.failure_kind is not None]
    assert any(
        e.failure_kind == tracing.FailureKind.Timeout and e.error for e in failures
    )
    session.abandon()
    return True


async def check_commands():
    state.hangControl = None
    device = (await usb.discover(usb.DeviceFilter.Diagnostic))[0]
    interface = (await usb.interfaces(device))[0]
    limits = usb.FirmwareLimits(
        memory_read=4, memory_write=4, configuration=256, console=256
    )
    s = await usb.Session.open(device, interface, limits)
    await memory.write_range(s, 0x2000, b"abcdef")
    assert await memory.read_range(s, 0x2000, 6) == b"abcdef"
    await s.write_memory(0x2000, b"xyz")
    assert await s.read_memory(0x2000, 3) == b"xyz"
    geometry = await s.configuration_geometry(usb.ConfigurationView.Ram)
    assert (geometry.capacity, geometry.page_bytes) == (4096, 256) and len(
        geometry.raw
    ) == 36
    await s.write_configuration(usb.ConfigurationView.Ram, 0x181, b"abc", page=2)
    assert (
        await s.read_configuration(usb.ConfigurationView.Ram, 0x181, 3, page=2)
        == b"abc"
    )
    mailbox = app80.Mailbox(page=2, offset=0x180, configuration_limit=256)
    request = app80.Request(app80.CommandCode(1), b"hi")
    assert await app80.exchange(s, mailbox, request) == request.raw
    result = await s.research_command(
        research.Request(
            research.CommandCode(0xDEADBEEF), bytes(range(16)), reply_capacity=32
        )
    )
    assert result.code == research.CommandCode(0xDEADBEEF) and result.body == bytes(
        range(16)
    )
    assert result.status == usb.CommandStatus.DeviceError and (
        result.primary,
        result.secondary,
    ) == (1, 0xFFFF)
    reply = await s.diagnostic_query(usb.DiagnosticTag.bulk_source())
    assert reply.status == usb.CommandStatus.DeviceError and reply.fields == [
        9,
        8,
        7,
        6,
    ]
    assert (
        await s.prepare_data_read(usb.DataReadMode.Echo)
    ).status == usb.CommandStatus.Success
    assert (
        await s.prepare_data_write(4, usb.DataWriteMode.FillOnes)
    ).status == usb.CommandStatus.Success
    await s.write_data(b"xyz")
    assert await s.empty_response() == b"\x01\x02\x03\x04"
    caps = await s.prepare_capabilities()
    assert caps.byte_length == 48 and len(caps.raw) == 32
    assert (
        await s.firmware_download_status()
    ).state == usb.FirmwareDownloadState.Writing
    assert (
        await s.clear_firmware_download_status()
    ).state == usb.FirmwareDownloadState.Normal
    assert await s.send_key("S1", usb.KeyState.On) == b""
    assert await s.request_mode(usb.CameraMode.Record) == b""
    assert await s.command_event("TEST") == b""
    await s.submit_console(b"help\n")
    log_reply = await s.read_log(8)
    assert log_reply.status == 7 and log_reply.data == b"log"
    console = await s.poll_console(8)
    assert (
        console.progress == usb.ConsoleProgress.Unknown
        and console.unknown_progress == 42
    )
    assert console.status == 7 and console.output == b"log"
    assert s.state == usb.SessionState.Ready
    assert await s.close() == b"\xaa\xbb"
    return True


async def run():
    await check()
    await check_pending_reader()
    await check_commands()
    return True
