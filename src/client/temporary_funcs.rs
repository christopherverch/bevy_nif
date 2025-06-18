use std::{f32::consts::PI, time::Instant};

use bevy::prelude::*;
use cdda_bevy::client::defs::Pause;

use crate::setup_funcs::HudText;
// pauses and unpauses the light rotating when we press space
pub fn switch_mode(
    mut query: Query<&mut Text, With<HudText>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<Pause>,
) {
    let mut text = match query.single_mut() {
        Ok(t) => t,
        Err(e) => {
            match e {
                bevy::ecs::query::QuerySingleError::NoEntities(_) => {
                    eprintln!("Warning: No UI text entity found with the HudText component.");
                }
                bevy::ecs::query::QuerySingleError::MultipleEntities(_) => {
                    eprintln!(
                        "Warning: Multiple UI text entities found with the HudText component. Expected only one."
                    );
                }
            }
            return; // Return early if there's an error
        }
    };
    text.clear();

    if keys.just_pressed(KeyCode::Space) {
        pause.0 = !pause.0;
    }
}
pub fn animate_light_direction(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<DirectionalLight>>,
    pause: Res<Pause>,
    mut frames_passed: Local<u32>,
    mut last_instant: Local<Option<Instant>>,
) {
    *frames_passed += 1;
    if *frames_passed == 60 {
        //let framerate = Instant::now() - *last_instant.get_or_insert(Instant::now());
        // println!("framerate: {}", 60000.0 / framerate.as_millis() as f32);
        *last_instant = Some(Instant::now());
        *frames_passed = 0;
    }
    if pause.0 {
        return;
    }
    for mut transform in &mut query {
        transform.rotate_y(time.delta_secs() * PI / 5.0);
    }
}
