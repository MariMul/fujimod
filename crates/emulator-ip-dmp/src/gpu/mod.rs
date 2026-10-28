//! Shared host GPU device, queue, and resident surface API.

mod surfaces;

pub use surfaces::{
    GpuContext, GpuSurface, PixelWriteMask, SurfaceExtent, SurfaceFormat, SurfaceKey,
    SurfaceLayout, SurfaceReadback, context,
};
