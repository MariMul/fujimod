"""FujiMod host tools: native Rust on desktop, WebUSB through Rust in Pyodide."""

import sys
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import catalog, corpus, emulator, firmware, usb
elif sys.platform == "emscripten":
    from . import catalog, usb
else:
    from . import corpus, emulator
    from ._native import catalog, firmware, usb

__all__ = ["catalog", "usb"]
if sys.platform != "emscripten":
    __all__ += ["firmware", "corpus", "emulator"]
