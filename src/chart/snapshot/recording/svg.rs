use super::{Glyph, Operation, Scene};
use crate::core::Color;
use crate::widget::graphics::geometry::fill::Rule;
use crate::widget::graphics::geometry::path::lyon_path::Event;
use crate::widget::graphics::geometry::{LineCap, LineJoin, Path};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use std::collections::HashMap;
use std::fmt::Write;

/// Previously embedded font files on a page of inline chart drawings.
#[derive(Default)]
pub struct Fonts<'a> {
    faces: HashMap<&'a [u8], (u64, usize)>,
}

impl Scene {
    /// Encode the recording as a standalone SVG with selectable text and embedded fonts.
    pub fn svg(&self) -> String {
        self.svg_scoped(0)
    }

    /// Encode an inline SVG using a document-unique chart number for its clips and fonts.
    pub fn svg_scoped(&self, scope: u64) -> String {
        self.svg_in(scope, &mut Fonts::default())
    }

    /// Write the same scoped SVG while embedding a font only on its first use
    /// in this document. Different bytes always receive different family names.
    pub fn svg_in<'a>(&'a self, scope: u64, fonts: &mut Fonts<'a>) -> String {
        let mut out = String::new();
        self.write_svg(&mut out, scope, fonts)
            .expect("writing to a String cannot fail");
        out
    }

    fn write_svg<'a>(&'a self, out: &mut String, scope: u64, fonts: &mut Fonts<'a>) -> std::fmt::Result {
        let bounds = self.bounds;
        write!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">",
            bounds.width, bounds.height, bounds.x, bounds.y, bounds.width, bounds.height
        )?;
        let mut families = Vec::with_capacity(self.faces.len());
        let mut opened = false;
        for (i, face) in self.faces.iter().enumerate() {
            let key = (scope, i);
            let entry = fonts.faces.entry(face.bytes.as_slice());
            let family = *entry.or_insert(key);
            families.push(family);
            if family == key {
                if !opened {
                    out.push_str("<defs><style>");
                    opened = true;
                }
                let mime = if face.bytes.starts_with(b"OTTO") {
                    "font/otf"
                } else {
                    "font/ttf"
                };
                write!(
                    out,
                    "@font-face{{font-family:hyozu-{scope}-face-{i};src:url(data:{mime};base64,"
                )?;
                STANDARD.encode_string(&face.bytes, out);
                out.push_str(");}");
            }
        }
        if opened {
            out.push_str("</style></defs>");
        }
        let mut clip = 0;
        for op in &self.operations {
            match op {
                Operation::Clip(r) => {
                    write!(
                        out,
                        "<defs><clipPath id=\"hyozu-{scope}-clip-{clip}\" clipPathUnits=\"userSpaceOnUse\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs><g clip-path=\"url(#hyozu-{scope}-clip-{clip})\">",
                        r.x, r.y, r.width, r.height
                    )?;
                    clip += 1;
                }
                Operation::Transform([a, b, c, d, x, y]) => {
                    write!(out, "<g transform=\"matrix({a} {b} {c} {d} {x} {y})\">")?
                }
                Operation::End => out.push_str("</g>"),
                Operation::Fill { path, color, rule } => {
                    let rule = match rule {
                        Rule::NonZero => "nonzero",
                        Rule::EvenOdd => "evenodd",
                    };
                    write!(out, "<path fill-rule=\"{rule}\" ")?;
                    paint(out, "fill", *color)?;
                    path_data(out, path)?;
                }
                Operation::Stroke {
                    path,
                    color,
                    width,
                    cap,
                    join,
                } => {
                    let cap = match cap {
                        LineCap::Butt => "butt",
                        LineCap::Square => "square",
                        LineCap::Round => "round",
                    };
                    let join = match join {
                        LineJoin::Miter => "miter",
                        LineJoin::Bevel => "bevel",
                        LineJoin::Round => "round",
                    };
                    write!(
                        out,
                        "<path fill=\"none\" stroke-width=\"{width}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\" "
                    )?;
                    paint(out, "stroke", *color)?;
                    path_data(out, path)?;
                }
                Operation::Text(glyphs) => {
                    out.push_str("<text xml:space=\"preserve\" style=\"font-synthesis:none;font-kerning:none;font-variant-ligatures:none;font-optical-sizing:none\">");
                    for run in glyphs.chunk_by(|a, b| {
                        a.face == b.face
                            && a.size == b.size
                            && a.color == b.color
                            && a.weight == b.weight
                            && a.optical_size == b.optical_size
                    }) {
                        span(out, run, families[run[0].face])?;
                    }
                    out.push_str("</text>");
                }
            }
        }
        out.push_str("</svg>");
        Ok(())
    }
}

fn paint(out: &mut String, property: &str, color: Color) -> std::fmt::Result {
    write!(
        out,
        "{property}=\"rgb({}% {}% {}%)\" {property}-opacity=\"{}\" ",
        color.r * 100.,
        color.g * 100.,
        color.b * 100.,
        color.a
    )
}

fn path_data(out: &mut String, path: &Path) -> std::fmt::Result {
    out.push_str("d=\"");
    for event in path.raw().iter() {
        match event {
            Event::Begin { at } => write!(out, "M{} {} ", at.x, at.y)?,
            Event::Line { to, .. } => write!(out, "L{} {} ", to.x, to.y)?,
            Event::Quadratic { ctrl, to, .. } => write!(out, "Q{} {} {} {} ", ctrl.x, ctrl.y, to.x, to.y)?,
            Event::Cubic { ctrl1, ctrl2, to, .. } => write!(
                out,
                "C{} {} {} {} {} {} ",
                ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y
            )?,
            Event::End { close: true, .. } => out.push('Z'),
            Event::End { close: false, .. } => {}
        }
    }
    out.push_str("\"/>");
    Ok(())
}

fn span(out: &mut String, run: &[Glyph], (scope, face): (u64, usize)) -> std::fmt::Result {
    let g = &run[0];
    write!(
        out,
        "<tspan font-family=\"hyozu-{scope}-face-{face}\" font-size=\"{}\" style=\"font-variation-settings:'wght' {}",
        g.size, g.weight
    )?;
    if let Some(size) = g.optical_size {
        write!(out, ", 'opsz' {size}")?;
    }
    out.push_str("\" ");
    paint(out, "fill", g.color)?;
    out.push_str("x=\"");
    for glyph in run {
        write!(out, "{} ", glyph.origin.x)?;
    }
    out.push_str("\" y=\"");
    for glyph in run {
        write!(out, "{} ", glyph.origin.y)?;
    }
    out.push_str("\">");
    for glyph in run {
        match glyph.character {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            c => out.push(c),
        }
    }
    out.push_str("</tspan>");
    Ok(())
}
