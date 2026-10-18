"""FujiMod host tools: native Rust on desktop, WebUSB through Rust in Pyodide."""

import sys
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import corpus, defs, firmware, usb
elif sys.platform == "emscripten":
    from . import defs, usb
else:
    from . import corpus
    from ._native import defs, firmware, usb

__all__ = ["defs", "usb"]
if sys.platform != "emscripten":
    __all__ += ["firmware", "corpus"]
