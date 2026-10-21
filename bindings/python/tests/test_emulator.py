"""Native emulator package and board execution contracts."""

import gc
import hashlib
import importlib
import os
import platform
import struct
import weakref

import pytest
from fujimod import catalog, emulator, usb
from fujimod._native import emulator as native
from fujimod.emulator import Asset, Definition, Image, make_board

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
                "model": "armv8a",
                "registers": [["Pc", 0x10000]],
                "released": True,
                "mprofile": None,
                "rprofile": None,
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
    unicorn_library = os.environ.get("FUJIMOD_UNICORN_LIBRARY")
    if unicorn_library:
        engine = "unicorn"
        library = unicorn_library
    elif platform.system() == "Darwin" and platform.machine() in {"arm64", "aarch64"}:
        engine = "hypervisor"
        library = None
    else:
        pytest.skip("Set FUJIMOD_UNICORN_LIBRARY to select an execution library")

    program = struct.pack("<II", 0xD2800540, 0x14000000)
    asset = Asset("program", len(program), hashlib.sha256(program).hexdigest())
    definition = Definition(
        execution=_execution_graph(),
        devices=_device_plan(),
        assets=(asset,),
        images=(Image(asset.id, 0x10000),),
        library=library,
        engine=engine,
        quantum=1,
    )
    board = make_board(definition, {asset.id: program})
    reference = weakref.ref(board)
    try:
        board.run(1)
        assert board.instructions == 1
        assert board.cpus[0].reg_read({"X": 0}) == 42
    finally:
        board.close()
    with pytest.raises(RuntimeError, match="runtime is closed"):
        board.state()
    del board
    gc.collect()
    assert reference() is None
