import json
from typing import cast

import pytest
from fujimod.emulator import catalog


def test_reference_resolution_returns_a_typed_selection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    expected = {
        "soc": {"id": "exr2"},
        "body": {"id": "x70"},
        "firmware": {"id": "x70-110"},
        "parts": [],
    }
    calls = []

    def resolve(body_id: str, firmware_id: str) -> str:
        calls.append((body_id, firmware_id))
        return json.dumps(expected)

    monkeypatch.setattr(
        catalog._native_catalog, "resolve_reference", resolve, raising=False
    )

    result: catalog.ResolvedSelection = catalog.resolve_reference("x70", "x70-110")

    assert result == expected
    assert calls == [("x70", "x70-110")]


def test_firmware_resolution_uses_only_the_firmware_id(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    expected = {
        "soc": {"id": "exr2"},
        "body": {"id": "x70"},
        "firmware": {"id": "x70-110"},
        "parts": [],
    }
    calls = []

    def resolve(firmware_id: str) -> str:
        calls.append(firmware_id)
        return json.dumps(expected)

    monkeypatch.setattr(
        catalog._native_catalog, "resolve_firmware", resolve, raising=False
    )

    assert catalog.resolve_firmware("x70-110") == expected
    assert calls == ["x70-110"]


def test_selection_requires_native_json_object(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        catalog._native_catalog,
        "resolve_reference",
        lambda body_id, firmware_id: "[]",
        raising=False,
    )

    with pytest.raises(RuntimeError, match="must return a JSON object"):
        catalog.resolve_reference("x70", "x70-110")


def test_selection_requires_native_json_text(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        catalog._native_catalog,
        "resolve_firmware",
        lambda firmware_id: {"id": firmware_id},
        raising=False,
    )

    with pytest.raises(RuntimeError, match="must return JSON text"):
        catalog.resolve_firmware("x70-110")


@pytest.mark.parametrize(
    ("body_id", "firmware_id"),
    [(1, "x70-110"), ("x70", None)],
)
def test_reference_resolution_rejects_non_string_ids(
    body_id: object, firmware_id: object
) -> None:
    with pytest.raises(TypeError, match="must be strings"):
        catalog.resolve_reference(cast(str, body_id), cast(str, firmware_id))
