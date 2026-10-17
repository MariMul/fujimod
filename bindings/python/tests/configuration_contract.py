"""Shared native/Pyodide configuration command contract."""


def check_configuration_values():
    import json

    from fujimod import defs
    from fujimod.usb import app80 as a

    layouts = [
        ("configuration_flags", 11),
        ("configuration_mode", 2),
        ("configuration_pack_options", 4),
        ("configuration_read_options", 4),
    ]
    slots = {name: [(i * 5 + 7) % 32 for i in range(n)] for name, n in layouts}
    doc = {
        "schema": 1,
        "firmware": {
            "id": "config-b",
            "model": "Synthetic",
            "version": "2",
            "sha256": "ab" * 32,
        },
        "mailbox": {"page": 3, "offset": 576, "configuration_limit": 512},
        "entries": [
            {
                "code": 200 + n,
                "label": name,
                "codec": name,
                "slots": slots[name],
                "enabled_modes": 4,
                "next_mode": 0,
                "firmware_kind": 1,
                "handler": 0,
                "parameter_table": 0,
            }
            for n, (name, _) in enumerate(layouts)
        ],
    }
    service = a.Service(
        defs.service.Definition.from_json(
            json.dumps(doc), defs.FirmwareIdentity("config-b", "ab" * 32)
        )
    )
    modes = defs.service.ServiceModes(4)

    def prepare(command, codec, expected):
        p = service.prepare(modes, command)
        assert [p.raw[1 + i] for i in slots[codec]] == expected
        return p

    for bank, b in [(a.ConfigurationBank.Bank0, 0), (a.ConfigurationBank.Bank1, 1)]:
        for option, o in [(a.Toggle.Disabled, 0), (a.Toggle.Enabled, 1)]:
            p = prepare(
                a.Command.ReadConfigurationFlags(bank=bank, option=option),
                "configuration_flags",
                [0, b, o] + [255] * 8,
            )
            value = p.decode(p.raw).value
            assert isinstance(value, a.ReplyValue.ConfigurationFlags)
            assert bytes(value.raw_data) == bytes([255] * 8)
            data = bytearray(p.raw)
            expected = bytes([0, 1, 127, 128, 253, 254, 255, 42])
            for i, byte in enumerate(expected):
                data[1 + slots["configuration_flags"][3 + i]] = byte
            assert bytes(p.decode(data).value.raw_data) == expected
            p = prepare(
                a.Command.ApplyConfigurationFlags(bank=bank, option=option),
                "configuration_flags",
                [1, b, o] + [0] * 8,
            )
            assert isinstance(p.decode(p.raw).value, a.ReplyValue.NoPayload)
    for flag, f in [(a.Toggle.Disabled, 0), (a.Toggle.Enabled, 1)]:
        for m in range(4):
            p = prepare(
                a.Command.SelectConfigurationMode(
                    flag=flag, mode=getattr(a.ConfigurationMode, f"Mode{m}")
                ),
                "configuration_mode",
                [f, m],
            )
            assert isinstance(p.decode(p.raw).value, a.ReplyValue.NoPayload)
        for option, o in [(a.Toggle.Disabled, 0), (a.Toggle.Enabled, 1)]:
            for upper, lower in [(0, 255), (255, 0), (137, 171)]:
                p = prepare(
                    a.Command.PackConfigurationOptions(
                        flag=flag, option=option, upper=upper, lower=lower
                    ),
                    "configuration_pack_options",
                    [f, o, upper, lower],
                )
                assert isinstance(p.decode(p.raw).value, a.ReplyValue.NoPayload)
    p = prepare(
        a.Command.ReadConfigurationOptions(), "configuration_read_options", [255] * 4
    )
    assert p.decode(p.raw).value.raw_value == 0xFFFFFFFF
    data = bytearray(p.raw)
    for i, byte in enumerate([137, 171, 205, 239]):
        data[1 + slots["configuration_read_options"][i]] = byte
    assert p.decode(data).value.raw_value == 0x89ABCDEF
    for create in [
        lambda: a.Command.ReadConfigurationFlags(bank=1, option=a.Toggle.Enabled),
        lambda: a.Command.SelectConfigurationMode(
            flag=a.Toggle.Enabled, mode=a.ConfigurationBank.Bank0
        ),
        lambda: a.Command.PackConfigurationOptions(
            flag=a.Toggle.Enabled, option=1, upper=0, lower=0
        ),
    ]:
        try:
            create()
        except TypeError:
            pass
        else:
            raise AssertionError("invalid configuration selector type accepted")
    return True
