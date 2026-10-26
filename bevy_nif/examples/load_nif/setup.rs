use bevy::camera_controller::free_camera::FreeCamera;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy_material::OpaqueRendererMethod;
use bevy_nif::attach_parts::AttachmentType;
use bevy_nif::loader::NifSettings;
use bevy_pbr::{MeshMaterial3d, StandardMaterial};

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_nif_unattached(commands.reborrow(), &asset_server);
    let player_entity = commands
        .spawn(Transform {
            translation: Vec3::new(-2.0, 0.0, -1.0),
            ..default()
        })
        .id();
    spawn_nif_attached(
        player_entity,
        commands.reborrow(),
        &asset_server,
        Transform::default(),
    );
    let player_entity = commands
        .spawn(Transform {
            translation: Vec3::new(-10.0, 0.0, -1.0),
            ..default()
        })
        .id();
    spawn_nif_attached(
        player_entity,
        commands.reborrow(),
        &asset_server,
        Transform::default(),
    );
}
use std::f32::consts::FRAC_PI_4;

fn spawn_nifs(
    paths_with_target_bone: Vec<(&str, AttachmentType)>,
    asset_server: &Res<AssetServer>,
    entity: Entity,
    commands: &mut Commands,
    transform: Transform,
) {
    for (path, mut attachment_type) in paths_with_target_bone {
        let asset_handle = asset_server.load(path.to_string());
        if let AttachmentType::DoubleSidedRigid {
            target_bone,
            skeleton_entity,
        } = attachment_type.clone()
        {
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Left {}", target_bone),
                skeleton_entity,
            };
            let child = commands
                .spawn((
                    WorldAssetRoot(asset_handle.clone()),
                    attachment_type,
                    transform,
                    InheritedVisibility::VISIBLE,
                ))
                .id();
            commands.entity(entity).add_child(child);
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Right {}", target_bone),
                skeleton_entity,
            };
        }
        let child = commands
            .spawn((
                WorldAssetRoot(asset_handle),
                transform,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        commands.entity(child).insert(attachment_type.clone());

        commands.entity(entity).add_child(child);
    }
}

pub fn setup_scene(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let head = asset_server.load("data/B_N_Dark Elf_M_Head_01.nif");
    let hair = asset_server.load("data/B_N_Dark Elf_M_Hair_01.nif");
    commands.spawn((
        Name::new("head"),
        WorldAssetRoot(head),
        Visibility::Visible,
        Transform::default(),
    ));

    commands.spawn((
        Name::new("hair"),
        Visibility::Visible,
        WorldAssetRoot(hair),
        Transform::default(),
    ));

    commands.spawn((
        Camera3d::default(),
        FreeCamera::default(),
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
    ));

    let transform_almost_down = Transform::from_rotation(Quat::from_axis_angle(
        Vec3::X,
        std::f32::consts::PI / 2.0 * -0.98,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 2_000.,
            shadow_maps_enabled: true,
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
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.85, 0.7),
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
        WorldAssetRoot(head_handle),
        InheritedVisibility::VISIBLE,
        Transform {
            translation: Vec3::new(-0.0, 0.5, -1.0),
            ..default()
        },
    ));
}

/// This system runs once for each newly spawned `WorldAssetRoot` entity which
/// also has a NeedsNifPhysic component.
/// It creates and attaches the appropriate Rapier physics components, handling
/// local collider offsets by spawning a dedicated child entity for the collider.

// TODO::
/*
pub fn setup_nif_physics(
    mut commands: Commands,
    nif_assets: Res<Assets<Nif>>,
    query: Query<(Entity, &WorldAssetRoot, &Children, &NeedsNifPhysics)>,
) {
    for (top_level_entity, nif_scene, children, needs_physics) in query.iter() {
        // Mark the top-level entity so we don't process it again.
        commands
            .entity(top_level_entity)
            .remove::<NeedsNifPhysics>();

        let Some(nif_asset) = nif_assets.get(&nif_scene.0) else {
            continue;
        };

        // The actual root of the spawned NIF visuals. This will be the RigidBody.
        let Some(nif_root_entity) = children.first() else {
            continue;
        };

        // --- Determine RigidBody Type
        let rigid_body_type = if nif_scene.0.path().map_or(false, |p| {
            p.path().to_str().unwrap_or("").contains("base_anim")
        }) {
            RigidBody::KinematicPositionBased
        } else {
            RigidBody::Fixed
        };

        // Add the RigidBody component to the root.
        // --- Process each collision shape and spawn it as a child ---
    }
}
*/
fn spawn_nif_attached(
    player_entity: Entity,
    mut commands: Commands,
    asset_server: &Res<AssetServer>,
    transform: Transform,
) -> Entity {
    let asset_handle = asset_server
        .load_builder()
        .with_settings(move |settings: &mut NifSettings| settings.is_skeleton = true)
        .load("data/base_anim.nif");
    let skeleton_entity = commands
        .spawn((
            WorldAssetRoot(asset_handle),
            Name::new("skeleton"),
            transform,
            InheritedVisibility::VISIBLE,
        ))
        .id();
    commands.entity(player_entity).add_child(skeleton_entity);
    let paths_with_target_bone: Vec<(&str, AttachmentType)> = vec![
        (
            "data/b_n_dark elf_m_skins.nif",
            AttachmentType::Skinned { skeleton_entity },
        ),
        (
            "data/B_N_Dark Elf_M_Head_01.nif",
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Hair_01.nif",
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Neck.nif",
            AttachmentType::Rigid {
                target_bone: "Neck".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Groin.nif",
            AttachmentType::Rigid {
                target_bone: "Groin".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Forearm.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Forearm".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Upper Arm.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Upper Arm".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Wrist.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Wrist".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Upper Leg.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Upper Leg".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Knee.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Knee".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Ankle.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Ankle".to_string(),
                skeleton_entity,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Foot.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Foot".to_string(),
                skeleton_entity,
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
