"""Shared native/Pyodide service-transfer control contract."""


def check_transfer_values():
    import json
    from fujimod import defs
    from fujimod.usb import app80 as a

    layouts = [
        ("transfer_prepare", 1),
        ("transfer_select_operation", 2),
        ("transfer_ready", 1),
        ("transfer_select_channel", 2),
        ("transfer_start", 1),
    ]
    slots = {name: [(i * 5 + 7) % 32 for i in range(count)] for name, count in layouts}
    entries = [
        dict(
            code=180 + n,
            label=name,
            codec=name,
            slots=slots[name],
            enabled_modes=4,
            next_mode=0,
            firmware_kind=1,
            handler=0,
            parameter_table=0,
        )
        for n, (name, _) in enumerate(layouts)
    ]
    doc = dict(
        schema=1,
        firmware=dict(
            id="transfer-b", model="Synthetic", version="2", sha256="ab" * 32
        ),
        mailbox=dict(page=3, offset=576, configuration_limit=512),
        entries=entries,
    )
    service = a.Service(
        defs.service.Definition.from_json(
            json.dumps(doc), defs.FirmwareIdentity("transfer-b", "ab" * 32)
        )
    )
    modes = defs.service.ServiceModes(4)

    def prepare(command):
        return service.prepare(modes, command)

    def logical(p, codec):
        return [p.raw[1 + i] for i in slots[codec]]

    def decode(p, codec, index, raw):
        data = bytearray(p.raw)
        data[1 + slots[codec][index]] = raw
        reply = p.decode(data)
        assert reply.raw == data
        return reply.value

    cases = [
        (
            a.Command.TransferPrepare(),
            "transfer_prepare",
            [255],
            0,
            a.ReplyValue.TransferPrepared,
        ),
        (
            a.Command.TransferReady(),
            "transfer_ready",
            [255],
            0,
            a.ReplyValue.TransferReadiness,
        ),
        (
            a.Command.TransferStart(),
            "transfer_start",
            [255],
            0,
            a.ReplyValue.TransferStarted,
        ),
    ]
    for operation, wire in [
        (a.TransferOperation.Operation0, 0),
        (a.TransferOperation.Operation1, 1),
        (a.TransferOperation.Operation2, 2),
        (a.TransferOperation.Operation3, 3),
        (a.TransferOperation.Operation6, 255),
    ]:
        cases.append(
            (
                a.Command.TransferSelectOperation(operation=operation),
                "transfer_select_operation",
                [wire, 255],
                1,
                a.ReplyValue.TransferOperationSelected,
            )
        )
    for channel, wire in [
        (a.TransferChannel.Operation1, 0),
        (a.TransferChannel.Operation3, 1),
        (a.TransferChannel.Operation4, 2),
    ]:
        cases.append(
            (
                a.Command.TransferSelectChannel(channel=channel),
                "transfer_select_channel",
                [wire, 255],
                1,
                a.ReplyValue.TransferChannelSelected,
            )
        )
    for command, codec, expected, index, reply_type in cases:
        p = prepare(command)
        assert logical(p, codec) == expected
        for raw in [0, 1, 2, 3, 255]:
            value = decode(p, codec, index, raw)
            assert isinstance(value, reply_type)
            if codec == "transfer_prepare":
                variants = {
                    0: a.TransferPrepareResult.Success,
                    2: a.TransferPrepareResult.PreparationError,
                    3: a.TransferPrepareResult.Status7,
                }
                cls = variants.get(raw, a.TransferPrepareResult.Unknown)
            else:
                variants = {0: a.TransferResult.Success, 1: a.TransferResult.Failed}
                cls = variants.get(raw, a.TransferResult.Unknown)
            assert isinstance(value.result, cls)
            if cls in [a.TransferPrepareResult.Unknown, a.TransferResult.Unknown]:
                assert value.result.raw_status == raw
    for create in [
        lambda: a.Command.TransferSelectOperation(operation=1),
        lambda: a.Command.TransferSelectChannel(channel=a.TransferOperation.Operation1),
    ]:
        try:
            create()
        except TypeError:
            pass
        else:
            raise AssertionError("wrong transfer selector type accepted")
    return True
