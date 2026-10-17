"""FujiMod host tools: native Rust on desktop, WebUSB through Rust in Pyodide."""

import sys

if sys.platform == "emscripten":
    from . import usb
    __all__ = ["usb"]
else:
    from ._native import defs, usb
    __all__ = ["defs", "usb"]
