use std::collections::VecDeque;

use crate::NifInstantiated;

use super::animation::BoneMap;
use bevy::prelude::*;

#[derive(Component, Clone)]
pub enum AttachmentType {
    Skinned, // Default?
    Rigid { target_bone: String }, // e.g., Rigid { target_bone: "Bip01 Head".to_string() }
             // Maybe Morphed { target_bone: String }, later
}
pub fn attach_parts(
    _: Trigger<NifInstantiated>,
    all_entities_with_children: Query<&Children>,
    names: Query<&Name>,
    attach_query: Query<(Entity, &AttachmentType)>,
    bone_map: Res<BoneMap>,
    mut commands: Commands,
) {
    if bone_map.root_skeleton_entity.is_none() {
        return;
    }
    for (entity, attach_type) in attach_query.iter() {
        if let AttachmentType::Rigid { target_bone } = attach_type {
            let target_bone_ninode = &format!("NiNode: {}", target_bone).to_string();
            if let Some(skeleton_bone) = bone_map.entity_map.get(target_bone_ninode) {
                println!("looking for entity");
                if let Some(entity) = find_child_with_name_containing(
                    &all_entities_with_children,
                    &names,
                    &entity,
                    "NifScene",
                ) {
                    commands.entity(entity).set_parent(*skeleton_bone);
                    commands.entity(entity).remove::<AttachmentType>();
                    println!("looking for entity3");
                }
            }
        } else {
            commands.entity(entity).remove::<AttachmentType>();
        }
    }
}
pub fn find_child_with_name_containing(
    all_entities_with_children: &Query<&Children>,
    names: &Query<&Name>,
    entity: &Entity,
    name_to_match: &str,
) -> Option<Entity> {
    let mut queue = VecDeque::new();
    queue.push_back(entity);
    println!("name to match: {}", name_to_match);
    while let Some(curr_entity) = queue.pop_front() {
        let name_result = names.get(*curr_entity);
        if let Ok(name) = name_result {
            if format!("{}", name).contains(name_to_match) {
                println!("found named entity");
                // found the named entity
                if let Ok(child_entities) = all_entities_with_children.get(*curr_entity) {
                    return child_entities.first().copied();
                }
            }
        }

        let children_result = all_entities_with_children.get(*curr_entity);
        if let Ok(children) = children_result {
            for child in children {
                println!("looking for entity2");
                queue.push_back(child)
            }
        }
    }

    None
}
