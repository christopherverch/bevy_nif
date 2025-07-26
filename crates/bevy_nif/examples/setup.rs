use bevy::pbr::OpaqueRendererMethod;
use bevy::prelude::*;
use bevy_nif::nif::attach_parts::AttachmentType;
use bevy_nif::nif::loader::Nif;
use bevy_nif::nif::physics::NifCollisionShape;
use bevy_nif::nif::spawner::NifScene;
use bevy_rapier3d::prelude::{ActiveEvents, Collider, RigidBody};
use bevy_third_person_camera::*;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_nif_unattached(commands.reborrow(), &asset_server);
    let player_entity = commands
        .spawn(Transform {
            translation: Vec3::new(-2.0, -2.0, -1.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        })
        .id();
    spawn_nif_attached(
        player_entity,
        commands.reborrow(),
        &asset_server,
        0,
        Transform::from_scale(Vec3::splat(0.03)),
    );
}
use std::f32::consts::{FRAC_PI_4, PI};

use bevy::render::mesh::Mesh;
use bevy::{
    core_pipeline::{
        fxaa::Fxaa,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
    },
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver},
};

fn spawn_nifs(
    paths_with_target_bone: Vec<(&str, AttachmentType)>,
    asset_server: &Res<AssetServer>,
    entity: Entity,
    commands: &mut Commands,
    transform: Transform,
) {
    for (path, mut attachment_type) in paths_with_target_bone {
        let asset_handle = asset_server.load(path);
        if let AttachmentType::DoubleSidedRigid {
            target_bone,
            skeleton_id,
        } = attachment_type.clone()
        {
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Left {}", target_bone),
                skeleton_id,
            };
            let child = commands
                .spawn((
                    NifScene(asset_handle.clone()),
                    attachment_type,
                    transform,
                    InheritedVisibility::VISIBLE,
                ))
                .id();
            commands.entity(entity).add_child(child);
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Right {}", target_bone),
                skeleton_id,
            };
        }
        let child = commands
            .spawn((
                NifScene(asset_handle),
                attachment_type,
                transform,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        commands.entity(entity).add_child(child);
    }
}

pub fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            // Deferred both supports both hdr: true and hdr: false
            hdr: true,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: PI / 3.6,
            aspect_ratio: 1.0,
            near: 0.1,
            far: 1000.0,
        }),
        ThirdPersonCamera::default(),
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        // MSAA needs to be off for Deferred rendering
        Msaa::Off,
        DepthPrepass,
        MotionVectorPrepass,
        DeferredPrepass,
        Fxaa::default(),
    ));

    let transform_almost_down = Transform::from_rotation(Quat::from_axis_angle(
        Vec3::X,
        std::f32::consts::PI / 2.0 * -0.98,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 2_000.,
            shadows_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 10.0,
            ..default()
        }
        .build(),
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 0.0, -FRAC_PI_4)),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 600.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.85, 0.7), // Slightly warm (orangey/yellowish) white
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 10.0,
            ..default()
        }
        .build(),
        transform_almost_down,
    ));

    // sky
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("888888").unwrap().into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        NotShadowCaster,
        NotShadowReceiver,
    ));
    let mut forward_mat: StandardMaterial = Color::srgb(0.1, 0.2, 0.1).into();
    forward_mat.opaque_render_method = OpaqueRendererMethod::Forward;
    let forward_mat_h = materials.add(forward_mat);

    let cube_h2 = meshes.add(Cuboid::new(0.5, 0.5, 0.5));
    commands.spawn((
        Mesh3d(cube_h2.clone()),
        MeshMaterial3d(forward_mat_h.clone()),
        Transform::default(),
    ));
}
fn spawn_nif_unattached(mut commands: Commands, asset_server: &Res<AssetServer>) {
    let head_handle = asset_server.load("data/A_Glass_Helmet.nif");
    commands.spawn((
        NifScene(head_handle),
        InheritedVisibility::VISIBLE,
        Transform {
            translation: Vec3::new(-0.0, 0.5, -1.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.03), //nifs are huge otherwise
        },
    ));
}

