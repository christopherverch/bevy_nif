use bevy::color::palettes::basic::*;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use tes3mp_rust::defs::{GameState, ServerIP};
#[derive(Resource, Default)]
struct FocusedIpInput {
    entity: Option<Entity>, // Use Option<Entity>
}

#[derive(Component)]
pub struct IpInputField;
#[derive(Component)]
pub struct MainMenuRoot;
#[derive(Component)]
struct ConnectButton;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FocusedIpInput::default())
            .add_systems(OnEnter(GameState::MainMenu), set_initial_focus)
            .add_systems(Update, button_system.run_if(in_state(GameState::MainMenu)))
            .add_systems(
                Update,
                listen_keyboard_input_events.run_if(in_state(GameState::MainMenu)),
            );
    }
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
                            font: font_handle.clone(),
                            font_size: 30.0,
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
                                    font: font_handle.clone(),
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::BLACK),
                                TextLayout::default(),
                                IpInputField,
                            ));
                        })
                        .insert(Interaction::default())
                        .insert(FocusPolicy::Pass); // Make it focusable

                    // Connect button
                    parent
                        .spawn((
                            Button,
                            Node {
                                // The main button Node
                                width: Val::Px(150.0),
                                height: Val::Px(65.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(5.0)), // The Border component
                                ..default()
                            },
                            BorderColor::all(Color::BLACK),
                            BackgroundColor(GREEN.into()), // Assuming NORMAL_BUTTON is Color::GREEN
                            BorderRadius::MAX,
                            ConnectButton,
                            Interaction::default(),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                Text::new("Connect"),
                                TextFont {
                                    font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                                    font_size: 33.0,
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
fn set_initial_focus(
    focused_input: Res<FocusedIpInput>,
    mut interaction_query: Query<&mut Interaction>,
) {
    if let Some(entity) = focused_input.entity {
        // Directly access the Option<Entity>
        if let Ok(mut interaction) = interaction_query.get_mut(entity) {
            *interaction = Interaction::Pressed; // Simulate a click to focus.
        }
    }
}
const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);
fn button_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &Children,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut Text, Without<IpInputField>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut server_addr: ResMut<ServerIP>,
    ip_input_field: Single<&mut Text, With<IpInputField>>,
) {
    for (interaction, mut color, mut border_color, children) in &mut interaction_query {
        let mut text = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                **text = "Press".to_string();
                *color = PRESSED_BUTTON.into();
                *border_color = BorderColor::all(RED);
                game_state.set(GameState::Connecting);
                server_addr.0 = ip_input_field.0.clone();
                println!("ip: {}", server_addr.0);
            }
            Interaction::Hovered => {
                **text = "Hover".to_string();
                *color = HOVERED_BUTTON.into();
                *border_color = BorderColor::all(Color::WHITE);
            }
            Interaction::None => {
                **text = "Button".to_string();
                *color = NORMAL_BUTTON.into();
                *border_color = BorderColor::all(Color::BLACK);
            }
        }
    }
}
fn listen_keyboard_input_events(
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
                    text.push_str(character);
                }
                _ => continue,
            }
        }
    }
}
