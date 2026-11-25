//! The original paths and positioned glyphs of a settled, single-pie chart.
//! A caller can choose its raster fallback when the recorder encounters an unsupported operation.
mod frame;
mod renderer;
mod svg;
#[cfg(test)]
mod tests;
mod text;

use crate::core::renderer::Settings;
use crate::core::{Color, Padding, Point, Rectangle};
use crate::widget::graphics::geometry::{LineCap, LineJoin, Path, fill};
use crate::{Data, Design};
use std::fmt;

/// One ordered operation. Groups use the coordinate system active at their opening.
#[derive(Debug, Clone)]
pub enum Operation {
    Clip(Rectangle),
    Transform([f32; 6]),
    End,
    Fill {
        path: Path,
        color: Color,
        rule: fill::Rule,
    },
    Stroke {
        path: Path,
        color: Color,
        width: f32,
        cap: LineCap,
        join: LineJoin,
    },
    Text(Vec<Glyph>),
}

/// A shaped scalar with the face, glyph identifier, advance and baseline used to paint it.
#[derive(Debug, Clone)]
pub struct Glyph {
    pub character: char,
    pub face: usize,
    pub id: u16,
    pub origin: Point,
    pub advance: f32,
    pub size: f32,
    pub color: Color,
    pub weight: u16,
    pub optical_size: Option<f32>,
}

/// A complete, standalone OpenType font matching the resolved glyph identifiers.
#[derive(Debug)]
pub struct Face {
    pub postscript_name: String,
    pub bytes: Vec<u8>,
    /// Whether the font's OS/2 embedding flags allow a consumer to embed a subset.
    pub subsetting_allowed: bool,
}

/// Recorded paint order, source coordinate space and font resources.
#[derive(Debug)]
pub struct Scene {
    bounds: Rectangle,
    operations: Vec<Operation>,
    faces: Vec<Face>,
}

impl Scene {
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }
}

/// The drawing operation or resource requiring a different export path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported(pub &'static str);

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Unsupported {}

/// Draw a single pie through the same scene and paragraph layout as the live chart.
#[allow(clippy::too_many_arguments)]
pub fn pie(
    data: &Data,
    design: &dyn Design,
    appearance: &crate::chart::Style,
    padding: Padding,
    settings: Settings,
    defaults: &crate::core::renderer::Style,
    bounds: Rectangle,
    viewport: &Rectangle,
) -> Result<Scene, Unsupported> {
    if !matches!(data.marks(), [crate::data::mark::Mark::Pie(_)]) || !data.secondary.marks().is_empty() {
        return Err(Unsupported("recording currently supports a single pie"));
    }
    if !(bounds.width.is_finite() && bounds.height.is_finite() && bounds.width > 0. && bounds.height > 0.) {
        return Err(Unsupported("invalid chart dimensions"));
    }
    let mut renderer = renderer::Recorder::new(settings);
    super::draw(
        data,
        design,
        appearance,
        padding,
        &mut renderer,
        defaults,
        bounds,
        viewport,
    );
    renderer.finish(bounds)
}
