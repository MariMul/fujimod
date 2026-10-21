"""Native emulator package and board execution contracts."""

import gc
import hashlib
import importlib
import os
import struct
import weakref
from typing import cast

import pytest
from fujimod import catalog, emulator, usb
from fujimod._native import emulator as native
from fujimod.emulator import Asset, Board, Definition, Image, make_board
from fujimod.emulator.board import Session, _boot_report

NATIVE_EXPORTS = frozenset({
    "load_input_assets",
    "verify_input_assets",
    "Sha256",
    "sha256_many",
    "NativeError",
    "Board",
    "Checkpoint",
    "Xp4",
    "AnalogInputs",
    "CardMedium",
    "NandMedium",
    "DeviceContext",
    "persistent_packet",
    "normalize_card_files",
    "zero_row_edges",
    "mono12_phase_gain",
    "jpeg_encode_yycbcr",
    "jpeg_decode_yycbcr",
    "FrontierEntry",
    "Intervention",
    "gpu_ui_render",
    "graphics_memo_len",
    "graphics_memo_clear",
    "UiBatch",
    "ui_batch",
    "ui_initialize",
    "clear_path_descriptor",
    "clear_parse",
    "clear_render",
    "math_check",
    "ui_stream_draws",
    "copy_untile",
    "ui_stream_parse",
    "DeviceRefusal",
    "HostImage",
    "virtual_scene_yycbcr",
    "virtual_scene_histogram",
    "virtual_scene_statistics",
    "FilmStage",
    "decode_display_rgb",
    "render_scanout_snapshot",
    "scanout_snapshot_state",
    "method4_decode",
    "ui_fill_counts",
    "ui_composite",
    "ui_composite_tiled",
    "UNICORN_ABI",
    "VERSION",
    "RUSTC_VERSION",
    "TARGET",
    "PYO3_VERSION",
    "VIRTUAL_SCENE_PROFILE",
    "STATISTICS_PROFILE",
})


def test_emulator_is_a_nested_native_module_with_complete_exports() -> None:
    assert len(NATIVE_EXPORTS) == 53
    importlib.import_module("fujimod")
    assert importlib.import_module("fujimod.catalog") is catalog
    assert importlib.import_module("fujimod.usb") is usb
    assert importlib.import_module("fujimod.emulator") is emulator
    assert importlib.import_module("fujimod._native.emulator") is native
    assert (
        frozenset(name for name in vars(native) if not name.startswith("_"))
        == NATIVE_EXPORTS
    )


def _execution_graph() -> dict[str, object]:
    return {
        "processors": [
            {
                "model": "cortex-r5f",
                "registers": [["Pc", 0x10000]],
                "released": True,
                "mprofile": None,
                "rprofile": {
                    "cp14_identity": 21930,
                    "tcm": {
                        "btcm": {"bytes": 4096, "reset": 1879048193},
                        "atcm": {"bytes": 8192, "reset": 4294934529},
                    },
                },
            }
        ],
        "ram": [{"base": 0x10000, "size": 0x4000}],
        "dma_mappings": [],
        "decoders": [],
        "mappings": [
            {
                "core": 0,
                "address": 0x10000,
                "physical": 0x10000,
                "size": 0x4000,
                "permissions": 7,
            }
        ],
        "peripherals": [],
        "instructions_per_tick": [1, 1],
    }


def _device_plan() -> dict[str, object]:
    return {
        "schema": 1,
        "boot_entry": "loader",
        "wires": {},
        "nodes": [
            {
                "name": "idle",
                "boot_entries": ["rom", "loader"],
                "windows": [{"base": 0xFFF86000, "size": 12}],
                "dependencies": [],
                "model": {"kind": "block86000", "options": {}},
            }
        ],
    }


def test_board_runs_authored_program_and_releases_native_state() -> None:
    library = os.environ.get("CAMERA_UNICORN_LIBRARY")
    if library is None:
        pytest.fail("Set CAMERA_UNICORN_LIBRARY to select an execution library")

    program = struct.pack("<II", 0xE3A0002A, 0xEAFFFFFE)
    asset = Asset("program", len(program), hashlib.sha256(program).hexdigest())
    definition = Definition(
        execution=_execution_graph(),
        devices=_device_plan(),
        assets=(asset,),
        images=(Image(asset.id, 0x10000, core=0),),
        library=library,
        engine="unicorn",
        quantum=1,
    )
    board = make_board(definition, {asset.id: program})
    reference = weakref.ref(board)
    try:
        board.run(1)
        assert board.instructions == 1
        assert board.cpus[0].reg_read({"Arm32": {"R": 0}}) == 42
    finally:
        board.close()
    assert board.closed
    board.close()
    with pytest.raises(RuntimeError, match="runtime is closed"):
        board.state()
    del board
    gc.collect()
    assert reference() is None


