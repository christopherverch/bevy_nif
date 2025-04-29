use crate::client::defs::{
    AnimateableEntity, FirstPersonMeshEntityLinks, SceneEntitiesByName, SceneName,
};
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use std::collections::HashMap;
use std::collections::VecDeque;

pub fn assemble_parts(
    _: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    all_entities_with_children: Query<&Children>,
    scene_query: Query<(Entity, &SceneName), With<SceneName>>,
    mut scene_entities_by_name: ResMut<SceneEntitiesByName>,
    mut transforms: Query<&mut Transform>,
    names: Query<&Name>,
) {
    println!("added gltf");
    let mut entities_to_parent: HashMap<Entity, Vec<Entity>> = HashMap::new();
    for ((scene_entity_name, unique_entity_id), _) in scene_entities_by_name.0.iter() {
        for (part_scene_entity, part_scene_name) in &scene_query {
            if part_scene_name.id != *unique_entity_id {
                //only attach meshes if the mesh id matches the player id
                continue;
            }
            if scene_entity_name.contains("casual_torso_arms.glb") {
                if let Some(arms) = find_child_with_name_containing(
                    &all_entities_with_children,
                    &names,
                    &part_scene_entity,
                    "Casual_Body_Arms",
                ) {
                    commands.entity(arms).insert(Visibility::Visible);
                    entities_to_parent
                        .entry(part_scene_name.parent)
                        .or_default()
                        .push(arms);
                    //add a link to the arms so we can unhide them
                }
            }
            if part_scene_name.scene_name.contains("sword.glb") {
                commands.entity(part_scene_entity);
                println!("parenting sword");
                entities_to_parent
                    .entry(part_scene_name.parent)
                    .or_default()
                    .push(part_scene_entity);
            }
            if scene_entity_name.contains("main_skeleton.glb") {
                println!("found skeleton with id: {}", unique_entity_id);
                let (main_skeleton_bones, main_armature_entity) =
                    get_main_skeleton_bones_and_armature(
                        &scene_entities_by_name,
                        &all_entities_with_children,
                        *unique_entity_id,
                        &names,
                    );

                if part_scene_name.scene_name.contains("sword.glb") {
                    println!("found sword entity");
                    let mut sword_entity_commands = commands.entity(part_scene_entity);
                    if let Some(handle_bone) = main_skeleton_bones.get("EquipmentHandle.R") {
                        println!("found sword entity2");
                        sword_entity_commands.set_parent(*handle_bone);
                    }
                } else if !part_scene_name.scene_name.contains("main_skeleton.glb") {
                    attach_part_to_main_skeleton(
                        &mut commands,
                        &all_entities_with_children,
                        &mut transforms,
                        &names,
                        &part_scene_name.scene_name,
                        &part_scene_entity,
                        &main_armature_entity,
                        &main_skeleton_bones,
                    );
                }
            }
        }
    }
    for (parent, entities) in entities_to_parent {
        commands
            .entity(parent)
            .insert(FirstPersonMeshEntityLinks(entities));
        scene_entities_by_name.0.clear();
    }
}

