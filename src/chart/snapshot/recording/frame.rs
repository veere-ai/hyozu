use super::{Operation, Unsupported};
use crate::core::{Point, Radians, Rectangle, Size, Vector};
use crate::widget::graphics::{cache, geometry};
use geometry::path::lyon_path::math::{Angle, Transform};
use geometry::{Fill, Path, Stroke, Style};

#[derive(Clone, Default)]
pub struct Geometry {
    pub operations: Vec<Operation>,
    pub error: Option<Unsupported>,
}

impl cache::Cached for Geometry {
    type Cache = Self;
    fn cache(self, _: cache::Group, _: Option<Self>) -> Self {
        self
    }
    fn load(cache: &Self) -> Self {
        cache.clone()
    }
}

pub struct Frame {
    bounds: Rectangle,
    geometry: Geometry,
    current: Transform,
    stack: Vec<Transform>,
}

impl Frame {
    pub fn new(bounds: Rectangle) -> Self {
        Self {
            bounds,
            geometry: Default::default(),
            current: Transform::identity(),
            stack: Vec::new(),
        }
    }
    fn emit(&mut self, operation: Operation) {
        if self.current == Transform::identity() {
            self.geometry.operations.push(operation);
        } else {
            self.geometry
                .operations
                .extend([Operation::Transform(self.current.to_array()), operation, Operation::End]);
        }
    }
    fn reject(&mut self, operation: &'static str) {
        self.geometry.error.get_or_insert(Unsupported(operation));
    }
}

impl geometry::frame::Backend for Frame {
    type Geometry = Geometry;
    fn width(&self) -> f32 {
        self.bounds.width
    }
    fn height(&self) -> f32 {
        self.bounds.height
    }
    fn size(&self) -> Size {
        self.bounds.size()
    }
    fn center(&self) -> Point {
        Point::new(self.width() / 2., self.height() / 2.)
    }
    fn push_transform(&mut self) {
        self.stack.push(self.current);
    }
    fn pop_transform(&mut self) {
        if let Some(t) = self.stack.pop() {
            self.current = t;
        } else {
            self.reject("unbalanced frame transform");
        }
    }
    fn translate(&mut self, v: Vector) {
        self.current = self
            .current
            .pre_translate(geometry::path::lyon_path::math::vector(v.x, v.y));
    }
    fn rotate(&mut self, angle: impl Into<Radians>) {
        self.current = self.current.pre_rotate(Angle::radians(angle.into().0));
    }
    fn scale(&mut self, scale: f32) {
        self.current = self.current.pre_scale(scale, scale);
    }
    fn scale_nonuniform(&mut self, scale: impl Into<Vector>) {
        let v = scale.into();
        self.current = self.current.pre_scale(v.x, v.y);
    }
    fn draft(&mut self, bounds: Rectangle) -> Self {
        Self::new(bounds)
    }
    fn paste(&mut self, child: Self) {
        let child = child.into_geometry();
        self.geometry.error = self.geometry.error.or(child.error);
        self.geometry.operations.extend(child.operations);
    }
    fn fill(&mut self, path: &Path, fill: impl Into<Fill>) {
        let fill = fill.into();
        match fill.style {
            Style::Solid(color) => self.emit(Operation::Fill {
                path: path.clone(),
                color,
                rule: fill.rule,
            }),
            Style::Gradient(_) => self.reject("gradient path fill"),
        }
    }
    fn fill_rectangle(&mut self, at: Point, size: Size, fill: impl Into<Fill>) {
        self.fill(&Path::rectangle(at, size), fill);
    }
    fn stroke<'a>(&mut self, path: &Path, stroke: impl Into<Stroke<'a>>) {
        let stroke = stroke.into();
        if !stroke.line_dash.segments.is_empty() {
            self.reject("dashed stroke");
            return;
        }
        match stroke.style {
            Style::Solid(color) => self.emit(Operation::Stroke {
                path: path.clone(),
                color,
                width: stroke.width,
                cap: stroke.line_cap,
                join: stroke.line_join,
            }),
            Style::Gradient(_) => self.reject("gradient stroke"),
        }
    }
    fn stroke_rectangle<'a>(&mut self, at: Point, size: Size, stroke: impl Into<Stroke<'a>>) {
        self.stroke(&Path::rectangle(at, size), stroke);
    }
    fn fill_text(&mut self, _: impl Into<geometry::Text>) {
        self.reject("canvas text");
    }
    fn stroke_text<'a>(&mut self, _: impl Into<geometry::Text>, _: impl Into<Stroke<'a>>) {
        self.reject("outlined canvas text");
    }
    fn draw_image(&mut self, _: Rectangle, _: impl Into<geometry::Image>) {
        self.reject("raster image");
    }
    fn draw_svg(&mut self, _: Rectangle, _: impl Into<geometry::Svg>) {
        self.reject("nested SVG image");
    }
    fn into_geometry(mut self) -> Geometry {
        if !self.stack.is_empty() {
            self.reject("unbalanced frame transform");
        }
        self.geometry.operations.insert(0, Operation::Clip(self.bounds));
        self.geometry.operations.push(Operation::End);
        self.geometry
    }
}