def test_asset_and_image_validate_inputs_before_board_construction() -> None:
    digest = "0" * 64
    assert Asset("program", 0, digest).manifest() == {
        "id": "program",
        "content": {"size": 0, "sha256": digest},
    }
    with pytest.raises(ValueError, match="Asset id"):
        Asset("../program", 1, digest)
    with pytest.raises(ValueError, match="Asset size"):
        Asset("program", True, digest)
    with pytest.raises(ValueError, match="SHA-256"):
        Asset("program", 1, "A" * 64)
    with pytest.raises(ValueError, match="Image address"):
        Image("program", -1)
    with pytest.raises(ValueError, match="Image core"):
        Image("program", 0, core=-1)


def test_board_rejects_invalid_execution_limits_before_native_calls() -> None:
    definition = Definition(
        execution=_execution_graph(),
        devices=_device_plan(),
        assets=(),
        quantum=0,
    )
    with pytest.raises(ValueError, match="quantum"):
        make_board(definition, {})


def test_board_from_inputs_validates_identifiers_and_paths_first() -> None:
    with pytest.raises(ValueError, match="body"):
        Board.from_inputs(
            body="../body",
            firmware="firmware",
            update="update.bin",
            boot_rom="boot.rom",
            unicorn="libunicorn.dylib",
        )
    with pytest.raises(TypeError, match="boot_rom"):
        Board.from_inputs(
            body="body",
            firmware="firmware",
            update="update.bin",
            boot_rom=cast(str, object()),
            unicorn="libunicorn.dylib",
        )


def test_boot_report_decodes_the_stopped_guest_observation() -> None:
    report = _boot_report({
        "status": "rec",
        "instructions": 120,
        "virtual_ticks": 8,
        "ui_mode": 1,
        "system_mode": 3,
        "menu_active": 0,
        "pending_mode": 0,
        "menu_node": 7,
        "pending": False,
        "setup_presses": 2,
        "failure": None,
    })
    assert report["status"] == "rec"
    assert report["instructions"] == 120
    assert report["menu_node"] == 7
    assert report["pending"] is False
    with pytest.raises(RuntimeError, match="unknown status"):
        _boot_report({"status": "ready"})


def test_session_facade_validates_inputs_and_returns_typed_observations() -> None:
    class Native:
        def adc_samples(self, node: str) -> list[int]:
            assert node == "adc0"
            return [0, 1023]

        def set_adc_samples(self, node: str, samples: list[int]) -> None:
            assert node == "adc0"
            assert samples == [17, 31]

        def gpio_input(self, name: str, port: int, pin: int, high: bool) -> None:
            assert (name, port, pin, high) == ("gpio0", 2, 7, True)

        def input_level(self, name: str, high: bool) -> None:
            assert (name, high) == ("key", False)

        def rotate_control(self, control_id: str, steps: int) -> None:
            assert (control_id, steps) == ("dial", -2)

        def scanout(
            self, node: str, instance: int, pixels: bool, diagnostic_hashes: bool
        ) -> tuple[bytes, bytes | None, str, int, int]:
            assert (node, instance, pixels, diagnostic_hashes) == (
                "display0",
                1,
                True,
                False,
            )
            return b"raw", b"rgb", '{"active":true}', 2, 1

        def scanout_main(
            self, node: str, instance: int, pixels: bool, diagnostic_hashes: bool
        ) -> tuple[bytes, bytes | None, str, int, int]:
            raise AssertionError("main scanout path was not requested")

        def screenshot(self) -> bytes:
            return b"png"

        def close(self) -> None:
            pass

    board = Board.__new__(Board)
    board._native = cast(native.Board, Native())
    board._closed = False
    board._boot_capable = False
    board.quantum = 1
    board.cpus = ()
    session = Session(board)

    assert session.adc_samples("adc0") == (0, 1023)
    session.set_adc_samples("adc0", [17, 31])
    session.gpio_input("gpio0", 2, 7, True)
    session.input_level("key", False)
    session.rotate("dial", -2)
    frame = session.scanout("display0", 1)
    assert frame is not None
    assert (frame.raw, frame.rgb, frame.width, frame.height) == (
        b"raw",
        b"rgb",
        2,
        1,
    )
    assert frame.state == {"active": True}
    assert session.screenshot() == b"png"

    with pytest.raises(ValueError, match="unsigned 16-bit"):
        session.set_adc_samples("adc0", [65536])
    with pytest.raises(ValueError, match="unsigned 8-bit"):
        session.gpio_input("gpio0", 256, 0, True)
    with pytest.raises(TypeError, match="bool"):
        session.input_level("key", cast(bool, 1))
    with pytest.raises(ValueError, match="signed 32-bit"):
        session.rotate("dial", 1 << 31)

    board.close()
    with pytest.raises(RuntimeError, match="runtime is closed"):
        session.adc_samples("adc0")