pub fn get_main_skeleton_bones_and_armature(
    scene_entities_by_name: &ResMut<SceneEntitiesByName>,
    all_entities_with_children: &Query<&Children>,
    entity_id: u64,
    names: &Query<&Name>,
) -> (HashMap<String, Entity>, Entity) {
    let mut main_bones = HashMap::new();

    let main_skeleton_entity = scene_entities_by_name
        .0
        .get(&("models/main_skeleton.glb".to_string(), entity_id))
        .expect("did not have the main skeleton scene");

    let root_bone = find_child_with_name_containing(
        all_entities_with_children,
        &names,
        &main_skeleton_entity,
        "Root",
    )
    .expect("the skeleton to have a bone called 'Root'");

    let main_skeleton_armature = find_child_with_name_containing(
        all_entities_with_children,
        &names,
        &main_skeleton_entity,
        "CharacterArmature",
    )
    .expect("the skeleton to have an armature");

    collect_bones(
        all_entities_with_children,
        &names,
        &root_bone,
        &mut main_bones,
    );

    //println!("main bones: {:#?}", main_bones);

    (main_bones, main_skeleton_armature)
}
pub fn attach_part_to_main_skeleton(
    commands: &mut Commands,
    all_entities_with_children: &Query<&Children>,
    transforms: &mut Query<&mut Transform>,
    names: &Query<&Name>,
    part_scene_name: &String,
    part_scene_entity: &Entity,
    main_armature_entity: &Entity,
    main_skeleton_bones: &HashMap<String, Entity>,
) {
    println!("attaching part: {}", part_scene_name);

    let root_bone_option = find_child_with_name_containing(
        all_entities_with_children,
        names,
        &part_scene_entity,
        "Root",
    );

    let part_armature_option = find_child_with_name_containing(
        all_entities_with_children,
        names,
        &part_scene_entity,
        "CharacterArmature",
    );

    if let Some(part_armature) = part_armature_option {
        let mut part_armature_entity_commands = commands.entity(part_armature);
        if let Ok(mut transform) = transforms.get_mut(part_armature) {
            transform.translation.x = 0.0;
            transform.translation.y = 0.0;
            transform.translation.z = 0.0;
            transform.rotation = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        }

        part_armature_entity_commands.set_parent(*main_armature_entity);
    }

    if let Some(root_bone) = root_bone_option {
        let mut part_bones = HashMap::new();
        collect_bones(
            all_entities_with_children,
            names,
            &root_bone,
            &mut part_bones,
        );

        for (name, part_bone) in part_bones {
            let mut entity_commands = commands.entity(part_bone);
            let new_parent_option = main_skeleton_bones.get(&name);

            if let Some(new_parent) = new_parent_option {
                if let Ok(mut transform) = transforms.get_mut(part_bone) {
                    transform.translation.x = 0.0;
                    transform.translation.y = 0.0;
                    transform.translation.z = 0.0;
                    transform.rotation = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
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
            if format!("{}", name).contains(name_to_match) {
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
/*pub fn walk_tree(
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
*/
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

#[derive(Component, Debug)]
pub struct AnimationEntityLink(pub Entity);

pub fn get_top_parent(
    mut curr_entity: Entity,
    all_entities_with_parents_query: &Query<&Parent>,
    all_entities_with_animateable_query: &Query<Entity, With<AnimateableEntity>>,
) -> Entity {
    //Loop up all the way to the top parent
    loop {
        if let Ok(ref_to_parent) = all_entities_with_parents_query.get(curr_entity) {
            //if the parent has AnimateableEntity component
            curr_entity = ref_to_parent.get();
            if (all_entities_with_animateable_query.get(ref_to_parent.get())).is_ok() {
                break;
            }
        } else {
            break;
        }
    }
    curr_entity
}

pub fn insert_animation_links(
    animation_players_query: Query<Entity, Added<AnimationPlayer>>,
    all_entities_with_parents_query: Query<&Parent>,
    all_entities_with_animateable_query: Query<Entity, With<AnimateableEntity>>,
    animations_entity_link_query: Query<&AnimationEntityLink>,
    mut commands: Commands,
) {
    // Get all the Animation players which can be deep and hidden in the heirachy
    for entity_with_animation_player in animation_players_query.iter() {
        let top_entity = get_top_parent(
            entity_with_animation_player,
            &all_entities_with_parents_query,
            &all_entities_with_animateable_query,
        );

        // If the top parent has an animation config ref then link the player to the config
        if animations_entity_link_query.get(top_entity).is_ok() {
            warn!("Problem with multiple animation players for the same top parent");
        } else {
            println!(
                "linking entity {:#?} to animation_player entity {:#?}",
                top_entity, entity_with_animation_player
            );
            commands
                .entity(top_entity)
                .insert(AnimationEntityLink(entity_with_animation_player.clone()));
        }
    }
}
