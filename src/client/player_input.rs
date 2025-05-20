use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::basic;
use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy_rapier3d::prelude::*;
use cdda_bevy::client::animation::trigger_request_animation;
use cdda_bevy::client::defs::{
    AttackState, CreatureStats, Direction, IsThirdPerson, LocalPlayer, LocalPlayerMesh,
    MOVEMENT_VOLUME, PlayerInput, PlayerState, Speed,
};
use cdda_bevy::client::events::{
    AttackTrigger, KnockdownFinished, RequestAnimation, TakeHit, ToggledThirdPerson,
};
use cdda_bevy::{AnimationState, RequestedAnimation};
use std::time::Instant;

use crate::wav_loader::MorrowindDecodedWav;
const GROUND_TIMER: f32 = 0.5;
const JUMP_SPEED: f32 = 0.06;
const GRAVITY: f32 = -0.04;
pub fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut player_input: ResMut<PlayerInput>,
    mut is_third_person: ResMut<IsThirdPerson>,
    mut commands: Commands,
    mut player_state_query: Query<(Entity, &mut PlayerState), With<LocalPlayer>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
) {
    let Some((player_entity, mut player_state)) = player_state_query.iter_mut().next() else {
        return; // Exit early if no player is found
    };
    if player_state.current_animation == AnimationState::Knockdown {
        return;
    }
    let mut first_person_entities = Vec::new();
    first_person_entities.push(player_entity);
    let mut move_direction = Vec2::ZERO;

    if keyboard_input.pressed(KeyCode::KeyW) {
        move_direction.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        move_direction.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        move_direction.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        move_direction.x += 1.0;
    }
    if keyboard_input.just_pressed(KeyCode::ControlLeft) {
        commands.trigger(RequestAnimation {
            entity: player_entity,
            requested_animation: AnimationState::SneakIdle.into(),
        });
    }
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        player_state.is_ready_weapon = !player_state.is_ready_weapon;
        let requested_anim: RequestedAnimation;
        if player_state.is_ready_weapon {
            player_state.attack_state = AttackState::Ready;
            requested_anim = RequestedAnimation::SwingReady;
        } else {
            player_state.attack_state = AttackState::Inactive;
            requested_anim = RequestedAnimation::SwingInactive;
        }
        hide_player_conditionally(
            &first_person_entities,
            is_third_person.0,
            //   &player_state,
            // &mesh_query,
            commands.reborrow(),
        );
        trigger_request_animation(
            commands.reborrow(),
            player_entity,
            requested_anim,
            &mut player_state,
        );
    }
    if keyboard_input.just_pressed(KeyCode::Tab) {
        is_third_person.0 = !is_third_person.0;
        commands.trigger(ToggledThirdPerson {
            entity: player_entity,
            is_third_person: is_third_person.0,
        });
        hide_player_conditionally(
            &first_person_entities,
            is_third_person.0,
            //&player_state,
            //&mesh_query,
            commands.reborrow(),
        );
    }
    player_input.move_direction = move_direction.normalize_or_zero();
    if !player_input.jump {
        player_input.jump = keyboard_input.just_pressed(KeyCode::Space);
    }
    if buttons.just_pressed(MouseButton::Left) {
        if player_state.attack_state == AttackState::Ready {
            player_state.started_attacking = Instant::now();
            player_state.attack_state = AttackState::Charging;
            trigger_request_animation(
                commands.reborrow(),
                player_entity,
                RequestedAnimation::SwingCharging,
                &mut player_state,
            );
        }
    }
    if buttons.pressed(MouseButton::Left) {}
    if player_state.attack_state == AttackState::Releasing
        && player_state.started_attacking.elapsed().as_millis() >= 300
    {
        player_state.attack_state = AttackState::Ready;
    }
    if player_state.attack_state == AttackState::ChargeImmediateRelease {
        if player_state.started_attacking.elapsed().as_millis() >= 300 {
            player_state.attack_state = AttackState::Charging;
            for (_camera, camera_transform) in &cameras {
                let forward_vector = camera_transform.forward();
                let ray_direction = forward_vector.normalize();
                let ray_origin = camera_transform.translation();
                player_state.started_attacking = Instant::now();
                player_state.attack_state = AttackState::Releasing;
                let damage = 5.0
                    + 10.0
                        * ((player_state.started_attacking.elapsed().as_millis() / 1200) as f32)
                            .min(1.0);
                commands.trigger(AttackTrigger {
                    entity: player_entity,
                    origin: ray_origin,
                    direction: ray_direction,
                    damage,
                });
            }
        }
    } else {
        if buttons.just_released(MouseButton::Left) {
            if player_state.attack_state == AttackState::Charging
                && player_state.started_attacking.elapsed().as_millis() < 300
            {
                //if we released it faster than the minimum attack delay
                player_state.attack_state = AttackState::ChargeImmediateRelease;
            } else if player_state.attack_state == AttackState::Charging {
                for (_camera, camera_transform) in &cameras {
                    let forward_vector = camera_transform.forward();
                    let ray_direction = forward_vector.normalize();
                    let ray_origin = camera_transform.translation();
                    player_state.attack_state = AttackState::Releasing;
                    let damage = 5.0
                        + 10.0
                            * (((player_state.started_attacking.elapsed().as_millis() / 100)
                                as f32)
                                / 12.0)
                                .min(1.0);
                    commands.trigger(AttackTrigger {
                        entity: player_entity,
                        origin: ray_origin,
                        direction: ray_direction,
                        damage,
                    });
                    player_state.started_attacking = Instant::now();
                }
            }
        }
    }
}
pub fn player_movement(
    mut player_input: ResMut<PlayerInput>,
    time: Res<Time<Fixed>>,
    mut player_query: Query<
        (
            Entity,
            &mut Transform,
            &mut KinematicCharacterController,
            Option<&KinematicCharacterControllerOutput>,
            &mut PlayerState,
            &Speed,
        ),
        (With<super::LocalPlayer>, Without<LocalPlayerMesh>),
    >,
    camera_query: Query<
        &Transform,
        (
            With<Camera3d>,
            Without<super::LocalPlayer>,
            Without<LocalPlayerMesh>,
        ),
    >,
    mut commands: Commands,
    mut vertical_movement: Local<f32>,
    mut grounded_timer: Local<f32>,
    mut last_ground_height: Local<f32>,
    //   rapier_context: ReadRapierContext,
    mut highest_air_height: Local<f32>,
    asset_server: Res<AssetServer>,
) {
    for (
        player_entity,
        mut player_transform,
        mut controller,
        output,
        mut player_state,
        player_speed,
    ) in player_query.iter_mut()
    {
        if player_state.current_animation == AnimationState::Knockdown {
            return;
        }
        player_state.transform = *player_transform;
        //handle animations
        //if we're on the ground
        if output.map(|o| o.grounded).unwrap_or(false) {
            //if we just transitioned to idle:
            if player_input.move_direction.x == 0.0 && player_input.move_direction.y == 0.0 {
                player_state.is_moving = false;
                if player_state.current_animation != AnimationState::Idle
                    && player_state.current_animation != AnimationState::Undefined
                {
                    player_state.current_animation = AnimationState::Idle;
                    trigger_request_animation(
                        commands.reborrow(),
                        player_entity,
                        AnimationState::Idle.into(),
                        &mut player_state,
                    );
                }
            } else {
                player_state.is_moving = true;
                //if we just transitioned to running:
                let mut curr_direction = Direction::Forward;
                if player_input.move_direction.x < 0.0 && player_input.move_direction.y == 0.0 {
                    curr_direction = Direction::Left;
                } else if player_input.move_direction.x > 0.0
                    && player_input.move_direction.y == 0.0
                {
                    curr_direction = Direction::Right;
                } else if player_input.move_direction.y < 0.0 {
                    curr_direction = Direction::Backward;
                }
                match player_state.current_animation {
                    AnimationState::Running { direction } => {
                        if direction != curr_direction {
                            let requested_animation = AnimationState::Running {
                                direction: curr_direction,
                            };
                            trigger_request_animation(
                                commands.reborrow(),
                                player_entity,
                                requested_animation.into(),
                                &mut player_state,
                            );
                        }
                    }
                    _ => {
                        let requested_animation = AnimationState::Running {
                            direction: curr_direction,
                        };
                        trigger_request_animation(
                            commands.reborrow(),
                            player_entity,
                            requested_animation.into(),
                            &mut player_state,
                        );
                    }
                }

                player_state.current_animation = AnimationState::Running {
                    direction: curr_direction,
                };
            }
        }
        if let Ok(camera_transform) = camera_query.single() {
            let delta_time = time.delta_secs();
            let forward = camera_transform.forward().normalize();
            let right = camera_transform.right().normalize();

            // Project input onto the ground plane (remove y component)
            let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

            // Calculate movement vector relative to camera
            let mut direction = (forward_flat * player_input.move_direction.y
                + right_flat * player_input.move_direction.x)
                * 5.0
                * delta_time;
            direction.y = 0.0;
            let mut movement = direction.normalize_or_zero() * player_speed.0 / 100.0;
            let jump_speed = if player_input.jump { 1.0 } else { 0.0 } * JUMP_SPEED;
            //clear input
            player_input.jump = false;
            *vertical_movement += GRAVITY * delta_time * controller.custom_mass.unwrap_or(1.0);
            let was_grounded = *grounded_timer > 0.0;
            if output.map(|o| o.grounded).unwrap_or(false) {
                *grounded_timer = GROUND_TIMER;
                *vertical_movement = 0.0;
                if !was_grounded {
                    let sound_handle: Handle<MorrowindDecodedWav> =
                        asset_server.load("sounds/bodyfallMED.wav");

                    commands.spawn((
                        AudioPlayer(sound_handle),
                        PlaybackSettings {
                            volume: MOVEMENT_VOLUME,
                            ..default()
                        },
                    ));
                    commands.trigger(RequestAnimation {
                        entity: player_entity,
                        requested_animation: RequestedAnimation::ResumeAnimation,
                    });
                }

                // Spawn the new sound instance with the marker
            }
            // If we are grounded we can jump
            if *grounded_timer > 0.0 {
                *grounded_timer -= delta_time;
                //record our height when we landed so we know if we should take fall damage
                *last_ground_height = player_transform.translation.y;
                // If we jump we clear the grounded tolerance
                if jump_speed > 0.0 {
                    trigger_request_animation(
                        commands.reborrow(),
                        player_entity,
                        AnimationState::Jumping.into(),
                        &mut player_state,
                    );
                    *vertical_movement = jump_speed;
                    *grounded_timer = 0.0;
                }
            }
            movement.y = *vertical_movement;
            if *vertical_movement < -0.4 {
                *vertical_movement = -0.4;
            }

            controller.translation = Some(movement);
            player_state.transform.translation = player_transform.translation;

            let camera_forward = camera_transform.forward().normalize();

            // Project the camera's forward vector onto the ground plane (remove y component)
            let target_forward =
                Vec3::new(-camera_forward.x, 0.0, -camera_forward.z).normalize_or_zero();
            let target_rotation = Quat::from_rotation_arc(Vec3::NEG_Z, target_forward);
            player_transform.rotation = target_rotation;

            if *highest_air_height < player_transform.translation.y {
                *highest_air_height = player_transform.translation.y;
            }
            //println!("Highest air height: {:.4}", *highest_air_height); // Printing with 4 decimal places

            if *highest_air_height < player_transform.translation.y {
                *highest_air_height = player_transform.translation.y;
            }
            //println!("Highest air height: {:.4}", *highest_air_height); // Printing with 4 decimal places
        }
    }
}
fn hide_player_conditionally(
    first_person_entities: &Vec<Entity>,
    is_third_person: bool,
    //player_state: &PlayerState,
    //  mesh_query: &Query<Entity, With<LocalPlayerMesh>>,
    mut commands: Commands,
) {
    //TEMPORARY
    for entity in first_person_entities {
        if !is_third_person {
            commands.entity(*entity).insert(Visibility::Hidden);
        } else {
            commands.entity(*entity).insert(Visibility::Visible);
        }
    }
    ////////
    /*if let Some(torso_entity) = mesh_query.iter().next() {
        let (first_person_visibility, torso_visibility) = if is_third_person {
            (Visibility::Visible, Visibility::Visible)
        } else if player_state.is_ready_weapon {
            (Visibility::Visible, Visibility::Hidden)
        } else {
            (Visibility::Hidden, Visibility::Hidden)
        };

        for entity in first_person_entities {
            commands.entity(*entity).insert(first_person_visibility);
        }

        commands.entity(torso_entity).insert(torso_visibility);
    }*/
}
pub fn trigger_attack(
    trigger: Trigger<AttackTrigger>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rapier_context: ReadRapierContext,
    creature_q: Query<&CreatureStats>,
) {
    println!(
        "drawing ray from: {} to {}",
        trigger.origin, trigger.direction
    );
    let mesh = create_line_mesh(trigger.origin, trigger.origin + (trigger.direction * 2.0));
    let sphere_color = Color::srgb(10.0, 4.0, 1.0);
    // Emissive sphere
    let unlit_mat: StandardMaterial = sphere_color.into();
    let material_h = materials.add(unlit_mat);
    let mesh_handle = meshes.add(mesh);
    commands.spawn((Mesh3d(mesh_handle), MeshMaterial3d(material_h)));
    let Ok(context) = rapier_context.single() else {
        return;
    };

    println!("ray_direction: {}", trigger.direction);
    let hit = context.cast_ray(
        trigger.origin,
        trigger.direction.into(),
        2.0,
        true,
        QueryFilter {
            exclude_collider: Some(trigger.entity),
            ..default()
        },
    );
    if let Some((entity, _toi)) = hit {
        println!("hit entity: {}", entity);
        if let Ok(creature) = creature_q.get(entity) {
            commands.trigger_targets(
                TakeHit {
                    creature_id: creature.id,
                    damage: trigger.damage,
                    is_from_network: false,
                },
                entity,
            );
        }
        // Color in blue the entity we just hit.
        // Because of the query filter, only colliders attached to a dynamic body
        // will get an event.
        let color = basic::BLUE.into();
        commands.entity(entity).insert(ColliderDebugColor(color));
    }
}
fn create_line_mesh(start: Vec3, end: Vec3) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::LineList, // Or LineStrip if you have connected lines
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![start.to_array(), end.to_array()],
    );
    // For LineList, each pair of vertices is a separate line.
    // If you were using LineStrip, it would connect them in order.
    mesh
}
pub fn knockdown_finished(
    trigger: Trigger<KnockdownFinished>,
    mut player_state_query: Query<(Entity, &mut PlayerState), With<LocalPlayer>>,
    mut commands: Commands,
) {
    if let Ok((player_entity, mut player_state)) = player_state_query.get_mut(trigger.0) {
        player_state.current_animation = AnimationState::Idle;
        trigger_request_animation(
            commands.reborrow(),
            player_entity,
            AnimationState::Idle.into(),
            &mut player_state,
        );
    }
    println!("knockdown finished!");
}

/*fn trigger_attack(player_entity: Entity, player_state: &mut PlayerState, mut commands: Commands) {
    if player_state.attack_state == AttackState::Charging {
        player_state.attack_state = AttackState::Releasing;
        trigger_request_animation(
            commands.reborrow(),
            player_entity,
            RequestedAnimation::SwingReleasing,
            player_state,
        );
        println!("attack!");
    }
}*/
