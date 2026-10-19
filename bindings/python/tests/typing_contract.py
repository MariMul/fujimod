"""Checker regression cases for the generated Rust and JavaScript boundaries."""

from typing import TYPE_CHECKING, assert_type

if TYPE_CHECKING:
    from fujimod import usb
    from fujimod.usb import app80

    def generated_values() -> None:
        bank = app80.PinBank(2)
        assert_type(bank.value, int)
        assert_type(app80.Command.ReadPins(bank), app80.Command.ReadPins)
        assert_type(app80.PeerAddress(bytearray(6)).data, bytes)
        app80.Command.ReadPins(2)  # ty: ignore[invalid-argument-type]  # Enforce the checked quantity boundary.
        bank.value = 3  # ty: ignore[invalid-assignment]  # Generated quantities are immutable.

    async def bridge_results(session: usb.Session) -> None:
        assert_type(await session.read_memory(0, 4), bytes)
        assert_type(await session.device_info(usb.InfoSelector.Product), str)
        await session.device_info(usb.CameraMode.Record)  # ty: ignore[invalid-argument-type]  # Enum groups remain distinct across the bridge.
        session.missing_operation()  # ty: ignore[unresolved-attribute]  # The bridge must not silently become dynamic.
