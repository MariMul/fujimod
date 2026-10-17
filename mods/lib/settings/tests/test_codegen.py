"""Exercise the actual generated C under ASan/UBSan, without a camera.

Known upstream failures are measured explicitly and reported as adoption
blockers, not treated as acceptable settings semantics. See qualification().
"""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import unittest


COMPONENT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('settings_codegen', COMPONENT / 'generate.py')
codegen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(codegen)
BUILD = COMPONENT / 'build/tests'


def compile_driver():
    cc = os.environ.get('CC') or shutil.which('cc')
    if not cc:
        raise RuntimeError('A host C compiler is required')
    codegen.generate(COMPONENT / 'tests/section.schema.json', BUILD)
    executable = BUILD / 'section-driver'
    subprocess.run([cc, '-std=c11', '-Wall', '-Wextra', '-Werror',
                    '-Wno-unused-function', '-fsanitize=address,undefined',
                    '-I', str(BUILD), str(BUILD / 'section.c'),
                    str(COMPONENT / 'tests/driver.c'), '-o', str(executable)], check=True)
    return executable


def run(executable, document):
    if not isinstance(document, str):
        document = json.dumps(document)
    result = subprocess.run([str(executable)], input=document, text=True,
                            capture_output=True, check=True)
    return result.stdout.rstrip('\n').split('\t')


def qualification(executable):
    checks = {}
    checks['decoded_string_escapes'] = run(executable, {'author': 'A\nB'})[5] == '410a42'
    for name, document in (
            ('reject_trailing_comma', '{"enabled":true,}'),
            ('reject_missing_comma', '{"enabled":true "hold_ms":400}'),
            ('reject_invalid_boolean', '{"enabled":truth}'),
            ('reject_leading_zero', '{"hold_ms":0400}'),
            ('reject_trailing_document', '{} {}')):
        checks[name] = run(executable, document)[0] == 'error'
    return dict(ready=all(checks.values()), checks=checks,
                blockers=[name for name, passed in checks.items() if not passed])


class GeneratedSettingsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.executable = compile_driver()

    def test_defaults_enums_nested_objects_and_empty_arrays(self):
        self.assertEqual(run(self.executable, {}), ['ok', '1', '400', '0', '300', '', '0'])
        self.assertEqual(run(self.executable, dict(enabled=False, hold_ms=150,
            theme='night', controls=dict(repeat_ms=0), points=[0, 255], author='Name')),
            ['ok', '0', '150', '2', '0', '4e616d65', '2', '0', '255'])

    def test_real_strings_own_their_storage_and_accept_the_bound(self):
        self.assertEqual(bytes.fromhex(run(self.executable, {'author': 'a'*32})[5]), b'a'*32)

    def test_invalid_sections_do_not_replace_live_state(self):
        for document in ({'author': 'a'*33}, {'hold_ms': 99}, {'hold_ms': 2001},
                         {'theme': 'unknown'}, {'points': [1, 2, 3, 4, 5]},
                         {'controls': {'repeat_ms': -1}}, {'unknown': 1},
                         '{"hold_ms":100,"hold_ms":200}'):
            with self.subTest(document=document):
                self.assertEqual(run(self.executable, document), ['error', '999'])

    def test_qualification_report(self):
        report = qualification(self.executable)
        (BUILD / 'qualification.json').write_text(json.dumps(report, indent=2)+'\n')
        self.assertEqual(report['ready'], not report['blockers'])

    # Desired settings semantics, currently broken upstream. Expected failures
    # keep defects visible; an unexpected success requires requalification.
    @unittest.expectedFailure
    def test_decode_string_escapes(self):
        self.assertEqual(run(self.executable, {'author': 'A\nB'})[5], '410a42')

    @unittest.expectedFailure
    def test_reject_trailing_comma(self):
        self.assertEqual(run(self.executable, '{"enabled":true,}')[0], 'error')

    @unittest.expectedFailure
    def test_reject_invalid_boolean(self):
        self.assertEqual(run(self.executable, '{"enabled":truth}')[0], 'error')

    @unittest.expectedFailure
    def test_reject_leading_zero(self):
        self.assertEqual(run(self.executable, '{"hold_ms":0400}')[0], 'error')

    @unittest.expectedFailure
    def test_reject_trailing_document(self):
        self.assertEqual(run(self.executable, '{} {}')[0], 'error')

    def test_generation_is_reproducible(self):
        first = (BUILD / 'section.c').read_bytes(), (BUILD / 'section.h').read_bytes()
        codegen.generate(COMPONENT / 'tests/section.schema.json', BUILD)
        self.assertEqual(first, ((BUILD / 'section.c').read_bytes(), (BUILD / 'section.h').read_bytes()))


if __name__ == '__main__':
    unittest.main()
