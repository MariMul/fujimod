"""One Pyodide value adapter for Rust-generated schemas.

The schema describes types, never camera commands or validation bounds.
Constructors and byte ownership still go through the Rust/Wasm implementation.
"""

import json
from enum import Enum
from ._bridge import Handle, buffer_in, buffer_out, call, uint, unwrap
from ._variants import Sum, variant, unwrap_variant


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
    # Allocate every payload class first, so nested types resolve independently
    # of declaration order. Converters run only after registration is complete.
    for union in description["unions"]:
        bases = (Sum, Handle) if union["input"] else (Sum,)
        values[union["name"]] = type(union["name"], bases, {"__module__": module})
    for union in description["unions"]:
        cls = values[union["name"]]
        constructors = {}
        for member in union["variants"]:
            fields = member["fields"]
            options = {}
            if union["input"]:
                factory = getattr(getattr(js, union["name"]), member["factory"])
                options["create"] = _factory(factory, fields, values, js)
            constructor = variant(
                cls,
                member["name"],
                [
                    (field["name"], _python_type(field["ty"], values))
                    for field in fields
                ],
                **options,
            )
            if not union["input"]:
                kinds = getattr(js, union["kind_export"])
                constructors[getattr(kinds, member["name"])] = (constructor, fields)
        if not union["input"]:
            decoders[cls] = _decoder(constructors, values, decoders, js)
    return values, decoders


def _bytes_constructor(constructor):
    def initialize(self, data):
        self._js = call(constructor, buffer_in(data))

    return initialize


def _number_constructor(constructor, bits):
    def initialize(self, value):
        self._js = call(constructor, uint(value, bits))

    return initialize


def _python_type(ty, values):
    kind = ty["kind"]
    if kind == "integer":
        return int
    if kind == "named":
        return values[ty["name"]]
    if kind == "bytes" or ty["bits"] == 8:
        return bytes
    return tuple


def _factory(factory, fields, values, js):
    def create(*arguments):
        converted = []
        for field, value in zip(fields, arguments):
            ty = field["ty"]
            if ty["kind"] == "integer":
                converted.append(uint(value, ty["bits"]))
                continue
            cls = values[ty["name"]]
            if issubclass(cls, Enum):
                if type(value) is not cls:
                    raise TypeError(f"expected {cls.__name__}")
                converted.append(getattr(getattr(js, ty["name"]), value.name))
            elif issubclass(cls, Sum):
                converted.append(unwrap_variant(value, cls))
            else:
                converted.append(unwrap(value, cls))
        return call(factory, *converted)

    return create


def _decoder(constructors, values, decoders, js):
    def decode(value):
        constructor, fields = constructors[value.kind]
        converted = []
        for field in fields:
            name = field["name"].split("_")
            item = getattr(value, name[0] + "".join(part.title() for part in name[1:]))
            ty = field["ty"]
            if ty["kind"] == "named":
                cls = values[ty["name"]]
                if issubclass(cls, Enum):
                    choices = getattr(js, ty["name"])
                    item = {getattr(choices, m.name): m for m in cls}[item]
                elif issubclass(cls, Sum):
                    item = decoders[cls](item)
                else:
                    item = cls._wrap(item)
            elif ty["kind"] == "bytes" or (ty["kind"] == "array" and ty["bits"] == 8):
                item = buffer_out(item)
            elif ty["kind"] == "array":
                item = tuple(item)
            converted.append(item)
        return constructor(*converted)

    return decode
