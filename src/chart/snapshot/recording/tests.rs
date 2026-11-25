use super::*;
use crate::core::{Font, Size, Theme, renderer};
use crate::mark::pie::{label, slice};
use crate::widget::graphics::text::font_system;

// The test picks a face that the host can legally embed and that maps its label
// characters. The glyph positions and paths still come from the real pie scene.
fn label_font() -> Font {
    let mut fonts = font_system().write().expect("font system");
    let db = fonts.raw().db();
    let name = db
        .faces()
        .find_map(|face| {
            let usable = db
                .with_face_data(face.id, |bytes, index| {
                    if index != 0 || bytes.starts_with(b"ttcf") {
                        return false;
                    }
                    ttf_parser::Face::parse(bytes, 0).is_ok_and(|font| {
                        matches!(
                            font.permissions(),
                            Some(ttf_parser::Permissions::Installable | ttf_parser::Permissions::Editable)
                        ) && font.is_outline_embedding_allowed()
                            && "257%".chars().all(|c| font.glyph_index(c).is_some())
                    })
                })
                .unwrap_or(false);
            usable.then(|| face.families[0].0.clone())
        })
        .expect("an embeddable font with percentage digits");
    Font::new(Box::leak(name.into_boxed_str()))
}

#[test]
fn pie_records_curved_slices_outside_leaders_then_selectable_percentage_labels() {
    let red = Color::from_rgb(1., 0., 0.);
    let blue = Color::from_rgb(0., 0., 1.);
    let font = label_font();
    let data = crate::data(
        crate::pie([
            slice(1).name("North").color(crate::Color::Fixed(red)),
            slice(3).name("South").color(crate::Color::Fixed(blue)),
        ])
        .labels(label::Label::percent().with_position(label::Outside).with_font(font)),
    );
    let bounds = Rectangle::new(Point::new(13., 19.), Size::new(420., 260.));
    let viewport = Rectangle::new(Point::new(24., 25.), Size::new(396., 240.));
    let theme = Theme::Light;
    let appearance = crate::chart::transparent(&theme);
    let scene = pie(
        &data,
        &theme,
        &appearance,
        Padding::new(12.),
        Settings {
            font,
            ..Settings::default()
        },
        &renderer::Style::default(),
        bounds,
        &viewport,
    )
    .expect("recorded pie");
    let slices = scene
        .operations
        .iter()
        .enumerate()
        .filter_map(|(i, op)| match op {
            Operation::Fill { path, color, .. } if *color == red || *color == blue => Some((i, path, *color)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(slices.len(), 2);
    assert_eq!([slices[0].2, slices[1].2], [red, blue]);
    for (_, path, _) in &slices {
        use crate::widget::graphics::geometry::path::lyon_path::Event;
        assert!(
            path.raw()
                .iter()
                .any(|e| matches!(e, Event::Cubic { .. } | Event::Quadratic { .. }))
        );
    }
    let leaders = scene
        .operations
        .iter()
        .enumerate()
        .filter_map(|(i, op)| matches!(op, Operation::Stroke { .. }).then_some(i))
        .collect::<Vec<_>>();
    assert_eq!(leaders.len(), 2);
    let labels = scene
        .operations
        .iter()
        .enumerate()
        .filter_map(|(i, op)| match op {
            Operation::Text(g) if !g.is_empty() => Some((i, g)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 2);
    assert!(slices[1].0 < leaders[0] && leaders[1] < labels[0].0);
    let values: Vec<String> = labels
        .iter()
        .map(|(_, g)| g.iter().map(|g| g.character).collect())
        .collect();
    assert_eq!(values, ["25%", "75%"]);
    assert_ne!(labels[0].1[0].origin, labels[1].1[0].origin);
    assert!(
        scene
            .operations
            .iter()
            .any(|op| matches!(op,Operation::Clip(r) if *r==viewport))
    );
    let svg = scene.svg();
    assert!(svg.contains("25%</tspan>") && svg.contains("75%</tspan>"));
    assert!(svg.contains("@font-face") && svg.contains("transform=\"matrix("));
    assert!(!svg.contains("<image"));
    for (scope, other) in [(17, 29), (29, 17)] {
        let inline = scene.svg_scoped(scope);
        assert!(inline.contains(&format!("id=\"hyozu-{scope}-clip-0\"")));
        assert!(inline.contains(&format!("url(#hyozu-{scope}-clip-0)")));
        assert!(inline.contains(&format!("font-family:hyozu-{scope}-face-0;")));
        assert!(inline.contains(&format!("font-family=\"hyozu-{scope}-face-0\"")));
        assert!(!inline.contains(&format!("hyozu-{other}-")));
        assert!(inline.contains("25%</tspan>") && inline.contains("75%</tspan>"));
    }
    let mut fonts = Fonts::default();
    let first = scene.svg_in(2, &mut fonts);
    let second = scene.svg_in(3, &mut fonts);
    assert!(first.contains("font-family:hyozu-2-face-0;") && !second.contains("@font-face"));
    assert!(second.contains("font-family=\"hyozu-2-face-0\""));
    assert!(second.contains("url(#hyozu-3-clip-0)"));
}
