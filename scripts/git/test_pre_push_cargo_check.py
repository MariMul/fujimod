from pathlib import Path

from pre_push_cargo_check import is_rust_input, package_supports_wasm, select_touched_crates


def test_touched_rust_paths_select_only_their_workspace_crates(tmp_path: Path) -> None:
    packages = [
        {"name": "fujimod-core", "manifest_path": str(tmp_path / "crates/core/Cargo.toml")},
        {"name": "fujimod-usb", "manifest_path": str(tmp_path / "crates/usb/Cargo.toml")},
        {"name": "fujimod-web", "manifest_path": str(tmp_path / "bindings/web/Cargo.toml")},
    ]

    selected = select_touched_crates(
        [
            "crates/core/src/lib.rs",
            "crates/usb/Cargo.toml",
            "crates/web/README.md",
            "README.md",
        ],
        packages,
        tmp_path,
    )

    assert [package["name"] for package in selected] == ["fujimod-core", "fujimod-usb"]
    assert is_rust_input("crates/core/src/lib.rs")
    assert not is_rust_input("README.md")


def test_workspace_rust_configuration_selects_every_crate(tmp_path: Path) -> None:
    packages = [
        {"name": "fujimod-core", "manifest_path": str(tmp_path / "crates/core/Cargo.toml")},
        {"name": "fujimod-usb", "manifest_path": str(tmp_path / "crates/usb/Cargo.toml")},
    ]

    selected = select_touched_crates(["Cargo.toml"], packages, tmp_path)

    assert [package["name"] for package in selected] == ["fujimod-core", "fujimod-usb"]


def test_wasm_check_is_selected_for_a_wasm_capable_crate(tmp_path: Path) -> None:
    package_root = tmp_path / "crates/web"
    package_root.mkdir(parents=True)
    manifest = package_root / "Cargo.toml"
    manifest.write_text(
        '[target.\'cfg(target_arch = "wasm32")\'.dependencies]\nweb-sys = "1"\n',
        encoding="utf-8",
    )
    package = {"name": "fujimod-web", "manifest_path": str(manifest)}

    assert package_supports_wasm(package)
