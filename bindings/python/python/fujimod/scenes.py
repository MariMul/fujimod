"""Shared scene assets for native runs and the browser build."""
from hashlib import sha256
from pathlib import Path
import subprocess

_DEFAULT_RAF_SHA256 = '2fdbda765e9abf147d4bdfdd5a2060d2c8702d7ad53561a5bde664bc50d53cf4'


def default_scene_raf() -> Path:
    """Return the verified original RAF, materializing it when missing or stale."""
    root = Path(__file__).resolve().parents[4]
    asset = root / 'build/assets/default-scene.raf'
    if not asset.is_file() or sha256(asset.read_bytes()).hexdigest() != _DEFAULT_RAF_SHA256:
        subprocess.run(['node', str(root / 'scripts/assets/default-scene.ts')],
                       cwd=root, check=True, capture_output=True, text=True)
    if sha256(asset.read_bytes()).hexdigest() != _DEFAULT_RAF_SHA256:
        raise ValueError('Default RAF does not match the recorded original SHA-256')
    return asset
