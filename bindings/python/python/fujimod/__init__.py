"""FujiMod host tools: native Rust on desktop, WebUSB through Rust in Pyodide."""

import sys

if sys.platform == "emscripten":
    from . import usb
else:
    from ._native import usb

__all__ = ["usb"]
