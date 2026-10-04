use bevy::ecs::prelude::*;
use bevy::ui::{Node, Val};
use bevy::ui_widgets::{SliderRange, SliderThumb, SliderValue};
use superui_css::SliderPart;

/// Position the thumb and size the fill of every `bevy_ui_widgets` slider from
/// its `SliderValue`/`SliderRange`, driving the ordinary `Node.left`/`Node.width`
/// cascade properties. Runs when the value or range changes.
///
/// Percent-based and pure: does not read `ComputedNode`, so it does not
/// compensate for thumb size (the thumb travels the full 0-100% track rather
/// than being inset by half its width at the ends). That compensation is a
/// documented follow-up.
pub fn position_slider_parts(
    sliders: Query<
        (&SliderValue, &SliderRange, &Children),
        Or<(Changed<SliderValue>, Changed<SliderRange>)>,
    >,
    thumbs: Query<(), With<SliderThumb>>,
    parts: Query<&SliderPart>,
    mut nodes: Query<&mut Node>,
) {
    for (value, range, children) in &sliders {
        let pct = range.thumb_position(value.0).clamp(0.0, 1.0) * 100.0;
        for child in children.iter() {
            if thumbs.get(child).is_ok() {
                if let Ok(mut n) = nodes.get_mut(child) {
                    n.left = Val::Percent(pct);
                }
            }
            if matches!(parts.get(child), Ok(SliderPart::Fill)) {
                if let Ok(mut n) = nodes.get_mut(child) {
                    n.width = Val::Percent(pct);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::prelude::*;
    use bevy::ecs::prelude::Entity;
    use bevy::ui::{Node, Val};
    use bevy::ui_widgets::{SliderRange, SliderThumb, SliderValue};
    use superui_css::{SliderPart, StyleData};

    fn val(app: &App, e: Entity) -> Val {
        app.world().get::<Node>(e).unwrap().left
    }
    fn width(app: &App, e: Entity) -> Val {
        app.world().get::<Node>(e).unwrap().width
    }

    #[test]
    fn positions_thumb_and_fill_at_value_fraction() {
        let mut app = App::new();
        app.add_systems(Update, position_slider_parts);
        let thumb = app.world_mut().spawn((Node::default(), SliderThumb)).id();
        let fill = app
            .world_mut()
            .spawn((Node::default(), SliderPart::Fill, StyleData::default()))
            .id();
        let host = app
            .world_mut()
            .spawn((
                Node::default(),
                SliderValue(25.0),
                SliderRange::new(0.0, 100.0),
            ))
            .add_children(&[thumb, fill])
            .id();
        let _ = host;
        app.update();
        assert_eq!(val(&app, thumb), Val::Percent(25.0));
        assert_eq!(width(&app, fill), Val::Percent(25.0));
    }

    #[test]
    fn positioning_wins_over_prior_left_value() {
        let mut app = App::new();
        app.add_systems(Update, position_slider_parts);
        let thumb = app
            .world_mut()
            .spawn((
                Node {
                    left: Val::Percent(0.0),
                    ..Node::default()
                },
                SliderThumb,
            ))
            .id();
        app.world_mut()
            .spawn((Node::default(), SliderValue(80.0), SliderRange::new(0.0, 100.0)))
            .add_child(thumb);
        app.update();
        assert_eq!(app.world().get::<Node>(thumb).unwrap().left, Val::Percent(80.0));
    }

    #[test]
    fn zero_span_does_not_nan() {
        let mut app = App::new();
        app.add_systems(Update, position_slider_parts);
        let thumb = app.world_mut().spawn((Node::default(), SliderThumb)).id();
        app.world_mut()
            .spawn((
                Node::default(),
                SliderValue(5.0),
                SliderRange::new(10.0, 10.0),
            ))
            .add_child(thumb);
        app.update();
        // thumb_position returns 0.5 for a zero span; must be finite.
        if let Val::Percent(p) = val(&app, thumb) {
            assert!(p.is_finite());
        } else {
            panic!()
        }
    }
}
