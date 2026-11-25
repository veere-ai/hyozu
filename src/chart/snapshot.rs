//! A settled chart scene drawn into the caller's renderer at its final logical size.

#[cfg(feature = "vector-export")]
pub mod recording;

use super::Style;
use super::scene::Scene;
use crate::core::{Layout, Padding, Rectangle, mouse, renderer, text};
use crate::widget::renderer::geometry;
use crate::{Data, Design};

/// Draw a single chart with the same frame, marks, guides, legend and shaped
/// labels as the live chart. Exporters can supply a renderer that records the
/// drawing operations. Bounds and viewport share the caller's coordinate space.
///
/// The chart always draws at the end of its animation. Composition outside the
/// Hyozu scene, such as a facet grid or a donut's centre widget, belongs to the caller.
#[allow(clippy::too_many_arguments)]
pub fn draw<R>(
    data: &Data,
    design: &dyn Design,
    appearance: &Style,
    padding: Padding,
    renderer: &mut R,
    defaults: &renderer::Style,
    bounds: Rectangle,
    viewport: &Rectangle,
) where
    R: text::Renderer + geometry::Renderer,
{
    let Some(clip) = bounds.intersection(viewport) else {
        return;
    };
    let mut scene = Scene::<(), R>::with_animation(data, false);
    let mut tree = scene.state();
    scene.layout_padded(&mut tree, renderer, bounds.size(), padding, Some(design));
    let origin = bounds.position() + tree.translation;
    let layout = Layout::new(tree.size).move_to(origin);
    renderer.with_layer(clip, |renderer| {
        super::draw_background(renderer, appearance, bounds);
        scene.draw(
            &tree,
            renderer,
            design,
            defaults,
            layout,
            mouse::Cursor::Unavailable,
            &clip,
            &Default::default(),
            appearance.corners,
            appearance.track,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::renderer::{Headless, Settings};
    use crate::core::widget::Tree;
    use crate::core::{Color, Point, Renderer as _, Size, Theme, Widget, layout};
    use crate::mark::pie::{label, slice};
    use crate::widget::Renderer;

    #[test]
    fn animated_pie_snapshot_matches_the_settled_widget_in_a_clipped_frame() {
        let mut renderer = iced::futures::executor::block_on(Renderer::new(Settings::default(), Some("tiny-skia")))
            .expect("headless renderer");
        let red = Color::from_rgb(1., 0., 0.);
        let blue = Color::from_rgb(0., 0., 1.);
        let data = crate::data(
            crate::pie([
                slice(1).name("North").color(crate::Color::Fixed(red)),
                slice(3).name("South").color(crate::Color::Fixed(blue)),
            ])
            .labels(label::Label::percent().with_position(label::Outside)),
        )
        .title("Revenue")
        .legend(crate::legend::Config::below())
        .animate(true);
        let settled = data.clone().animate(false);
        let theme = Theme::Light;
        let appearance = crate::chart::default(&theme);
        let defaults = renderer::Style::default();
        let padding = Padding::new(14.);
        let bounds = Rectangle::new(Point::new(18., 22.), Size::new(320., 204.));
        let clip = Rectangle::new(Point::new(25., 27.), Size::new(285., 188.));
        let window = Size::new(360, 252);
        let viewport = Rectangle::with_size(Size::new(360., 252.));

        let mut chart = crate::chart::<(), Theme>(&settled).design(&theme).padding(padding);
        let mut tree = Tree::new(&chart);
        chart.diff(&mut tree);
        chart.layout(&mut tree, &renderer, &layout::Limits::new(Size::ZERO, bounds.size()));
        renderer.reset(viewport);
        renderer.with_layer(clip, |renderer| {
            chart.draw(
                &tree,
                renderer,
                &theme,
                &defaults,
                Layout::new(tree.size).move_to(bounds.position()),
                mouse::Cursor::Unavailable,
                &clip,
            );
        });
        let reference = renderer.screenshot(window, 1., Color::TRANSPARENT);

        renderer.reset(viewport);
        draw(
            &data,
            &theme,
            &appearance,
            padding,
            &mut renderer,
            &defaults,
            bounds,
            &clip,
        );
        let snapshot = renderer.screenshot(window, 1., Color::TRANSPARENT);
        assert!(snapshot.chunks_exact(4).any(|p| p == [255, 0, 0, 255]));
        assert!(snapshot.chunks_exact(4).any(|p| p == [0, 0, 255, 255]));
        assert_eq!(snapshot[(24 + 35 * window.width as usize) * 4 + 3], 0);
        assert!(
            snapshot == reference,
            "the settled chart and export must paint identical pixels"
        );
    }
}