#[derive(Component)]
pub struct NeedsNifPhysics(pub Entity);
/// This system runs once for each newly spawned `NifScene` entity which
/// also has a NeedsNifPhysic component.
/// It creates and attaches the appropriate Rapier physics components, handling
/// local collider offsets by spawning a dedicated child entity for the collider.
pub fn setup_nif_physics(
    mut commands: Commands,
    nif_assets: Res<Assets<Nif>>,
    query: Query<(Entity, &NifScene, &Children, &NeedsNifPhysics)>,
) {
    for (top_level_entity, nif_scene, children, needs_physics) in query.iter() {
        // Mark the top-level entity so we don't process it again.
        commands
            .entity(top_level_entity)
            .remove::<NeedsNifPhysics>();

        let Some(nif_asset) = nif_assets.get(&nif_scene.0) else {
            continue;
        };
        if nif_asset.collision_shapes.is_empty() {
            continue;
        };

        // The actual root of the spawned NIF visuals. This will be the RigidBody.
        let Some(nif_root_entity) = children.first() else {
            continue;
        };

        commands.entity(needs_physics.0).insert(Name::new("test"));

        // --- Determine RigidBody Type
        let rigid_body_type = if nif_scene.0.path().map_or(false, |p| {
            p.path().to_str().unwrap_or("").contains("base_anim")
        }) {
            RigidBody::KinematicPositionBased
        } else {
            RigidBody::Fixed
        };

        // Add the RigidBody component to the root.
        commands.entity(needs_physics.0).insert(rigid_body_type);
        // --- Process each collision shape and spawn it as a child ---
        for collision_shape in &nif_asset.collision_shapes {
            // This match produces a tuple: the Collider shape and its local Transform.
            let collider_data_opt: Option<(Collider, Transform)> = match collision_shape {
                NifCollisionShape::Box { center, extents } => {
                    Some((
                        Collider::cuboid(extents.x, extents.y, extents.z),
                        Transform::from_translation(*center), // The local offset
                    ))
                }
                NifCollisionShape::Capsule {
                    extent,
                    radius,
                    center,
                    ..
                } => Some((
                    Collider::capsule_y(*extent / 2.0, *radius),
                    Transform::from_translation(*center), // The local offset
                )),
                NifCollisionShape::Sphere { center, radius } => Some((
                    Collider::ball(*radius),
                    Transform::from_translation(*center), // The local offset
                )),
                NifCollisionShape::TriangleMesh { vertices, indices } => {
                    // Trimeshes from NIFs usually don't have a separate offset.
                    match Collider::trimesh(vertices.clone(), indices.clone()) {
                        Ok(collider) => Some((collider, Transform::IDENTITY)),
                        Err(e) => {
                            warn!("Failed to create trimesh collider: {:?}", e);
                            None
                        }
                    }
                }
                NifCollisionShape::Union(shapes) => {
                    // For a Union, we spawn multiple children, one for each sub-shape.
                    for sub_shape in shapes {
                        // (This is a simplified recursive call for clarity)
                        if let Some((sub_collider, sub_transform)) =
                            convert_to_collider_and_transform(sub_shape)
                        {
                            let collider_entity = commands
                                .spawn((
                                    sub_collider,
                                    sub_transform, // Apply the sub-shape's local transform
                                    Name::new("SubCollider"),
                                ))
                                .id();
                            commands.entity(*nif_root_entity).add_child(collider_entity);
                        }
                    }
                    None // The parent Union doesn't create a collider itself.
                }
            };

            // If we successfully created a collider and its transform, spawn it.
            if let Some((collider, mut local_transform)) = collider_data_opt {
                local_transform.translation *= 0.03;
                dbg!(local_transform);
                dbg!(&collider);
                let collider_entity = commands
                    .spawn((
                        collider,
                        local_transform.with_scale(Vec3::splat(0.03)),
                        Name::new("Collider"),
                        // We also add these so the child collider can report events.
                        ActiveEvents::COLLISION_EVENTS,
                    ))
                    .id();
                commands.entity(needs_physics.0).add_child(collider_entity);
                // Parent the new collider entity to the NIF's root entity.
            }
        }
    }
}

// A small helper to avoid duplicating the match logic inside the Union arm.
fn convert_to_collider_and_transform(shape: &NifCollisionShape) -> Option<(Collider, Transform)> {
    match shape {
        NifCollisionShape::Box { center, extents } => Some((
            Collider::cuboid(extents.x, extents.y, extents.z),
            Transform::from_translation(*center),
        )),
        _ => None, // Add other shapes here as needed
    }
}
fn spawn_nif_attached(
    player_entity: Entity,
    mut commands: Commands,
    asset_server: &Res<AssetServer>,
    skeleton_id: u64,
    transform: Transform,
) -> Entity {
    let asset_handle = asset_server.load("data/base_anim.nif");
    let child = commands
        .spawn((
            NifScene(asset_handle),
            AttachmentType::MainSkeleton { skeleton_id },
            transform,
            InheritedVisibility::VISIBLE,
            NeedsNifPhysics(player_entity),
        ))
        .id();
    commands.entity(player_entity).add_child(child);
    let paths_with_target_bone: Vec<(&str, AttachmentType)> = vec![
        (
            "data/b_n_dark elf_m_skins.nif",
            AttachmentType::Skinned { skeleton_id },
        ),
        (
            "data/B_N_Dark Elf_M_Head_01.nif",
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Hair_01.nif",
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Neck.nif",
            AttachmentType::Rigid {
                target_bone: "Neck".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Groin.nif",
            AttachmentType::Rigid {
                target_bone: "Groin".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Forearm.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Forearm".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Upper Arm.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Upper Arm".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Wrist.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Wrist".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Upper Leg.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Upper Leg".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Knee.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Knee".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Ankle.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Ankle".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Foot.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Foot".to_string(),
                skeleton_id,
            },
        ),
    ];
    spawn_nifs(
        paths_with_target_bone,
        &asset_server,
        player_entity,
        &mut commands,
        Transform::default(),
    );
    player_entity
}
