use super::{Face, Operation, Scene, Unsupported, frame, text as shaped};
use crate::core::renderer::{Quad, Scale, Settings};
use crate::core::text::{self, Paragraph as _};
use crate::core::{self, Background, Color, Point, Rectangle, Transformation, image};
use crate::widget::graphics::{geometry, text as graphics};
use geometry::Path;
use geometry::fill::Rule;
use graphics::cosmic_text::fontdb;
use std::cell::Cell;

/// An iced drawing target for a settled chart and its surrounding static widgets.
/// Unsupported operations are reported together when the host finishes the frame.
pub struct Recorder {
    operations: Vec<Operation>,
    faces: Vec<(fontdb::ID, Face)>,
    problem: Cell<Option<Unsupported>>,
    scale: Option<Scale>,
    settings: Settings,
}

impl Recorder {
    /// Start a recording with the source view's renderer settings.
    pub fn new(settings: Settings) -> Self {
        Self {
            operations: vec![],
            faces: vec![],
            problem: Cell::new(None),
            scale: None,
            settings,
        }
    }
    fn reject(&self, reason: &'static str) {
        self.problem.set(self.problem.get().or(Some(Unsupported(reason))));
    }
    /// Finish a complete scene in the given coordinate space.
    pub fn finish(self, bounds: Rectangle) -> Result<Scene, Unsupported> {
        if let Some(problem) = self.problem.get() {
            return Err(problem);
        }
        if ![bounds.x, bounds.y, bounds.width, bounds.height]
            .into_iter()
            .all(f32::is_finite)
            || bounds.width <= 0.
            || bounds.height <= 0.
        {
            return Err(Unsupported("invalid chart dimensions"));
        }
        let mut depth = 0usize;
        for op in &self.operations {
            match op {
                Operation::Transform(_) | Operation::Clip(_) => depth += 1,
                Operation::End => depth = depth.checked_sub(1).ok_or(Unsupported("unbalanced chart group"))?,
                _ => {}
            }
        }
        if depth != 0 {
            return Err(Unsupported("unclosed chart group"));
        }
        Ok(Scene {
            bounds,
            operations: self.operations,
            faces: self.faces.into_iter().map(|(_, face)| face).collect(),
        })
    }
}

impl core::Renderer for Recorder {
    fn start_composite(&mut self, _: Rectangle, _: f32, _: core::renderer::BlendMode) {
        self.reject("isolated chart group");
    }
    fn start_rotated_clip(&mut self, _: Rectangle, _: core::renderer::Rotation, _: [f32; 4]) {
        self.reject("rotated chart clip");
    }
    fn start_content_clip(&mut self, visible: Rectangle, frame: Rectangle, radii: [f32; 4]) {
        if radii != [0.; 4] {
            self.reject("rounded chart clip");
        }
        self.start_layer(visible.intersection(&frame).unwrap_or_default());
    }
    fn start_layer(&mut self, bounds: Rectangle) {
        self.operations.push(Operation::Clip(bounds));
    }
    fn end_layer(&mut self) {
        self.operations.push(Operation::End);
    }
    fn start_transformation(&mut self, t: Transformation) {
        let [a, b, p, q, c, d, r, s, _, _, _, _, x, y, _, w]: [f32; 16] = t.into();
        if [p, q, r, s, w] != [0., 0., 0., 0., 1.] {
            self.reject("non-affine chart transformation");
        }
        self.operations.push(Operation::Transform([a, b, c, d, x, y]));
    }
    fn end_transformation(&mut self) {
        self.operations.push(Operation::End);
    }
    fn fill_quad(&mut self, quad: Quad, background: impl Into<Background>) {
        if quad.shadow.color.a > 0. {
            self.reject("quad shadow");
            return;
        }
        let Background::Color(color) = background.into() else {
            self.reject("gradient quad background");
            return;
        };
        let bounds = quad.bounds;
        let width = quad.border.width;
        let width = width.max(0.).min(bounds.width.min(bounds.height) / 2.);
        let outer = quad.border.radius;
        let inner = bounds.shrink(width);
        let mut radius = outer;
        radius.top_left = (radius.top_left - width).max(0.);
        radius.top_right = (radius.top_right - width).max(0.);
        radius.bottom_left = (radius.bottom_left - width).max(0.);
        radius.bottom_right = (radius.bottom_right - width).max(0.);
        if color.a > 0. {
            self.operations.push(Operation::Fill {
                path: Path::rounded_rectangle(inner.position(), inner.size(), radius),
                color,
                rule: Rule::NonZero,
            });
        }
        if width > 0. && quad.border.color.a > 0. {
            let path = Path::new(|p| {
                p.rounded_rectangle(bounds.position(), bounds.size(), outer);
                if inner.width > 0. && inner.height > 0. {
                    p.rounded_rectangle(inner.position(), inner.size(), radius);
                }
            });
            self.operations.push(Operation::Fill {
                path,
                color: quad.border.color,
                rule: Rule::EvenOdd,
            });
        }
    }
    fn allocate_image(
        &self,
        _: &image::Handle,
        complete: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
        self.reject("image allocation");
        complete(Err(image::Error::Unsupported));
    }
    fn hint(&mut self, scale: Scale) {
        self.scale = Some(scale);
    }
    fn scale(&self) -> Option<Scale> {
        self.scale
    }
    fn settings(&self) -> Settings {
        self.settings
    }
    fn reset(&mut self, _: Rectangle) {
        self.operations.clear();
        self.faces.clear();
        self.problem.set(None);
    }
}

