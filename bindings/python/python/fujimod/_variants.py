"""Python representations of payload enums returned by the Rust bindings."""

from dataclasses import field, make_dataclass


class Sum:
    def __new__(cls, *args, **kwargs):
        if not cls.__dict__.get("_is_variant", False):
            raise TypeError(f"construct a named {cls.__name__} variant")
        return object.__new__(cls)


def variant(base, name, fields=(), *, create=None):
    """A frozen variant, optionally owning a Rust enum through a JS handle.

    The factory converts language values only; it never encodes protocol bytes.
    """
    namespace = {
        "__module__": base.__module__,
        "__qualname__": f"{base.__qualname__}.{name}",
        "_is_variant": True,
    }
    fields = list(fields)
    if create is not None:
        names = [name for name, _ in fields]

        def initialize(self):
            object.__setattr__(
                self, "_js", create(*(getattr(self, name) for name in names))
            )

        namespace["__post_init__"] = initialize
        fields.append(("_js", object, field(init=False, repr=False)))
    result = make_dataclass(
        name, fields, bases=(base,), namespace=namespace, frozen=True, eq=False
    )
    setattr(base, name, result)
    return result


def enum_out(value, enum, choices):
    return {getattr(choices, member.name): member for member in enum}[value]


def unwrap_variant(value, cls):
    if not isinstance(value, cls):
        raise TypeError(f"expected {cls.__name__} variant")
    return value._js
