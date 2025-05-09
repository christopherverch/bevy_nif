use bevy::prelude::*;
use std::collections::HashMap;
use std::collections::VecDeque;

use crate::NifInstantiated;
use crate::nif::animation::BoneMap;
use crate::nif::loader::Nif;
use crate::setup::SceneEntitiesByName;
use crate::setup::SceneName;
#[derive(Component)]
pub struct NeedsNifAnimator(pub Handle<Nif>);
pub fn assemble_parts(
    _: Trigger<NifInstantiated>,
    mut commands: Commands,
    all_entities_with_children: Query<&Children>,
    scene_query: Query<(Entity, &SceneName), With<SceneName>>,
    scene_entities_by_name: ResMut<SceneEntitiesByName>,
    mut transforms: Query<&mut Transform>,
    names: Query<&Name>,
    bone_map: Res<BoneMap>,
) {
    for ((scene_entity_name, unique_entity_id), _) in scene_entities_by_name.0.iter() {
        println!("scene entities: {} {}", scene_entity_name, unique_entity_id);
        for (part_scene_entity, part_scene_name) in &scene_query {
            println!(
                "part_scene entities: {} {}",
                part_scene_entity, part_scene_name.id,
            );
            if scene_entity_name.contains("skeleton") {
                println!("found skeleton with id: {}", unique_entity_id);
                if !part_scene_name.scene_name.contains("skeleton") {
                    attach_part_to_main_skeleton(
                        &mut commands,
                        &all_entities_with_children,
                        &mut transforms,
                        &names,
                        &part_scene_entity,
                        &bone_map,
                    );
                }
            }
        }
    }
}

pub fn attach_part_to_main_skeleton(
    commands: &mut Commands,
    all_entities_with_children: &Query<&Children>,
    transforms: &mut Query<&mut Transform>,
    names: &Query<&Name>,
    part_scene_entity: &Entity,
    bone_map: &Res<BoneMap>,
) {
    walk_tree(all_entities_with_children, names, part_scene_entity, 0);
    let root_bone_option = find_child_with_name_containing(
        all_entities_with_children,
        names,
        &part_scene_entity,
        "NiNode: Bip01",
    );

    if let Some(root_bone) = root_bone_option {
        let mut part_bones = HashMap::new();
        collect_bones(
            all_entities_with_children,
            names,
            &root_bone,
            &mut part_bones,
        );

        for (name, part_bone) in part_bones {
            if !name.contains("NiNode: Bip01") {
                continue;
            }
            let mut entity_commands = commands.entity(part_bone);
            let new_parent_option = bone_map.entity_map.get(&name);
            println!("FOUND BONE: {} {:?}", name, new_parent_option);

            if let Some(new_parent) = new_parent_option {
                if let Ok(mut transform) = transforms.get_mut(part_bone) {
                    println!("setting transform for {:?}", part_bone);
                }

                entity_commands.set_parent(*new_parent);
            }
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

    while let Some(curr_entity) = queue.pop_front() {
        let name_result = names.get(*curr_entity);
        if let Ok(name) = name_result {
            if format!("{}", name) == name_to_match {
                // found the named entity
                return Some(*curr_entity);
            }
        }

        let children_result = all_entities_with_children.get(*curr_entity);
        if let Ok(children) = children_result {
            for child in children {
                queue.push_back(child)
            }
        }
    }

    None
}
pub fn collect_bones(
    all_entities_with_children: &Query<&Children>,
    names: &Query<&Name>,
    root_bone: &Entity,
    collected: &mut HashMap<String, Entity>,
) {
    if let Ok(name) = names.get(*root_bone) {
        collected.insert(format!("{}", name), *root_bone);

        if let Ok(children) = all_entities_with_children.get(*root_bone) {
            for child in children {
                collect_bones(all_entities_with_children, names, child, collected)
            }
        }
    }
}
pub fn walk_tree(
    all_entities_with_children: &Query<&Children>,
    names: &Query<&Name>,
    entity: &Entity,
    depth: u32,
) {
    let mut padding = String::from("");
    for _ in 0..depth {
        padding.push_str("-")
    }

    if let Ok(name) = names.get(*entity) {
        println!("{padding}{:#?} ({:?})", name, entity)
    } else {
        println!("{padding}unnamed entity ({:?})", entity)
    }

    if let Ok(children_of_current_entity) = all_entities_with_children.get(*entity) {
        for child_entity in children_of_current_entity {
            walk_tree(all_entities_with_children, names, child_entity, depth + 1)
        }
    }
}
pub fn paint_cubes_on_joints(
    all_entities_with_children: &Query<&Children>,
    //names: &Query<&Name>,
    entity_parent: &Entity,
    mesh_query: &Query<&Mesh3d>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    global_transforms: &Query<&GlobalTransform>,
    mut commands: Commands,
) {
    //   let font = asset_server.load("fonts/FiraMono-Medium.ttf");
    let cube_h = meshes.add(Cuboid::new(0.02, 0.02, 0.02));
    let forward_mat: StandardMaterial = Color::srgb(0.1, 0.2, 0.1).into();
    let forward_mat_h = materials.add(forward_mat);
    for entity in all_entities_with_children.iter_descendants(*entity_parent) {
        if let Err(_) = mesh_query.get(entity) {
            if let Ok(_) = global_transforms.get(entity) {
                // Cubes
                let mut cube_entity = commands.spawn((
                    Mesh3d(cube_h.clone()),
                    MeshMaterial3d(forward_mat_h.clone()),
                ));
                cube_entity.set_parent(entity);
            }
        }
    }
}
