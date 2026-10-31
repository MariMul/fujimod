//! Rasterization workload of one command list, counted from the decoded
//! geometry and register state. Every renderer counts through this module from
//! the same vertex-shader outputs, so the totals do not depend on whether the
//! CPU rasterizer or a host GPU produced the pixels.
use super::{
    draw_state::{DrawState, PrimitiveMode},
    raster,
    shader::Vector,
};
use serde::{Deserialize, Serialize};

/// Draw calls and covered fragments of one command list. A fragment is a pixel
/// center inside a primitive, the viewport and the scissor, counted once per
/// primitive that covers it, before alpha, stencil or depth tests. Fragments of
/// draws with blending enabled count as blended; all others count as filled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workload {
    pub draws: u64,
    pub filled_pixels: u64,
    pub blended_pixels: u64,
}

/// Primitive assembly for counting, independent of each renderer's own
/// batching. It persists across lists like the hardware's vertex queue.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Assembly {
    vertices: Vec<[Vector; 16]>,
    primitive_index: u64,
    immediate_open: bool,
}

impl Assembly {
    /// A primitive restart (0x431 or 0x45f) discards queued vertices.
    pub(super) fn restart(&mut self) {
        self.vertices.clear();
        self.primitive_index = 0;
    }

    /// An indexed or array draw trigger is one draw call.
    pub(super) fn draw_trigger(&mut self, workload: &mut Workload) {
        workload.draws += 1;
    }

    /// Immediate vertices between two other register writes form one draw call.
    pub(super) fn immediate_vertex(&mut self, workload: &mut Workload) {
        if !self.immediate_open {
            self.immediate_open = true;
            workload.draws += 1;
        }
    }

    /// Any register write other than the attribute FIFO ends an immediate draw.
    pub(super) fn register_write(&mut self, index: u16) {
        if !matches!(index, 0x432..=0x435) {
            self.immediate_open = false;
        }
    }

    /// Assemble one shaded vertex and count the fragments of every primitive
    /// it completes, mirroring the CPU rasterizer's assembly order.
    pub(super) fn vertex(
        &mut self,
        output: [Vector; 16],
        state: &DrawState,
        workload: &mut Workload,
    ) -> Result<(), String> {
        let mode = state.raster.primitive_mode;
        let covered = if mode == PrimitiveMode::Points {
            raster::point_coverage(state, &output)?
        } else {
            self.vertices.push(output);
            let n = self.vertices.len();
            match mode {
                PrimitiveMode::Triangles if n == 3 => {
                    let triangle = [self.vertices[0], self.vertices[1], self.vertices[2]];
                    self.vertices.clear();
                    raster::triangle_coverage(state, triangle)?
                }
                PrimitiveMode::TriangleStrip if n >= 3 => {
                    let v = &self.vertices;
                    let triangle = if self.primitive_index & 1 != 0 {
                        [v[n - 2], v[n - 3], v[n - 1]]
                    } else {
                        [v[n - 3], v[n - 2], v[n - 1]]
                    };
                    self.primitive_index += 1;
                    if n > 3 {
                        self.vertices.remove(0);
                    }
                    raster::triangle_coverage(state, triangle)?
                }
                PrimitiveMode::TriangleFan if n >= 3 => {
                    let triangle = [self.vertices[0], self.vertices[n - 2], self.vertices[n - 1]];
                    self.vertices.remove(1);
                    raster::triangle_coverage(state, triangle)?
                }
                PrimitiveMode::Lines | PrimitiveMode::LineStrip if n == 2 => {
                    let line = [self.vertices[0], self.vertices[1]];
                    if mode == PrimitiveMode::Lines {
                        self.vertices.clear();
                    } else {
                        self.vertices.remove(0);
                    }
                    raster::line_coverage(state, line)?
                }
                PrimitiveMode::Other(code) => {
                    return Err(format!("DMP unsupported primitive mode {code}"));
                }
                _ => 0,
            }
        };
        if state.blend.enabled {
            workload.blended_pixels += covered;
        } else {
            workload.filled_pixels += covered;
        }
        Ok(())
    }
}
