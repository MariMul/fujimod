"""One Pyodide value adapter for Rust-generated schemas.

The schema describes types, never camera commands or validation bounds.
Constructors and byte ownership still go through the Rust/Wasm implementation.
"""

import json
from inspect import Parameter, Signature
from enum import Enum
from ._bridge import Handle, buffer_in, buffer_out, call, sint, uint, unwrap
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
        name = quantity["name"]
        constructor = getattr(js, name).new
        initialize = _quantity_constructor(constructor, quantity["parameters"])
        if quantity["getter"] == "data":
            getter = property(lambda self: buffer_out(self._js.data))
            property_name = "data"
        else:
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


def _quantity_constructor(constructor, parameters):
    signature = Signature(
        [Parameter(p["name"], Parameter.POSITIONAL_OR_KEYWORD) for p in parameters]
    )

    def initialize(self, *args, **kwargs):
        arguments = signature.bind(*args, **kwargs).arguments
        converted = []
        for parameter in parameters:
            value = arguments[parameter["name"]]
            input_type = parameter["input"]
            if input_type == "bytes":
                converted.append(buffer_in(value))
            else:
                bits = 32 if input_type == "Address" else int(input_type[1:])
                converted.append(
                    (sint if input_type.startswith("i") else uint)(value, bits)
                )
        self._js = call(constructor, *converted)

    return initialize


def _python_type(ty, values):
    kind = ty["kind"]
    if kind == "integer":
        return int
    if kind == "named":
        return values[ty["name"]]
    if kind == "bytes" or (ty["bits"] == 8 and not ty["signed"]):
        return bytes
    return tuple


def _factory(factory, fields, values, js):
    def create(*arguments):
        converted = []
        for field, value in zip(fields, arguments):
            ty = field["ty"]
            if ty["kind"] == "integer":
                converted.append((sint if ty["signed"] else uint)(value, ty["bits"]))
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
            elif ty["kind"] == "bytes" or (
                ty["kind"] == "array" and ty["bits"] == 8 and not ty["signed"]
            ):
                item = buffer_out(item)
            elif ty["kind"] == "array":
                item = tuple(item)
            converted.append(item)
        return constructor(*converted)

    return decode
