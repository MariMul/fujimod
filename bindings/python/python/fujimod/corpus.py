"""Function-family index. Inputs are boundaries and mapped bytes from any source.

Function ids and family ids are local to one build; persist function keys in locators.
All addresses are integers and ranges are half-open. No firmware policy lives here.
"""
import json
from ._native import corpus as _core

CorpusIndex = _core.CorpusIndex


class Index:
    def __init__(self):
        self._native = CorpusIndex()

    @classmethod
    def open(cls, path):
        obj = cls.__new__(cls)
        obj._native = CorpusIndex.open(str(path))
        return obj

    def save(self, path):
        self._native.save(str(path))

    def add_unit(self, metadata, regions):
        self._native.add_unit(json.dumps(metadata), regions)

    def build(self):
        self._native.build()

    def merge(self, other):
        self._native.merge(other._native)

    def subset(self, firmwares):
        obj = self.__class__.__new__(self.__class__)
        obj._native = self._native.subset(list(firmwares))
        return obj

    def _query(self, operation, **arguments):
        return json.loads(self._native.query(operation, json.dumps(arguments)))

    def stats(self):
        return self._query('stats')

    def find(self, name=None, *, address=None, firmware=None, unit=None):
        return self._query('find', name=name, address=address, firmware=firmware, unit=unit)

    def family(self, family):
        return self._query('family', id=family)

    def members(self, family, firmware=None):
        return self._query('members', id=family, firmware=firmware)

    def normalized_tokens(self, function):
        return self._query('normalized_tokens', id=function)

    def calls(self, function):
        return self._query('calls', id=function)

    def data_references(self, function):
        return self._query('data_references', id=function)

    def references_to(self, firmware, address, unit=None):
        return self._query('references_to', firmware=firmware, address=address, unit=unit)

    def search(self, firmware, pattern, *, unit=None, mask=None):
        return self._query('search', firmware=firmware, unit=unit,
                           pattern=list(pattern), mask=list(mask) if mask is not None else None)

    def pairs(self, reference, other):
        return self._query('pairs', reference=reference, other=other)

    def names(self, function, donors=None, *, normalized_only=False):
        return self._query('names', id=function, donors=donors, normalized_only=normalized_only)

    def annotate(self, family, kind, text, source):
        self._native.annotate(family, json.dumps(dict(kind=kind, text=text, source=source)))
