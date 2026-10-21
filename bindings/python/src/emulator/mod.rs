//! Native bindings for the shared execution runtime and host algorithms.
#[cfg(not(target_os = "emscripten"))]
mod board;
mod discovery;
mod display_pixels;
mod film;
mod jpeg;
mod method4;
mod mono_raster;
mod python_graphics;
mod python_observation;
mod python_values;
mod raster;
mod scanout_observation;
mod scene;
mod xp4;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyModule};

pyo3::create_exception!(fujimod._native.emulator, NativeError, PyRuntimeError);

pub(crate) fn error(message: String) -> PyErr {
    NativeError::new_err(message)
}

/// Load explicitly located, identity-checked inputs before constructing a board.
#[pyfunction]
fn load_input_assets<'py>(
    py: Python<'py>,
    root: std::path::PathBuf,
    assets: &str,
    locations: &str,
    max_asset_bytes: u64,
) -> PyResult<Bound<'py, PyDict>> {
    let assets: Vec<fujimod_catalog::emulator::Asset> = serde_json::from_str(assets)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let locations = serde_json::from_str(locations)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let provider = fujimod_emulator::inputs::Files::new(
        &root,
        &root,
        Default::default(),
        locations,
        max_asset_bytes,
    )
    .map_err(error)?;
    let output = PyDict::new(py);
    for asset in assets {
        if output.contains(&asset.id)? {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "duplicate input asset identity",
            ));
        }
        let bytes = asset.load(&provider).map_err(error)?;
        output.set_item(asset.id, PyBytes::new(py, &bytes))?;
    }
    Ok(output)
}

/// Verify each declared asset identity before constructing an execution graph.
#[pyfunction]
fn verify_input_assets(manifest: &str, assets: &Bound<'_, PyDict>) -> PyResult<()> {
    let declared: Vec<fujimod_catalog::emulator::Asset> = serde_json::from_str(manifest)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let mut seen = std::collections::BTreeSet::new();
    for asset in declared {
        if !seen.insert(asset.id.clone()) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "duplicate input asset identity",
            ));
        }
        let bytes = assets.get_item(&asset.id)?.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!("missing input asset {}", asset.id))
        })?;
        asset
            .check(bytes.cast::<PyBytes>()?.as_bytes())
            .map_err(pyo3::exceptions::PyValueError::new_err)?;
    }
    if seen.len() != assets.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "undeclared input assets",
        ));
    }
    Ok(())
}

/// Incremental SHA-256 for bounded host file verification.
#[pyclass(module = "fujimod._native.emulator")]
struct Sha256 {
    digest: sha2::Sha256,
}

#[pymethods]
impl Sha256 {
    #[new]
    #[pyo3(signature=(data=None))]
    fn new(data: Option<&[u8]>) -> Self {
        use sha2::Digest;
        let mut digest = sha2::Sha256::new();
        if let Some(data) = data {
            digest.update(data);
        }
        Self { digest }
    }

    fn update(&mut self, data: &[u8]) {
        use sha2::Digest;
        self.digest.update(data);
    }

    fn hexdigest(&self) -> String {
        use sha2::Digest;
        format!("{:x}", self.digest.clone().finalize())
    }
}

/// Pure host checksums; batching avoids a call per immutable sector.
#[pyfunction]
fn sha256_many(values: Vec<Bound<'_, PyBytes>>) -> Vec<String> {
    values
        .iter()
        .map(|value| fujimod_emulator_devices::exr2::display::state::sha256(value.as_bytes()))
        .collect()
}

#[pymodule(
    name = "emulator",
    module = "fujimod._native",
    submodule,
    gil_used = true
)]
mod binding {
    #[pymodule_export]
    use super::{NativeError, Sha256, load_input_assets, sha256_many, verify_input_assets};

    #[cfg(not(target_os = "emscripten"))]
    #[pymodule_export]
    use super::board::{
        AnalogInputs, Board, CardMedium, Checkpoint, DeviceContext, NandMedium, Xp4,
        normalize_card_files, persistent_packet,
    };

    #[pymodule_export]
    use super::discovery::{FrontierEntry, Intervention};
    #[pymodule_export]
    use super::display_pixels::decode_display_rgb;
    #[pymodule_export]
    use super::film::FilmStage;
    #[pymodule_export]
    use super::jpeg::{jpeg_decode_yycbcr, jpeg_encode_yycbcr};
    #[pymodule_export]
    use super::method4::method4_decode;
    #[pymodule_export]
    use super::mono_raster::{mono12_phase_gain, zero_row_edges};
    #[pymodule_export]
    use super::python_graphics::{
        UiBatch, clear_parse, clear_path_descriptor, clear_render, copy_untile, gpu_ui_render,
        graphics_memo_clear, graphics_memo_len, math_check, ui_batch, ui_initialize,
        ui_stream_draws, ui_stream_parse,
    };
    #[pymodule_export]
    use super::python_values::DeviceRefusal;
    #[pymodule_export]
    use super::raster::{ui_composite, ui_composite_tiled, ui_fill_counts};
    #[pymodule_export]
    use super::scanout_observation::{render_scanout_snapshot, scanout_snapshot_state};
    #[pymodule_export]
    use super::scene::{
        HostImage, virtual_scene_histogram, virtual_scene_statistics, virtual_scene_yycbcr,
    };

    #[pymodule_export]
    const UNICORN_ABI: &str = "2.1.4";
    #[pymodule_export]
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    #[pymodule_export]
    const RUSTC_VERSION: &str = fujimod_emulator::RUSTC_VERSION;
    #[pymodule_export]
    const TARGET: &str = fujimod_emulator::TARGET;
    #[pymodule_export]
    const PYO3_VERSION: &str = "0.30.1";
    #[pymodule_export]
    const VIRTUAL_SCENE_PROFILE: &str = super::scene::PROFILE;
    #[pymodule_export]
    const STATISTICS_PROFILE: &str = super::scene::STATISTICS_PROFILE;
}

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = binding::_PYO3_DEF.make_module(py)?;
    parent.add_submodule(module.bind(py))?;
    let modules = py
        .import("sys")?
        .getattr("modules")?
        .cast_into::<PyDict>()?;
    modules.set_item("fujimod._native.emulator", &module)?;
    Ok(())
}

pub(crate) fn client_register(id: i32) -> Result<fujimod_emulator_cpu::Register, String> {
    use fujimod_emulator_cpu::arm::Register as R;
    Ok(fujimod_emulator_cpu::Register::Arm32(match id {
        66..=78 => R::R((id - 66) as u8),
        10 => R::R(14),
        11 => R::R(15),
        12 => R::R(13),
        3 => R::Cpsr,
        13 => R::Spsr,
        4 => R::Fpexc,
        111 => R::Cpacr,
        140 => {
            return Ok(fujimod_emulator_cpu::Register::Diagnostic(
                fujimod_emulator_cpu::Diagnostic::ExceptionSyndrome,
            ));
        }
        6 => R::Fpscr,
        8 => R::Fpsid,
        14..=45 => R::D((id - 14) as u8),
        50..=65 => R::Q((id - 50) as u8),
        79..=110 => R::S((id - 79) as u8),
        _ => return Err(format!("unknown native ARM register ID {id}")),
    }))
}
