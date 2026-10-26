use gpui_kit::{Bounds, Path, PathBuilder, Pixels, Rgba, Window, point};

// Match VideoDisplay::Render and DrawOverscanMask: the two safe regions use
// the existing 16:9 / 4:3 margins and cubic corners, before visual tool drawing.
fn margins(aspect: f32) -> [(f32, f32); 2] {
    if aspect > 1.75 {
        [(0.1, 0.05), (0.035, 0.035)]
    } else {
        [(0.067, 0.05), (0.033, 0.035)]
    }
}

fn mask_path(bounds: Bounds<Pixels>, horizontal: f32, vertical: f32) -> Option<Path<Pixels>> {
    let inset = point(
        bounds.size.width * horizontal,
        bounds.size.height * vertical,
    );
    let corners = [
        bounds.origin + inset,
        point(bounds.right() - inset.x, bounds.top() + inset.y),
        bounds.bottom_right() - inset,
        point(bounds.left() + inset.x, bounds.bottom() - inset.y),
    ];
    // The default even/odd fill subtracts the rounded inner region from the image.
    let mut builder = PathBuilder::fill();
    builder.move_to(bounds.origin);
    builder.line_to(bounds.top_right());
    builder.line_to(bounds.bottom_right());
    builder.line_to(bounds.bottom_left());
    builder.close();
    for index in 0..corners.len() {
        let corner = corners[index];
        let from = (corners[(index + 3) % 4] + corner * 4.) / 5.;
        let to = (corners[(index + 1) % 4] + corner * 4.) / 5.;
        if index == 0 {
            builder.move_to(from);
        } else {
            builder.line_to(from);
        }
        builder.cubic_bezier_to(to, corner, corner);
    }
    builder.close();
    builder.build().ok()
}

pub fn paint(bounds: Bounds<Pixels>, aspect: f32, window: &mut Window) {
    for (horizontal, vertical) in margins(aspect) {
        if let Some(path) = mask_path(bounds, horizontal, vertical) {
            window.paint_path(
                path,
                Rgba {
                    r: 30. / 255.,
                    g: 70. / 255.,
                    b: 200. / 255.,
                    a: 0.5,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{Point, px, size};

    fn area(path: &Path<Pixels>) -> f32 {
        path.vertices
            .chunks_exact(3)
            .map(|vertices| {
                let a = vertices[1].xy_position - vertices[0].xy_position;
                let b = vertices[2].xy_position - vertices[0].xy_position;
                (f32::from(a.x) * f32::from(b.y) - f32::from(a.y) * f32::from(b.x)).abs() / 2.
            })
            .sum()
    }

    fn contains(path: &Path<Pixels>, position: Point<Pixels>) -> bool {
        path.vertices.chunks_exact(3).any(|vertices| {
            let cross = |a: Point<Pixels>, b: Point<Pixels>| {
                f32::from(a.x - position.x) * f32::from(b.y - position.y)
                    - f32::from(a.y - position.y) * f32::from(b.x - position.x)
            };
            let signs = [
                cross(vertices[0].xy_position, vertices[1].xy_position),
                cross(vertices[1].xy_position, vertices[2].xy_position),
                cross(vertices[2].xy_position, vertices[0].xy_position),
            ];
            signs.iter().all(|sign| *sign >= -0.001) || signs.iter().all(|sign| *sign <= 0.001)
        })
    }

    #[test]
    fn masks_preserve_safe_centre_and_original_aspect_threshold() {
        assert_eq!(margins(1.75), [(0.067, 0.05), (0.033, 0.035)]);
        assert_eq!(margins(16. / 9.), [(0.1, 0.05), (0.035, 0.035)]);
        for aspect in [0.5, 4. / 3., 16. / 9., 2.35] {
            let bounds = Bounds::new(point(px(31.), px(47.)), size(px(720. * aspect), px(720.)));
            for (horizontal, vertical) in margins(aspect) {
                let mask = mask_path(bounds, horizontal, vertical).unwrap();
                assert!(!contains(&mask, bounds.center()));
                assert!(contains(&mask, bounds.origin + point(px(1.), px(1.))));
                let image_area = f32::from(bounds.size.width) * f32::from(bounds.size.height);
                let rectangular_ring =
                    image_area * (1. - (1. - 2. * horizontal) * (1. - 2. * vertical));
                assert!(area(&mask) > rectangular_ring);
                assert!(area(&mask) < image_area * 0.5);
            }
        }
    }

    #[test]
    fn masks_follow_zoom_and_pan_without_filling_the_safe_region() {
        let base = Bounds::new(point(px(100.), px(200.)), size(px(640.), px(360.)));
        let delta = point(px(24.), px(-40.));
        for zoom in [0.125, 0.5, 1., 2., 10.] {
            let bounds = crate::visual_panel::zoomed_video_bounds(base, zoom, delta);
            for (horizontal, vertical) in margins(16. / 9.) {
                let path = mask_path(bounds, horizontal, vertical).unwrap();
                assert!(!contains(&path, bounds.center()));
                assert!(!contains(&path, bounds.origin - point(px(1.), px(1.))));
                assert!(contains(&path, bounds.origin + point(px(1.), px(1.))));
                let unscaled = mask_path(base, horizontal, vertical).unwrap();
                let normalized_area = area(&path) / (zoom * zoom);
                assert!((normalized_area - area(&unscaled)).abs() < 100.);
            }
        }
    }
}
