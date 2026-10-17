"""Shared firmware data loaded by Rust through the JavaScript module in Pyodide."""

from _fujimod_js import defs as _jsdefs

from .._bridge import Handle, call
from .._variants import Sum, variant


class DefinitionError(ValueError):
    pass


class DefinitionFailure(Sum):
    pass


variant(DefinitionFailure, "Json", [("line", int), ("column", int)])
variant(DefinitionFailure, "Schema", [("schema", int)])
for _name in (
    "EmptyIdentity",
    "InvalidDigest",
    "FirmwareMismatch",
    "EmptyModes",
    "InvalidMailbox",
    "SourceCount",
    "EmptyLabel",
):
    variant(DefinitionFailure, _name)
# Codec is supplied by service after both namespaces have been initialized.
variant(DefinitionFailure, "InvalidLayout", [("code", int), ("codec", object)])


def _failure(value):
    from .._variants import enum_out
    from .service import Codec

    kinds = _jsdefs.DefinitionFailureKind
    convert = {
        kinds.Json: lambda: DefinitionFailure.Json(value.line, value.column),
        kinds.Schema: lambda: DefinitionFailure.Schema(value.schema),
        kinds.EmptyIdentity: DefinitionFailure.EmptyIdentity,
        kinds.InvalidDigest: DefinitionFailure.InvalidDigest,
        kinds.FirmwareMismatch: DefinitionFailure.FirmwareMismatch,
        kinds.EmptyModes: DefinitionFailure.EmptyModes,
        kinds.InvalidMailbox: DefinitionFailure.InvalidMailbox,
        kinds.InvalidLayout: lambda: DefinitionFailure.InvalidLayout(
            value.code, enum_out(value.codec, Codec, _jsdefs.service.Codec)
        ),
        kinds.SourceCount: DefinitionFailure.SourceCount,
        kinds.EmptyLabel: DefinitionFailure.EmptyLabel,
    }
    return convert[value.kind]()


class FirmwareIdentity(Handle):
    def __init__(self, id, sha256):
        if type(id) is not str or type(sha256) is not str:
            raise TypeError("firmware identity requires strings")
        self._js = call(_jsdefs.FirmwareIdentity.new, id, sha256)

    id = property(lambda self: self._js.id)
    sha256 = property(lambda self: self._js.sha256)


# Child definitions use FirmwareIdentity while this package initializes.
from . import service  # noqa: E402

__all__ = ["FirmwareIdentity", "DefinitionError", "DefinitionFailure", "service"]
