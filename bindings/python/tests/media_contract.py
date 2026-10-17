"""Media value contract shared by native Python and real Pyodide."""


def check_media_values():
    import json

    from fujimod import defs
    from fujimod.usb import app80 as a

    layouts = [
        ("display_option", 4),
        ("display_mode", 1),
        ("sound", 3),
        ("hdmi", 19),
        ("parameter_service", 1),
        ("charge_control", 3),
        ("charge_status", 2),
    ]
    slots = {
        codec: [(n * 5 + 7) % 32 for n in range(count)] for codec, count in layouts
    }
    entries = [
        dict(
            code=210 + n,
            label=codec,
            codec=codec,
            slots=slots[codec],
            enabled_modes=4,
            next_mode=0,
            firmware_kind=1,
            handler=0,
            parameter_table=0,
        )
        for n, (codec, _) in enumerate(layouts)
    ]
    document = dict(
        schema=1,
        firmware=dict(id="media-v2", model="Synthetic", version="2", sha256="ab" * 32),
        mailbox=dict(page=3, offset=576, configuration_limit=512),
        entries=entries,
    )
    definition = defs.service.Definition.from_json(
        json.dumps(document), defs.FirmwareIdentity("media-v2", "ab" * 32)
    )
    service = a.Service(definition)
    modes = defs.service.ServiceModes(4)

    def prepare(command):
        return service.prepare(modes, command)

    def logical(invocation, codec):
        return [invocation.raw[1 + s] for s in slots[codec]]

    def decode(invocation, codec, index, value):
        raw = bytearray(invocation.raw)
        raw[1 + slots[codec][index]] = value
        reply = invocation.decode(raw)
        assert reply.raw == raw
        return reply.value

    def invalid(function, kind):
        try:
            function()
        except a.CommandError as error:
            assert (
                isinstance(error.reason, a.CommandFailure.Argument)
                and error.reason.kind == kind
            )
        else:
            raise AssertionError("invalid media quantity accepted")

    for command, codec, expected in [
        (
            a.Command.DisplayOption(action=a.DisplayOptionAction.IdlePins()),
            "display_option",
            [0, 0, 0, 0],
        ),
        (
            a.Command.DisplayOption(
                action=a.DisplayOptionAction.ForwardPair(first=7, second=8)
            ),
            "display_option",
            [0, 1, 7, 8],
        ),
        (a.Command.DisplayMode(mode=a.DisplayMode.Option0), "display_mode", [0]),
        (a.Command.DisplayMode(mode=a.DisplayMode.Option1), "display_mode", [1]),
        (
            a.Command.ParameterService(enabled=a.Toggle.Disabled),
            "parameter_service",
            [0],
        ),
        (
            a.Command.ParameterService(enabled=a.Toggle.Enabled),
            "parameter_service",
            [1],
        ),
    ]:
        invocation = prepare(command)
        assert logical(invocation, codec) == expected
        assert isinstance(
            invocation.decode(invocation.raw).value, a.ReplyValue.NoPayload
        )
    invalid(lambda: a.SoundClip(23), a.ArgumentKind.SoundClip)
    assert a.SoundClip(22).value == 22
    for format, wire in [
        (a.SoundFormat.Format1, 0),
        (a.SoundFormat.Format2, 1),
        (a.SoundFormat.Format3, 2),
    ]:
        for clip in [0, 22]:
            invocation = prepare(
                a.Command.Sound(
                    action=a.SoundAction.Clip(clip=a.SoundClip(clip), format=format)
                )
            )
            assert logical(invocation, "sound") == [1, clip, wire]
            assert isinstance(
                invocation.decode(invocation.raw).value, a.ReplyValue.NoPayload
            )
        for track, base in [
            (a.SoundTrack.Track2, 23),
            (a.SoundTrack.Track3, 26),
            (a.SoundTrack.Track4, 29),
        ]:
            for operation, offset in [
                (a.SoundOperation.First, 0),
                (a.SoundOperation.Second, 1),
                (a.SoundOperation.Third, 2),
            ]:
                invocation = prepare(
                    a.Command.Sound(
                        action=a.SoundAction.Track(
                            track=track, operation=operation, format=format
                        )
                    )
                )
                assert invocation.raw[0] == 212
                assert logical(invocation, "sound") == [1, base + offset, wire]
    assert logical(prepare(a.Command.Sound(action=a.SoundAction.Noop())), "sound") == [
        0,
        0,
        0,
    ]

    for length in [0, 15, 17]:
        invalid(lambda: a.HdmiPacket(bytes(length)), a.ArgumentKind.HdmiPacketLength)
    invalid(lambda: a.HdmiValue(4096), a.ArgumentKind.HdmiValue)
    assert a.HdmiValue(4095).value == 4095
    original = bytearray(range(16))
    packet = a.HdmiPacket(original)
    original[:] = bytes([255] * 16)
    assert packet.data == bytes(range(16))
    invocation = prepare(a.Command.Hdmi(action=a.HdmiAction.Packet(packet=packet)))
    assert logical(invocation, "hdmi") == [0, 1, 0] + list(reversed(range(16)))
    assert isinstance(invocation.decode(invocation.raw).value, a.ReplyValue.NoPayload)
    invocation = prepare(
        a.Command.Hdmi(
            action=a.HdmiAction.Triplet(
                first=a.HdmiValue(0x123),
                second=a.HdmiValue(0x456),
                third=a.HdmiValue(4095),
            )
        )
    )
    assert (
        logical(invocation, "hdmi") == [1, 0, 0, 1, 0x23, 4, 0x56, 15, 255] + [0] * 10
    )
    assert (
        logical(prepare(a.Command.Hdmi(action=a.HdmiAction.ResetStub())), "hdmi")
        == [0] * 19
    )

    for action, expected in [
        (a.ChargeAction.Control(enabled=a.Toggle.Disabled), [0, 0, 255]),
        (a.ChargeAction.Control(enabled=a.Toggle.Enabled), [0, 1, 255]),
        (a.ChargeAction.EnableAndSetFlag(enabled=a.Toggle.Disabled), [1, 0, 255]),
        (a.ChargeAction.EnableAndSetFlag(enabled=a.Toggle.Enabled), [1, 1, 255]),
    ]:
        invocation = prepare(a.Command.ChargeControl(action=action))
        assert logical(invocation, "charge_control") == expected
        assert isinstance(
            invocation.decode(invocation.raw).value.status, a.ChargeResult.Unknown
        )
        for raw, result in [
            (0, a.ChargeResult.Success),
            (1, a.ChargeResult.Failed),
            (42, a.ChargeResult.Unknown),
            (255, a.ChargeResult.Unknown),
        ]:
            reply = decode(invocation, "charge_control", 2, raw)
            assert isinstance(reply, a.ReplyValue.ChargeApplied) and isinstance(
                reply.status, result
            )
            if result is a.ChargeResult.Unknown:
                assert reply.status.raw_status == raw
    invocation = prepare(a.Command.ChargeStatus(query=a.ChargeQuery.Presence))
    assert logical(invocation, "charge_status") == [0, 255]
    for raw, result in [
        (0, a.ChargePresence.Failed),
        (1, a.ChargePresence.Absent),
        (2, a.ChargePresence.Present),
        (3, a.ChargePresence.Unknown),
    ]:
        reply = decode(invocation, "charge_status", 1, raw)
        assert isinstance(reply, a.ReplyValue.ChargePresence) and isinstance(
            reply.status, result
        )
        if result is a.ChargePresence.Unknown:
            assert reply.status.raw_status == raw
    invocation = prepare(a.Command.ChargeStatus(query=a.ChargeQuery.State))
    assert logical(invocation, "charge_status") == [1, 255]
    for raw, result in [
        (0, a.ReportedChargeState.Failed),
        (1, a.ReportedChargeState.Unknown),
        (2, a.ReportedChargeState.Unknown),
        (3, a.ReportedChargeState.Value0),
        (4, a.ReportedChargeState.Values2To5),
        (5, a.ReportedChargeState.Value6),
        (6, a.ReportedChargeState.OtherValue),
        (255, a.ReportedChargeState.Unknown),
    ]:
        reply = decode(invocation, "charge_status", 1, raw)
        assert isinstance(reply, a.ReplyValue.ChargeState) and isinstance(
            reply.status, result
        )
        if result is a.ReportedChargeState.Unknown:
            assert reply.status.raw_status == raw
    try:
        a.Command.ParameterService(enabled=1)
    except TypeError:
        pass
    else:
        raise AssertionError("integer toggle accepted")
    try:
        a.SoundAction.Clip(clip=a.SoundClip(0), format=a.DisplayMode.Option0)
    except TypeError:
        pass
    else:
        raise AssertionError("wrong enum group accepted")
    return True
