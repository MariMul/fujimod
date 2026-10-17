"""Pyodide conversion for Rust-owned service commands and typed replies."""

from dataclasses import dataclass
from types import SimpleNamespace
from _fujimod_js import valueSchemas as _schemas
from .._binding_values import bind_values as _bind_values

from .._bridge import Handle, buffer_in, buffer_out, call, jsusb, snapshot, unwrap
from .._variants import Sum, enum_out, variant, unwrap_variant
from ..defs import service as definitions
from . import Session
from .app80 import CommandCode

_js = jsusb.app80
_value_types, _result_decoders = _bind_values(
    _schemas.service, _js, "fujimod.usb.app80"
)
_v = SimpleNamespace(**_value_types)


class CommandError(ValueError):
    pass


class CommandFailure(Sum):
    pass


variant(CommandFailure, "Unsupported", [("codec", definitions.Codec)])
variant(CommandFailure, "Ambiguous", [("codec", definitions.Codec)])
variant(CommandFailure, "Unavailable", [("code", int)])
variant(CommandFailure, "Argument", [("kind", _v.ArgumentKind)])
variant(CommandFailure, "OversizeArguments")


def _command_failure(value):
    from _fujimod_js import defs

    kinds = _js.CommandFailureKind
    return {
        kinds.Unsupported: lambda: CommandFailure.Unsupported(
            enum_out(value.codec, definitions.Codec, defs.service.Codec)
        ),
        kinds.Ambiguous: lambda: CommandFailure.Ambiguous(
            enum_out(value.codec, definitions.Codec, defs.service.Codec)
        ),
        kinds.Unavailable: lambda: CommandFailure.Unavailable(value.code),
        kinds.Argument: lambda: CommandFailure.Argument(
            enum_out(value.argument, _v.ArgumentKind, _js.ArgumentKind)
        ),
        kinds.OversizeArguments: CommandFailure.OversizeArguments,
    }[value.kind]()


@dataclass(frozen=True)
class CommandReply:
    value: _v.ReplyValue
    raw: bytes

    @classmethod
    def _from_js(cls, value):
        return snapshot(
            value,
            lambda reply: cls(
                _result_decoders[_v.ReplyValue](reply.value), buffer_out(reply.raw)
            ),
        )


class Invocation(Handle):
    def __init__(self):
        raise TypeError("invocations come from Service.prepare")

    raw = property(lambda self: buffer_out(self._js.raw))
    configuration_limit = property(lambda self: self._js.configurationLimit)

    def decode(self, raw):
        return CommandReply._from_js(call(self._js.decode, buffer_in(raw)))


class Service(Handle):
    def __init__(self, definition):
        self._js = call(_js.Service.new, unwrap(definition, definitions.Definition))

    def prepare(self, modes, command):
        return Invocation._wrap(
            call(
                self._js.prepare,
                unwrap(modes, definitions.ServiceModes),
                unwrap_variant(command, _v.Command),
            )
        )

    def prepare_research(self, code, modes, arguments):
        return Invocation._wrap(
            call(
                self._js.prepareResearch,
                unwrap(code, CommandCode),
                unwrap(modes, definitions.ServiceModes),
                buffer_in(arguments),
            )
        )


async def execute(session, invocation):
    return await session._call(
        _js.execute,
        unwrap(session, Session),
        unwrap(invocation, Invocation),
        decode=CommandReply._from_js,
    )
