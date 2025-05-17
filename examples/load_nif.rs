use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_nif::nif::animation::{NifAnimator, SkeletonMap};
use bevy_nif::*;
use bevy_third_person_camera::*;
mod setup;
use bitflags::bitflags;

bitflags! {
    // `Default` will make `BlendMask::empty()` the default.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BlendMask: u8 {
        const LowerBody = 1 << 0;
        const Torso     = 1 << 1;
        const LeftArm   = 1 << 2;
        const RightArm  = 1 << 3;

        const UpperBody = Self::Torso.bits() | Self::LeftArm.bits() | Self::RightArm.bits();
        const All       = Self::LowerBody.bits() | Self::UpperBody.bits();

    }
}

const BLEND_MASK_ROOTS: [&str; 3] = [
    "Bip01 Spine1",     /* Torso */
    "Bip01 L Clavicle", /* Left arm */
    "Bip01 R Clavicle", /* Right arm */
];
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin {})
        .add_plugins(BevyNifPlugin)
        .insert_resource(AmbientLight {
            // Add ambient light resource
            color: Color::srgb(1.0, 0.8, 0.6), // Match warm tone
            brightness: 100.0,
        })
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(ThirdPersonCameraPlugin)
        .add_systems(Startup, setup::setup_scene)
        .add_systems(Startup, setup::setup)
        .add_systems(Update, test_animations)
        .run();
}

fn test_animations(
    mut animator_q: Query<(&mut AnimationPlayer, &NifAnimator, &AnimationGraphHandle)>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    mut run_once: Local<bool>,
    skeleton_map_res: Res<SkeletonMap>,
) {
    if !*run_once {
        if animator_q.iter().len() < 2 {
            return;
        }
        for (id, skeleton) in &skeleton_map_res.skeletons {
            println!("id: {}", id);
            for (mut animation_player, nif_animator, graph_handle) in animator_q.iter_mut() {
                if nif_animator.skeleton_id != *id {
                    continue;
                }
                let anim_graph = anim_graphs.get_mut(graph_handle).unwrap();
                let walk_back_animation = nif_animator.animations.get("runforward").unwrap();

                let pickprobe_animation = nif_animator.animations.get("spellcast").unwrap();
                if let Some(anim_graph_node) = anim_graph.get_mut(*pickprobe_animation) {
                    let mut mask: u64 = 0;
                    for bone_data in skeleton.get_all_children("Bip01 L Thigh") {
                        mask |= 1 << bone_data.id.0 as u32;
                    }
                    for bone_data in skeleton.get_all_children("Bip01 R Thigh") {
                        mask |= 1 << bone_data.id.0 as u32;
                    }
                    println!("mask: {:b}", mask);
                    anim_graph_node.mask = mask as u64;
                }
                let mut mask = 0;

                for bone_data in skeleton.get_all_children("Bip01 R Clavicle") {
                    mask |= 1 << bone_data.id.0 as u32;
                }
                for bone_data in skeleton.get_all_children("Bip01 L Clavicle") {
                    mask |= 1 << bone_data.id.0 as u32;
                }
                let walk_anim_graph_node = anim_graph.get_mut(*walk_back_animation).unwrap();
                walk_anim_graph_node.mask = mask;
                animation_player
                    .play(*pickprobe_animation)
                    .seek_to(0.0)
                    .repeat();
                animation_player.play(*walk_back_animation).repeat();
                *run_once = true;
            }
        }
    }
}
