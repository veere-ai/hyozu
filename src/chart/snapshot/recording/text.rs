use super::{Face, Glyph, Unsupported};
use crate::core::text::Paragraph as _;
use crate::core::{Color, Point};
use crate::widget::graphics::text::cosmic_text::{CacheKeyFlags, fontdb};
use crate::widget::graphics::text::{self, Paragraph};

fn face_index(
    faces: &mut Vec<(fontdb::ID, Face)>,
    db: &fontdb::Database,
    id: fontdb::ID,
) -> Result<usize, Unsupported> {
    if let Some(index) = faces.iter().position(|(known, _)| *known == id) {
        return Ok(index);
    }
    let info = db.face(id).ok_or(Unsupported("missing resolved font face"))?;
    let face = db
        .with_face_data(id, |bytes, index| {
            if index != 0 || bytes.starts_with(b"ttcf") {
                return Err(Unsupported("font collection"));
            }
            let font = ttf_parser::Face::parse(bytes, 0).map_err(|_| Unsupported("unsupported OpenType face"))?;
            if !matches!(
                font.permissions(),
                Some(ttf_parser::Permissions::Installable | ttf_parser::Permissions::Editable)
            ) || !font.is_outline_embedding_allowed()
            {
                return Err(Unsupported("font does not allow editable outline embedding"));
            }
            Ok(Face {
                bytes: bytes.to_vec(),
                postscript_name: info.post_script_name.clone(),
                subsetting_allowed: font.is_subsetting_allowed(),
            })
        })
        .ok_or(Unsupported("font bytes unavailable"))??;
    let index = faces.len();
    faces.push((id, face));
    Ok(index)
}

pub(super) fn glyphs(
    paragraph: &Paragraph,
    position: Point,
    color: Color,
    faces: &mut Vec<(fontdb::ID, Face)>,
) -> Result<Vec<Glyph>, Unsupported> {
    if paragraph.hint_factor().is_some() {
        return Err(Unsupported("hinted paragraph metrics"));
    }
    let mut fonts = text::font_system()
        .write()
        .map_err(|_| Unsupported("font system unavailable"))?;
    let db = fonts.raw().db();
    let buffer = paragraph.buffer();
    let left = position.x - buffer.scroll().horizontal;
    let mut output = Vec::new();
    for run in buffer.layout_runs() {
        if run.rtl || !run.decorations.is_empty() {
            return Err(Unsupported("RTL or decorated paragraph"));
        }
        for glyph in run.glyphs {
            let cluster = run
                .text
                .get(glyph.start..glyph.end)
                .ok_or(Unsupported("invalid source cluster"))?;
            let mut chars = cluster.chars();
            let character = chars.next().ok_or(Unsupported("empty shaped cluster"))?;
            if chars.next().is_some()
                || glyph.level.is_rtl()
                || !glyph.font_variations.variations.is_empty()
                || glyph.cache_key_flags.contains(CacheKeyFlags::FAKE_ITALIC)
            {
                return Err(Unsupported("complex or synthetic shaped glyph"));
            }
            let face = face_index(faces, db, glyph.font_id)?;
            let font =
                ttf_parser::Face::parse(&faces[face].1.bytes, 0).map_err(|_| Unsupported("invalid font face"))?;
            if font.glyph_index(character).map(|id| id.0) != Some(glyph.glyph_id) || glyph.glyph_id == 0 {
                return Err(Unsupported("shaped glyph has no direct character mapping"));
            }
            let color = glyph.color_opt.map_or(color, |c| {
                Color::from_rgba8(c.r(), c.g(), c.b(), f32::from(c.a()) / 255.)
            });
            output.push(Glyph {
                character,
                face,
                id: glyph.glyph_id,
                advance: glyph.w,
                size: glyph.font_size,
                color,
                origin: Point::new(
                    left + glyph.x + glyph.font_size * glyph.x_offset,
                    position.y + run.line_y + glyph.y - glyph.font_size * glyph.y_offset,
                ),
                weight: glyph.font_weight.0,
                optical_size: glyph.optical_size.resolve(glyph.font_size),
            });
        }
    }
    Ok(output)
}
