use bevy::prelude::*;
use bevy_nif::{attach_parts::AttachmentType, spawner::NifScene};
use esm_parser_lib::{Bodypart, BodypartId, BodypartType, RefId, types::bodypart::BodypartFlags};
use esm_store::EsmStore;
use strum::EnumCount;
use strum_macros::EnumCount as EnumCountMacro;

use crate::{
    animation::NifSkeletonId,
    defs::{LocalPlayerMesh, MeshEntityLink},
    physics::{CreatureControllerLink, RootMotionState},
    utils::prepend_mesh_data_path,
};

#[derive(Component)]
pub struct NpcMesh;
pub fn spawn_npc(
    root_entity: Entity,
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    skeleton_id: u64,
    parts: Vec<Option<&Bodypart>>,
    head_path: String,
    hair_path: String,
    is_local_player: bool,
) -> Entity {
    let asset_handle = asset_server.load("data/base_anim.nif");
    let skeleton_entity = commands
        .spawn((
            NifScene(asset_handle),
            NpcMesh,
            AttachmentType::MainSkeleton { skeleton_id },
            CreatureControllerLink(root_entity),
            Transform {
                translation: Vec3::ZERO,
                ..default()
            },
            Visibility::Inherited,
            ShowAabbGizmo::default(),
        ))
        .id();
    commands.entity(skeleton_entity).insert((
        // for tracking animation times with the animation system
        RootMotionState::default(),
        Name::new("player mesh"),
    ));
    commands
        .entity(root_entity)
        .insert(MeshEntityLink(skeleton_entity));
    if is_local_player {
        commands.entity(skeleton_entity).insert((
            //Default to first person, so hide the player's mesh
            LocalPlayerMesh,
        ));
    }
    commands.entity(root_entity).add_child(skeleton_entity);
    commands
        .entity(root_entity)
        .insert(NifSkeletonId(skeleton_id));
    let mut skins_path = String::new();
    for part in &parts {
        if let Some(part) = part {
            if part.model.contains("Skins") {
                skins_path = prepend_mesh_data_path(part.model.clone());
                break;
            }
        }
    }
    let mut paths_to_spawn = Vec::new();
    for (idx, part_opt) in parts.iter().enumerate() {
        if let Some(part) = part_opt {
            let part_ref = PartReference::from_usize_unchecked(idx);
            if part.model.contains("Skins") {
                // Skip the skinned mesh parts, we already took care of that
                continue;
            }

            if let Some(attachment) = get_attachment_info(part_ref).build(skeleton_id) {}
        }
    }
    if skins_path != "" {
        paths_to_spawn.push((skins_path, AttachmentType::Skinned { skeleton_id }));
    };
    paths_to_spawn.push((
        hair_path,
        AttachmentType::Rigid {
            target_bone: "Head".to_string(),
            skeleton_id,
        },
    ));
    paths_to_spawn.push((
        head_path,
        AttachmentType::Rigid {
            target_bone: "Head".to_string(),
            skeleton_id,
        },
    ));
    spawn_nifs(
        paths_to_spawn,
        &asset_server,
        skeleton_entity,
        commands,
        Transform::from_scale(Vec3::splat(1.0)),
    );
    skeleton_entity
}

fn spawn_nifs(
    paths_with_target_bone: Vec<(String, AttachmentType)>,
    asset_server: &Res<AssetServer>,
    skeleton_entity: Entity,
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
                    Visibility::Inherited,
                ))
                .id();
            commands.entity(skeleton_entity).add_child(child);
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Right {}", target_bone),
                skeleton_id,
            };
        }
        let child = commands
            .spawn((
                NifScene(asset_handle),
                attachment_type.clone(),
                transform,
                Visibility::Inherited,
            ))
            .id();

        commands.entity(skeleton_entity).add_child(child);
    }
}

