use bevy::prelude::*;
const DEFAULT_BORDER_SIZE: f32 = 2.0;
const DEFAULT_MARGIN_SIZE: f32 = 5.0;
pub fn generic_button<B: Bundle>(marker_bundle: B) -> impl Bundle {
    (
        Button,
        Pickable::default(),
        Node {
            min_width: Val::Px(64.0),
            min_height: Val::Px(64.0),
            max_height: Val::Px(64.0),
            max_width: Val::Px(64.0),
            width: Val::Px(64.0),
            height: Val::Px(64.0),
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            flex_shrink: 0.0,
            border: UiRect::all(Val::Px(DEFAULT_BORDER_SIZE)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            margin: UiRect::all(Val::Px(DEFAULT_MARGIN_SIZE)),
            position_type: PositionType::Relative,
            ..default()
        },
        BorderColor::all(Color::WHITE),
        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)), // Add background for better visibility
        marker_bundle,
    )
}
