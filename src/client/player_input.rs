use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::basic;
use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy_nif::nif::animation::BlendMask;
use bevy_rapier3d::prelude::*;
use cdda_bevy::client::defs::{
    AttackState, CreatureStats, Direction, IsThirdPerson, LocalPlayer, LocalPlayerMesh,
    LocomotionState, MESH_HEIGHT_OFFSET, PlayerInput, PlayerState, Speed,
};
use cdda_bevy::client::events::{
    KnockdownFinished, RequestAnimation, SetAnimSpeedSeekEvent, SoundEvent, ToggledThirdPerson,
    TriggerHit, TryAttackTrigger,
};
use cdda_bevy::{AnimationConfig, NetworkEvent, RepeatAnim};
use std::time::Instant;

const GROUND_TIMER: f32 = 0.5;
const JUMP_SPEED: f32 = 0.06;
const GRAVITY: f32 = -0.04;
pub fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut player_input: ResMut<PlayerInput>,
    mut is_third_person: ResMut<IsThirdPerson>,
    mut commands: Commands,
    mut player_state_query: Query<
        (Entity, &mut PlayerState, &mut CreatureStats),
        With<LocalPlayer>,
    >,
    cameras: Query<(&Camera, &Transform)>,
) {
    let Some((player_entity, mut player_state, mut creature_stats)) =
        player_state_query.iter_mut().next()
    else {
        return; // Exit early if no player is found
    };
    if !creature_stats.input_enabled {
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
    player_input.move_direction = move_direction.normalize_or_zero();
    if keyboard_input.just_pressed(KeyCode::ControlLeft) {
        commands.trigger(RequestAnimation {
            skeleton_id: player_state.id,
            animation_config: AnimationConfig {
                name: "idlesneak".to_string(),
                repeat: RepeatAnim::Forever,
                priority: 0,
                mask: BlendMask::all(),
            },
            networked: true,
        });
    }
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        if player_state.attack_state == AttackState::Ready {
            creature_stats.current_weapon_state = "".to_string();
            player_state.attack_state = AttackState::Inactive;
        } else {
            creature_stats.current_weapon_state = "hh".to_string();
            player_state.attack_state = AttackState::Ready;
        }
        commands.trigger(RequestAnimation {
            skeleton_id: player_state.id,
            animation_config: AnimationConfig {
                name: get_movement_animation_from_locomotive_state(
                    &creature_stats.current_locomotion_state,
                    &creature_stats.current_weapon_state,
                ),
                repeat: RepeatAnim::Forever,
                priority: 0,
                mask: BlendMask::all(),
            },
            networked: true,
        });
        hide_player_conditionally(
            &first_person_entities,
            is_third_person.0,
            //   &player_state,
            // &mesh_query,
            commands.reborrow(),
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
    // do this since physics runs at 60fps but player input checks run faster
    if !player_input.jump {
        player_input.jump = keyboard_input.just_pressed(KeyCode::Space);
    }
    handle_player_attack_system(
        buttons,
        &player_entity,
        &mut player_state,
        &creature_stats,
        cameras,
        commands,
    );
}
fn handle_player_attack_system(
    buttons: Res<ButtonInput<MouseButton>>, // Reads mouse input directly
    player_entity: &Entity,
    player_state: &mut PlayerState,
    creature_stats: &CreatureStats,
    camera_query: Query<(&Camera, &Transform)>, // For raycasting
    mut commands: Commands,
    // time: Res<Time>, // If you need delta_time for anything not covered by Instant
) {
    if !creature_stats.input_enabled {
        // If an attack was in progress, you might want to reset it
        if player_state.attack_state != AttackState::Ready {
            player_state.attack_state = AttackState::Ready;
        }
        return;
    }

    if buttons.just_pressed(MouseButton::Left) {
        if player_state.attack_state == AttackState::Ready {
            player_state.started_attacking = Instant::now();
            player_state.attack_state = AttackState::Charging;
            // Consider triggering an "AttackChargeStart" animation/sound event here
        }
    }

    // Logic for cooldown after releasing an attack
    if player_state.attack_state == AttackState::Releasing
        && player_state.started_attacking.elapsed().as_millis() >= 300
    {
        //
        player_state.attack_state = AttackState::Ready;
    }

    // Logic for ChargeImmediateRelease
    if player_state.attack_state == AttackState::ChargeImmediateRelease {
        // Min charge time
        if player_state.started_attacking.elapsed().as_millis() >= 300 {
            for (_camera, camera_transform) in &camera_query {
                execute_attack(
                    *player_entity,
                    player_state,
                    creature_stats,
                    camera_transform,
                    &mut commands,
                );
            }
        }
    } else {
        // This 'else' ensures this block doesn't run if we just transitioned ChargeImmediateRelease -> Charging
        if buttons.just_released(MouseButton::Left) {
            if player_state.attack_state == AttackState::Charging {
                if player_state.started_attacking.elapsed().as_millis() < 300 {
                    // Quick tap resulted in release
                    player_state.attack_state = AttackState::ChargeImmediateRelease;
                } else {
                    // Normal release after proper charge
                    if let Ok((_camera, camera_transform)) = camera_query.single() {
                        execute_attack(
                            *player_entity,
                            player_state,
                            creature_stats,
                            camera_transform,
                            &mut commands,
                        );
                        player_state.started_attacking = Instant::now(); // Reset for cooldown timing 
                        // Consider triggering an "AttackRelease" animation/sound event here
                    }
                }
            }
        }
    }
}
fn execute_attack(
    attacker_entity: Entity,
    player_state: &mut PlayerState, // Needs mutable access to update state and timer
    creature_stats: &CreatureStats,
    camera_transform: &Transform, // To get attack direction
    commands: &mut Commands,
) {
    let forward_vector = camera_transform.forward();
    let ray_direction = forward_vector.normalize();
    // Ray origin from player's current transform
    let ray_origin =
        player_state.transform.translation + Vec3::new(0.0, MESH_HEIGHT_OFFSET + 0.3, 0.0);

    player_state.attack_state = AttackState::Releasing;
    let charge_duration_ms = player_state.started_attacking.elapsed().as_millis();
    // Calculate damage based on the charge duration
    let damage = 5.0 + 10.0 * (charge_duration_ms as f32 / 1200.0).min(1.0);

    commands.trigger(TryAttackTrigger {
        attacker_entity,
        attacker_id: player_state.id, // Make sure PlayerState has `id`
        origin: ray_origin,
        direction: ray_direction,
        damage,
    });
    commands.trigger(RequestAnimation {
        skeleton_id: player_state.id,
        animation_config: AnimationConfig {
            name: "hit1".to_string(),
            repeat: RepeatAnim::Never,
            priority: 0,
            mask: BlendMask::UPPER_BODY,
        },
        networked: true,
    });

    // Reset the attack timer AFTER using its elapsed value for damage calculation
    player_state.started_attacking = Instant::now();
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
            &mut CreatureStats,
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
) {
    for (
        player_entity,
        mut player_transform,
        mut controller,
        output,
        mut player_state,
        mut creature_stats,
        player_speed,
    ) in player_query.iter_mut()
    {
        let original_locomotion_state = &creature_stats.current_locomotion_state; // For change detection
        let prev_state_was_airborne = matches!(
            original_locomotion_state,
            LocomotionState::Jumping | LocomotionState::Falling
        );
        // ignore player inputs if control is removed (ex: knocked down)
        if !creature_stats.input_enabled {
            *player_input = PlayerInput::default();
        }
        player_state.transform = *player_transform;

        let is_grounded = output.map(|o| o.grounded).unwrap_or(false);

        // --- 1. Determine Target LocomotionState ---
        let target_locomotion_state = determine_target_locomotive_state(
            original_locomotion_state,
            &player_input,
            is_grounded,
            *grounded_timer,
        );

        // --- 2. Trigger animation based on target locomotion state ---
        if target_locomotion_state != *original_locomotion_state {
            trigger_animation_request_from_locomotion_state(
                &target_locomotion_state,
                creature_stats.id,
                &mut commands,
                &creature_stats.current_weapon_state,
            );
            creature_stats.current_locomotion_state = target_locomotion_state;
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

            if is_grounded {
                //record our height so we know if we should take fall damage
                *last_ground_height = player_transform.translation.y;
                *grounded_timer = GROUND_TIMER;
                *vertical_movement = 0.0;
                if prev_state_was_airborne {
                    let sound_event = SoundEvent {
                        sound_anchor_entity: Some(player_entity),
                        translation: None,
                        sound_path: "sounds/bodyfallMED.wav".to_string(),
                        is_attached_to_local_player: true,
                    };

                    commands.trigger(sound_event.clone());
                    commands.trigger(NetworkEvent::Sound(sound_event));

                    // handle landing from a jump, seek to start of landing animation
                    commands.trigger(SetAnimSpeedSeekEvent {
                        skeleton_id: player_state.id,
                        animation: "jump".to_string(),
                        speed: 1.0,
                        seek_to: Some(0.3),
                        networked: true,
                    });
                }
            }
            // If we are grounded we can jump
            if *grounded_timer > 0.0 {
                *grounded_timer -= delta_time;
                // If we jump we clear the grounded tolerance
                if jump_speed > 0.0 {
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
            // fall damage
            if *last_ground_height > player_transform.translation.y + 0.5 {
                creature_stats.health -= 5.0;
            }
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
fn trigger_animation_request_from_locomotion_state(
    target_locomotion_state: &LocomotionState,
    skeleton_id: u64,
    commands: &mut Commands,
    current_weapon_state: &str,
) {
    let animation_config_opt = match target_locomotion_state {
        LocomotionState::Idle => Some(AnimationConfig {
            name: format!("idle{}", current_weapon_state),
            repeat: RepeatAnim::Forever, // Idles usually loop via their own _loop event
            priority: 0,
            mask: BlendMask::all(),
        }),
        LocomotionState::Moving(direction) => {
            let name = match direction {
                Direction::Forward => format!("runforward{}", current_weapon_state),
                Direction::Left => format!("runleft{}", current_weapon_state),
                Direction::Right => format!("runright{}", current_weapon_state),
                Direction::Backward => format!("runback{}", current_weapon_state),
                Direction::None => format!("idle{}", current_weapon_state),
            };
            Some(AnimationConfig {
                name,
                repeat: RepeatAnim::Never,
                priority: 0,
                mask: BlendMask::all(),
            })
        }
        LocomotionState::Jumping => Some(AnimationConfig {
            name: format!("jump{}", current_weapon_state),
            repeat: RepeatAnim::Never,
            priority: 1,
            mask: BlendMask::all(),
        }),
        LocomotionState::Falling => Some(AnimationConfig {
            name: format!("jump{}", current_weapon_state),
            repeat: RepeatAnim::Never,
            priority: 0, // Same as jump, or slightly lower
            mask: BlendMask::all(),
        }),
        LocomotionState::Knockdown => Some(AnimationConfig {
            name: "knockdown".to_string(),
            repeat: RepeatAnim::Never,
            priority: 100,
            mask: BlendMask::all(),
        }),
        // When transitioning to Landing state, we do NOT trigger a new animation.
        // The existing animation (e.g., "jump" or "fall_loop") is expected to have
        // landing frames and will emit a "ResumeIdle" (or similar) event when done.
        // The SetAnimSpeedSeekEvent in player_movement already ensures "jump" plays at speed 1.0 on land.
        LocomotionState::Landing => None,
        LocomotionState::FinishedLanding => Some(AnimationConfig {
            name: format!("idle{}", current_weapon_state),
            repeat: RepeatAnim::Forever, // Idles usually loop via their own _loop event
            priority: 0,
            mask: BlendMask::all(),
        }),
    };

    if let Some(config) = animation_config_opt {
        commands.trigger(RequestAnimation {
            skeleton_id,
            animation_config: config,
            networked: true,
        });
    }
}
fn get_movement_animation_from_locomotive_state(
    original_locomotion_state: &LocomotionState,
    current_weapon_state: &str,
) -> String {
    match original_locomotion_state {
        LocomotionState::Idle => format!("idle{}", current_weapon_state),
        LocomotionState::Moving(direction) => match direction {
            Direction::Forward => format!("runforward{}", current_weapon_state),
            Direction::Left => format!("runleft{}", current_weapon_state),
            Direction::Right => format!("runright{}", current_weapon_state),
            Direction::Backward => format!("runback{}", current_weapon_state),
            Direction::None => format!("idle{}", current_weapon_state),
        },
        LocomotionState::Jumping => format!("jump{}", current_weapon_state),

        _ => "idle".to_string(),
    }
}
fn determine_target_locomotive_state(
    original_locomotion_state: &LocomotionState,
    player_input: &PlayerInput,
    is_grounded: bool,
    grounded_timer: f32,
) -> LocomotionState {
    let no_horizontal_input = player_input.move_direction.length_squared() <= 0.0; // Check for significant input
    let can_jump_from_ground_or_coyote = grounded_timer > 0.0;
    // Handle ongoing knockdown first (highest precedence if active)
    if *original_locomotion_state == LocomotionState::Knockdown {
        // Remain Knockdown until an event like KnockdownFinished changes it externally
        return LocomotionState::Knockdown;
    }

    // --- Check for interruptions or persistence of the Landing state ---
    if *original_locomotion_state == LocomotionState::Landing {
        // Player can interrupt the landing animation by jumping or moving.
        if player_input.jump && can_jump_from_ground_or_coyote {
            return LocomotionState::Jumping;
        }
        if !no_horizontal_input && is_grounded {
            // Should still be grounded if landing
            return LocomotionState::Moving(determine_movement_direction_from_input(player_input));
        }
        // Otherwise, stay in the Landing state. The animation will trigger "ResumeIdle".
        return LocomotionState::Landing;
    }

    // --- Determine state if NOT currently Landing or Knockdown ---

    // Just Landed: If previously airborne (Jumping or Falling) and now grounded
    if (*original_locomotion_state == LocomotionState::Jumping
        || *original_locomotion_state == LocomotionState::Falling)
        && is_grounded
    {
        return LocomotionState::Landing; // Transition to Landing state
    }

    // Handle jump request (if not just landed into Landing state)
    if player_input.jump && can_jump_from_ground_or_coyote {
        return LocomotionState::Jumping;
    }

    // Handle grounded states (Idle or Moving)
    if is_grounded {
        if no_horizontal_input {
            return LocomotionState::Idle;
        } else {
            return LocomotionState::Moving(determine_movement_direction_from_input(player_input));
        }
    } else {
        // In air
        // If was in a jump animation, let it continue (it might transition to Falling via its own event or logic)
        if *original_locomotion_state == LocomotionState::Jumping {
            return LocomotionState::Jumping;
        }
        // Otherwise, if airborne and not in an active jump sequence, must be falling.
        return LocomotionState::Falling;
    }
}
fn determine_movement_direction_from_input(player_input: &PlayerInput) -> Direction {
    if player_input.move_direction.y > 0.0 {
        Direction::Forward
    } else if player_input.move_direction.x < 0.0 && player_input.move_direction.y == 0.0 {
        Direction::Left
    } else if player_input.move_direction.x > 0.0 && player_input.move_direction.y == 0.0 {
        Direction::Right
    } else if player_input.move_direction.y < 0.0 {
        Direction::Backward
    } else {
        Direction::None
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
    trigger: Trigger<TryAttackTrigger>,
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
            exclude_collider: Some(trigger.attacker_entity),
            ..default()
        },
    );
    if let Some((entity, _toi)) = hit {
        println!("hit entity: {}", entity);
        if let Ok(creature) = creature_q.get(entity) {
            commands.trigger(TriggerHit {
                attacker_id: trigger.attacker_id,
                receiver_id: creature.id,
                damage: trigger.damage,
                networked: true,
            });
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
    mut player_query: Query<(&mut PlayerState, &mut CreatureStats), With<LocalPlayer>>,
    mut commands: Commands,
) {
    if let Ok((player_state, mut creature_stats)) = player_query.get_mut(trigger.0) {
        creature_stats.input_enabled = true;
        commands.trigger(RequestAnimation {
            skeleton_id: player_state.id,
            animation_config: AnimationConfig {
                //TODO:: check if weapon/magic ready
                name: format!("idle{}", creature_stats.current_weapon_state),
                repeat: RepeatAnim::Forever,
                priority: 1,
                mask: BlendMask::all(),
            },
            networked: true,
        });
    }
    info!("knockdown finished!");
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