pub fn resolve_body_parts<'a>(
    race: &RefId,
    female: bool,
    first_person: bool,
    werewolf: bool,
    store: &'a EsmStore,
) -> Vec<Option<&'a Bodypart>> {
    let mut parts = vec![None; PartReference::COUNT];

    if werewolf {
        return parts;
    }

    // Iterate through all body parts once
    for bodypart in store.bodyparts.iter_shared() {
        if bodypart.data.flags.contains(BodypartFlags::NOT_PLAYABLE)
            || bodypart.data.bodypart_type != BodypartType::Skin
            || bodypart.race != *race
        {
            continue;
        }

        let is_hand = matches!(
            bodypart.data.part,
            BodypartId::Hand | BodypartId::Wrist | BodypartId::Forearm | BodypartId::Upperarm
        );
        let part_is_fp = bodypart.is_first_person_bodypart();
        let is_same_gender = bodypart.data.flags.contains(BodypartFlags::FEMALE) == female;
        let targets = get_part_references(bodypart.data.part);

        // 1. First Person Arm Fallback
        if first_person && is_hand && !part_is_fp {
            for &target in targets {
                let slot = target as usize;
                // Conditions map directly to C++ logic:
                // If empty and same gender, OR if we have a male part but found a female part
                let existing = parts[slot].as_ref();

                if parts[slot].is_none() && is_same_gender {
                    parts[slot] = Some(bodypart);
                } else if is_same_gender
                    && (existing.map_or(true, |p| {
                        p.data.flags.contains(BodypartFlags::FEMALE) != female
                    }))
                {
                    parts[slot] = Some(bodypart);
                } else if parts[slot].is_none() && female {
                    parts[slot] = Some(bodypart);
                }
            }
            continue;
        }

        // 2. View Mode enforcement
        if part_is_fp != first_person {
            continue;
        }

        // 3. Female Gender Fallback (Allow male parts if female missing)
        if female && !bodypart.data.flags.contains(BodypartFlags::FEMALE) {
            for &target in targets {
                let slot = target as usize;
                let existing = parts[slot].as_ref();

                if parts[slot].is_none() {
                    parts[slot] = Some(bodypart);
                } else if is_hand
                    && existing.map_or(false, |p| !p.is_first_person_bodypart())
                    && part_is_fp
                {
                    parts[slot] = Some(bodypart);
                }
            }
            continue;
        }

        // 4. Exact match (Use properly found bodypart, replacing previous fallbacks)
        if female == bodypart.data.flags.contains(BodypartFlags::FEMALE) {
            for &target in targets {
                parts[target as usize] = Some(bodypart);
            }
        }
    }
    parts
}
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumCountMacro)]
pub enum PartReference {
    Neck,
    Cuirass,
    Groin,
    RHand,
    LHand,
    RWrist,
    LWrist,
    RForearm,
    LForearm,
    RUpperarm,
    LUpperarm,
    RFoot,
    LFoot,
    RAnkle,
    LAnkle,
    RKnee,
    LKnee,
    RLeg,
    LLeg,
    Tail,
}
impl PartReference {
    #[inline(always)]
    pub fn from_usize_unchecked(index: usize) -> Self {
        // SAFETY: We strictly assume the caller ensures index < COUNT.
        // This is a direct cast with no branching or runtime checks.
        unsafe { std::mem::transmute(index as u8) }
    }
}
pub fn get_part_references(mesh_part: BodypartId) -> &'static [PartReference] {
    match mesh_part {
        BodypartId::Neck => &[PartReference::Neck],
        BodypartId::Chest => &[PartReference::Cuirass],
        BodypartId::Groin => &[PartReference::Groin],
        BodypartId::Hand => &[PartReference::RHand, PartReference::LHand],
        BodypartId::Wrist => &[PartReference::RWrist, PartReference::LWrist],
        BodypartId::Forearm => &[PartReference::RForearm, PartReference::LForearm],
        BodypartId::Upperarm => &[PartReference::RUpperarm, PartReference::LUpperarm],
        BodypartId::Foot => &[PartReference::RFoot, PartReference::LFoot],
        BodypartId::Ankle => &[PartReference::RAnkle, PartReference::LAnkle],
        BodypartId::Knee => &[PartReference::RKnee, PartReference::LKnee],
        BodypartId::Upperleg => &[PartReference::RLeg, PartReference::LLeg],
        BodypartId::Tail => &[PartReference::Tail],
        BodypartId::Head | BodypartId::Hair | BodypartId::Clavicle => &[],
    }
}
pub enum AttachmentTypeBuilder {
    Rigid { target_bone: String },
    None,
}

impl AttachmentTypeBuilder {
    pub fn build(&self, skeleton_id: u64) -> Option<AttachmentType> {
        match self {
            Self::Rigid { target_bone } => Some(AttachmentType::Rigid {
                target_bone: target_bone.clone(),
                skeleton_id,
            }),
            Self::None => None,
        }
    }
}

pub fn get_attachment_info(part_ref: PartReference) -> AttachmentTypeBuilder {
    match part_ref {
        PartReference::Neck => AttachmentTypeBuilder::Rigid {
            target_bone: "Neck".into(),
        },
        PartReference::Cuirass => AttachmentTypeBuilder::Rigid {
            target_bone: "Chest".into(),
        },
        PartReference::Groin => AttachmentTypeBuilder::Rigid {
            target_bone: "Groin".into(),
        },

        // Hands/Wrists/Forearms/Upperarms
        PartReference::RHand => AttachmentTypeBuilder::Rigid {
            target_bone: "Right Hand".into(),
        },
        PartReference::LHand => AttachmentTypeBuilder::Rigid {
            target_bone: "Left Hand".into(),
        },
        PartReference::RWrist => AttachmentTypeBuilder::Rigid {
            target_bone: "Right Wrist".into(),
        },
        PartReference::LWrist => AttachmentTypeBuilder::Rigid {
            target_bone: "Left Wrist".into(),
        },
        PartReference::RForearm => AttachmentTypeBuilder::Rigid {
            target_bone: "Right Forearm".into(),
        },
        PartReference::LForearm => AttachmentTypeBuilder::Rigid {
            target_bone: "Left Forearm".into(),
        },
        PartReference::RUpperarm => AttachmentTypeBuilder::Rigid {
            target_bone: "Right Upper Arm".into(),
        },
        PartReference::LUpperarm => AttachmentTypeBuilder::Rigid {
            target_bone: "Left Upper Arm".into(),
        },

        // Legs/Feet/Knees/Ankles
        PartReference::RLeg => AttachmentTypeBuilder::Rigid {
            target_bone: "Right Upper Leg".into(),
        },
        PartReference::LLeg => AttachmentTypeBuilder::Rigid {
            target_bone: "Left Upper Leg".into(),
        },
        PartReference::RKnee => AttachmentTypeBuilder::Rigid {
            target_bone: "Right Knee".into(),
        },
        PartReference::LKnee => AttachmentTypeBuilder::Rigid {
            target_bone: "Left Knee".into(),
        },
        PartReference::RAnkle => AttachmentTypeBuilder::Rigid {
            target_bone: "Right Ankle".into(),
        },
        PartReference::LAnkle => AttachmentTypeBuilder::Rigid {
            target_bone: "Left Ankle".into(),
        },
        PartReference::RFoot => AttachmentTypeBuilder::Rigid {
            target_bone: "Right Foot".into(),
        },
        PartReference::LFoot => AttachmentTypeBuilder::Rigid {
            target_bone: "Left Foot".into(),
        },

        PartReference::Tail => AttachmentTypeBuilder::Rigid {
            target_bone: "Tail".into(),
        },
    }
}
