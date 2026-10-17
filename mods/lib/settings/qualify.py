#!/usr/bin/env python3
"""Compile and qualify the pinned generator; exit nonzero if adoption is blocked."""
import json
import sys

from tests.test_generated import BUILD, compile_driver, qualification


def main():
    report = qualification(compile_driver())
    (BUILD / 'qualification.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report, indent=2))
    return 0 if report['ready'] else 1


if __name__ == '__main__':
    sys.exit(main())
