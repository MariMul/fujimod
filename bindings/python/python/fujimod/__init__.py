"""FujiMod host tools: native Rust on desktop, WebUSB through Rust in Pyodide."""

import sys

if sys.platform == "emscripten":
    from . import defs, usb
else:
    from ._native import defs, usb, firmware
    from . import corpus

__all__ = ["defs", "usb"]
if sys.platform != "emscripten":
    __all__.extend(["firmware", "corpus"])
