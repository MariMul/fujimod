"""Typed identity-based access to the bundled hardware reference catalog."""

from __future__ import annotations

import json
from typing import TypeGuard, cast

from fujimod._native import catalog as _native_catalog

from ._catalog_types import (
    Body,
    Firmware,
    Part,
    ResolvedSelection,
    RunConfiguration,
    RunOverlay,
    Soc,
)


def _string_keyed_object(value: object) -> TypeGuard[dict[str, object]]:
    return isinstance(value, dict) and all(isinstance(key, str) for key in value)


def _selection(result: object) -> ResolvedSelection:
    if type(result) is not str:
        raise RuntimeError("Native catalog resolution must return JSON text")
    value = json.loads(result)
    if not _string_keyed_object(value):
        raise RuntimeError("Native catalog resolution must return a JSON object")
    return cast(ResolvedSelection, value)


def resolve_reference(body_id: str, firmware_id: str) -> ResolvedSelection:
    """Resolve a checked body and firmware selection by their public IDs."""
    if type(body_id) is not str or type(firmware_id) is not str:
        raise TypeError("body_id and firmware_id must be strings")
    return _selection(_native_catalog.resolve_reference(body_id, firmware_id))


def resolve_firmware(firmware_id: str) -> ResolvedSelection:
    """Resolve a firmware ID and its referenced body, SoC and parts."""
    if type(firmware_id) is not str:
        raise TypeError("firmware_id must be a string")
    return _selection(_native_catalog.resolve_firmware(firmware_id))


__all__ = [
    "Body",
    "Firmware",
    "Part",
    "ResolvedSelection",
    "RunConfiguration",
    "RunOverlay",
    "Soc",
    "resolve_firmware",
    "resolve_reference",
]
