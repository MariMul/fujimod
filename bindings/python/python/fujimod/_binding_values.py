"""One Pyodide value adapter for Rust-generated schemas.

The schema describes types, never camera commands or validation bounds.
Constructors and byte ownership still go through the Rust/Wasm implementation.
"""

import json
from enum import Enum
from ._bridge import Handle, buffer_in, buffer_out, call, uint
from ._variants import Sum, variant


def bind_values(schema, js, module):
    description = json.loads(str(schema))
    values, decoders = {}, {}
    for choice in description["choices"]:
        name, members = choice["name"], choice["variants"]
        if not choice["result"]:
            values[name] = Enum(name, members, module=module)
            continue
        cls = type(name, (Sum,), {"__module__": module})
        for member in members:
            variant(cls, member)
        variant(cls, "Unknown", [("raw_status", int)])
        kinds = getattr(js, choice["kind_name"])
        constructors = {
            getattr(kinds, member): getattr(cls, member) for member in members
        }

        def decode(value, cls=cls, kinds=kinds, constructors=constructors):
            if value.kind == kinds.Unknown:
                return cls.Unknown(value.rawStatus)
            return constructors[value.kind]()

        values[name] = cls
        decoders[cls] = decode
    for quantity in description["quantities"]:
        name, input_type = quantity["name"], quantity["input"]
        constructor = getattr(js, name).new
        if input_type == "bytes":
            initialize = _bytes_constructor(constructor)
            getter = property(lambda self: buffer_out(self._js.data))
            property_name = "data"
        else:
            bits = {"u8": 8, "u16": 16, "u32": 32, "Address": 32}[input_type]
            initialize = _number_constructor(constructor, bits)
            getter = property(lambda self: self._js.value)
            property_name = "value"
        values[name] = type(
            name,
            (Handle,),
            {"__module__": module, "__init__": initialize, property_name: getter},
        )
    return values, decoders


def _bytes_constructor(constructor):
    def initialize(self, data):
        self._js = call(constructor, buffer_in(data))

    return initialize


def _number_constructor(constructor, bits):
    def initialize(self, value):
        self._js = call(constructor, uint(value, bits))

    return initialize
