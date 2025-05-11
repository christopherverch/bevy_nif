use crate::nif::attach_parts::AttachmentType;
use crate::nif::spawner::NifScene;
use bevy::prelude::*;
use bevy_third_person_camera::*;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let entity = commands
        .spawn((
            InheritedVisibility::VISIBLE,
            Transform {
                translation: Vec3::new(0.0, -2.5, 1.0),
                rotation: Quat::from_rotation_x(-FRAC_PI_2),
                scale: Vec3::splat(0.03),
            },
        ))
        .id();
    let skeleton_handle = asset_server.load("data/base_anim.nif");
    let head_handle = asset_server.load("data/B_N_Dark Elf_M_Head_01.nif");
    commands.spawn((
        NifScene(head_handle.clone()),
        InheritedVisibility::VISIBLE,
        AttachmentType::Rigid {
            skeleton_id: 3,
            target_bone: "Head".to_string(),
        },
        Transform {
            translation: Vec3::new(0.0, -2.5, 3.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1.0),
        },
    ));

    for i in 0..1 {
        commands.spawn((
            NifScene(skeleton_handle.clone()),
            AttachmentType::MainSkeleton { skeleton_id: 3 },
            InheritedVisibility::VISIBLE,
            Transform {
                translation: Vec3::new(0.0, -2.5, i as f32),
                rotation: Quat::from_rotation_x(-FRAC_PI_2),
                scale: Vec3::splat(0.03),
            },
        ));
        commands.spawn((
            NifScene(head_handle.clone()),
            InheritedVisibility::VISIBLE,
            Transform {
                translation: Vec3::new(0.0, -2.5, i as f32),
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.03),
            },
        ));
    }
    let skeleton_id = 0;
    let paths_with_target_bone: Vec<(&str, AttachmentType)> = vec![
        (
            "data/base_anim.nif",
            AttachmentType::MainSkeleton {
                skeleton_id: skeleton_id,
            },
        ),
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
    let skeleton_id = 1;
    let paths_with_target_bone2: Vec<(&str, AttachmentType)> = vec![
        (
            "data/base_anim.nif",
            AttachmentType::MainSkeleton {
                skeleton_id: skeleton_id,
            },
        ),
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
        entity,
        &mut commands,
        Transform::from_scale(Vec3::splat(1.0)),
    );
    let entity2 = commands
        .spawn((
            InheritedVisibility::VISIBLE,
            Transform {
                translation: Vec3::new(2.0, -2.5, 1.0),
                rotation: Quat::from_rotation_x(-FRAC_PI_2),
                scale: Vec3::splat(0.03),
            },
        ))
        .id();
    spawn_nifs(
        paths_with_target_bone2,
        &asset_server,
        entity2,
        &mut commands,
        Transform::from_scale(Vec3::splat(1.0)),
    );
}
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

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
    let font = asset_server.load("fonts/FiraMono-Medium.ttf");
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font,
            font_size: 00.0,
            ..default()
        },
    ));
    let transform_almost_down = Transform::from_rotation(Quat::from_axis_angle(
        Vec3::X,
        std::f32::consts::PI / 2.0 * -0.98,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.,
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

    // Example instructions
    let font_handle = asset_server.load("fonts/FiraMono-Medium.ttf");
    commands.spawn((
        // Accepts a `String` or any type that converts into a `String`, such as `&str`
        Text::new("test"),
        TextFont {
            // This font is loaded and will be used instead of the default font.
            font: font_handle,
            font_size: 25.0,
            ..default()
        },
        // Set the justification of the Text
        TextLayout::new_with_justify(JustifyText::Left),
        // Set the style of the Node itself.
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}
