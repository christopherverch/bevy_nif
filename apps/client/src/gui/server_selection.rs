use bevy::{
    color::palettes::css::{GRAY, GREEN, WHITE},
    input::keyboard::{Key, KeyboardInput},
    picking::hover::Hovered,
    prelude::*,
    ui::FocusPolicy,
    ui_widgets::Activate,
};

use crate::networking::ServerIP;
use common::state::GameState;

#[derive(Component)]
pub struct IpInputField;
#[derive(Component)]
pub struct MainMenuRoot;
#[derive(Component)]
struct ConnectButton;
#[derive(Resource, Default)]
pub struct FocusedIpInput {
    entity: Option<Entity>, // Use Option<Entity>
}
pub fn setup_main_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    println!("spawning menu");
    commands
        .spawn((
            Visibility::Visible,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            MainMenuRoot,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(GRAY.into()), // Assuming NORMAL_BUTTON is Color::GREEN
                ))
                .with_children(|parent| {
                    // Text label
                    let font_handle = asset_server.load("fonts/FiraMono-Medium.ttf");
                    parent.spawn((
                        Text::new("Enter Server IP:"),
                        TextFont {
                            font: FontSource::Handle(font_handle.clone()),
                            font_size: px(30.0).into(),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextLayout::default(), // Default justification
                    ));

                    // Input field
                    parent
                        .spawn((
                            Node {
                                width: Val::Px(200.0),
                                height: Val::Px(30.0),
                                margin: UiRect::bottom(Val::Px(10.0)),
                                padding: UiRect::all(Val::Px(5.0)),
                                ..default()
                            },
                            BackgroundColor(WHITE.into()), // Assuming NORMAL_BUTTON is Color::GREEN
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                Text::new(""),
                                TextFont {
                                    font: FontSource::Handle(font_handle.clone()),
                                    font_size: px(20.0).into(),
                                    ..default()
                                },
                                TextColor(Color::BLACK),
                                TextLayout::default(),
                                IpInputField,
                            ));
                        })
                        .insert(Hovered::default())
                        .insert(FocusPolicy::Pass);

                    // Connect button
                    parent
                        .spawn((
                            bevy::ui_widgets::Button,
                            Node {
                                // The main button Node
                                width: Val::Px(150.0),
                                height: Val::Px(65.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(5.0)), // The Border component
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BorderColor::all(Color::BLACK),
                            BackgroundColor(GREEN.into()), // Assuming NORMAL_BUTTON is Color::GREEN
                            ConnectButton,
                            Hovered::default(),
                        ))
                        .observe(on_pressed)
                        .with_children(|parent| {
                            parent.spawn((
                                Text::new("Connect"),
                                TextFont {
                                    font: FontSource::Handle(
                                        asset_server.load("fonts/FiraMono-Medium.ttf"),
                                    ),
                                    font_size: px(33.0).into(),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                                TextLayout::default(),
                            ));
                        });
                });
        });
}
pub fn despawn_main_menu_ui(mut commands: Commands, mut query: Query<Entity, With<MainMenuRoot>>) {
    for entity in &mut query {
        commands.entity(entity).despawn();
    }
}
pub fn set_initial_focus(
    focused_input: Res<FocusedIpInput>,
    mut commands: Commands,
    window: Single<Entity, With<Window>>,
) {
    if let Some(entity) = focused_input.entity {
        commands.trigger(bevy::input_focus::AcquireFocus {
            focused_entity: entity,
            window: *window,
        });
    }
}
const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);
pub fn button_system(
    mut interaction_query: Query<
        (&Hovered, &mut BackgroundColor, &mut BorderColor, &Children),
        (Changed<Hovered>, With<bevy::ui_widgets::Button>),
    >,
    mut text_query: Query<&mut Text, Without<IpInputField>>,
) {
    for (hovered, mut color, mut border_color, children) in &mut interaction_query {
        let mut text = text_query.get_mut(children[0]).unwrap();

        if hovered.0 {
            **text = "Hover".to_string();
            *color = HOVERED_BUTTON.into();
            *border_color = BorderColor::all(Color::WHITE);
        } else {
            **text = "Button".to_string();
            *color = NORMAL_BUTTON.into();
            *border_color = BorderColor::all(Color::BLACK);
        }
    }
}
fn on_pressed(
    _ev: On<Activate>,
    ip_input_field: Single<&mut Text, With<IpInputField>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut server_addr: ResMut<ServerIP>,
) {
    game_state.set(GameState::Connecting);
    server_addr.0 = ip_input_field.0.clone();
    println!("ip: {}", server_addr.0);
}
pub fn listen_keyboard_input_events(
    mut events: MessageReader<KeyboardInput>,
    text_query: Single<&mut Text, With<IpInputField>>,
    focused_input: Option<Res<FocusedIpInput>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut server_addr: ResMut<ServerIP>,
) {
    if focused_input.is_some() {
        let mut text = text_query.into_inner();
        for event in events.read() {
            // Only trigger changes when the key is first pressed.
            if !event.state.is_pressed() {
                continue;
            }

            match &event.logical_key {
                Key::Enter => {
                    if text.is_empty() {
                        server_addr.0 = "".to_string();
                        game_state.set(GameState::Connecting);
                        println!("ip: {}", server_addr.0);
                        return;
                        //   let old_value = mem::take(&mut **text);
                    }
                    match text.0.as_str() {
                        "" => {
                            server_addr.0 = text.0.clone();
                        }
                        "1" => {
                            server_addr.0 = "127.0.0.1:5000".to_string();
                        }
                        _ => {
                            server_addr.0 = "107.136.161.231:5000".to_string();
                        }
                    }
                    game_state.set(GameState::Connecting);
                    println!("server addr: {}", server_addr.0);
                }
                Key::Space => {
                    text.push(' ');
                }
                Key::Backspace => {
                    text.pop();
                }
                Key::Character(character) => {
                    text.push_str(&character);
                }
                _ => continue,
            }
        }
    }
}