impl geometry::Renderer for Recorder {
    type Geometry = frame::Geometry;
    type Frame = frame::Frame;
    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        frame::Frame::new(bounds)
    }
    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        self.problem.set(self.problem.get().or(geometry.error));
        self.operations.extend(geometry.operations);
    }
}

impl text::Renderer for Recorder {
    type Paragraph = graphics::Paragraph;
    type Editor = graphics::Editor;
    const ICON_FONT: core::Font = <crate::widget::Renderer as text::Renderer>::ICON_FONT;
    const CHECKMARK_ICON: char = <crate::widget::Renderer as text::Renderer>::CHECKMARK_ICON;
    const ARROW_DOWN_ICON: char = <crate::widget::Renderer as text::Renderer>::ARROW_DOWN_ICON;
    const SCROLL_UP_ICON: char = <crate::widget::Renderer as text::Renderer>::SCROLL_UP_ICON;
    const SCROLL_DOWN_ICON: char = <crate::widget::Renderer as text::Renderer>::SCROLL_DOWN_ICON;
    const SCROLL_LEFT_ICON: char = <crate::widget::Renderer as text::Renderer>::SCROLL_LEFT_ICON;
    const SCROLL_RIGHT_ICON: char = <crate::widget::Renderer as text::Renderer>::SCROLL_RIGHT_ICON;
    const ICED_LOGO: char = <crate::widget::Renderer as text::Renderer>::ICED_LOGO;
    fn fill_paragraph(&mut self, p: &Self::Paragraph, position: Point, color: Color, clip: Rectangle) {
        match shaped::glyphs(p, position, color, &mut self.faces) {
            Ok(glyphs) => self
                .operations
                .extend([Operation::Clip(clip), Operation::Text(glyphs), Operation::End]),
            Err(reason) => self.reject(reason.0),
        }
    }
    fn fill_text(&mut self, text: text::Text<String>, mut position: Point, color: Color, clip: Rectangle) {
        let p = graphics::Paragraph::with_text(text.as_ref());
        position.x -= match text.align_x {
            text::Alignment::Center => p.min_width() / 2.,
            text::Alignment::Right => p.min_width(),
            _ => 0.,
        };
        position.y -= match text.align_y {
            core::alignment::Vertical::Center => p.min_height() / 2.,
            core::alignment::Vertical::Bottom => p.min_height(),
            _ => 0.,
        };
        self.fill_paragraph(&p, position, color, clip);
    }
    fn fill_editor(&mut self, _: &Self::Editor, _: Point, _: Color, _: Rectangle) {
        self.reject("editable text");
    }
}
