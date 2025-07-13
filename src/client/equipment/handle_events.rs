//use super::events::WeaponEquipped;
//use bevy::prelude::*;
/*fn attach_weapon_to_bone(
    animator: &NifAnimator,
    skeleton_map: &SkeletonMap,
    commands: &mut Commands,
) {
    // Use the Animator ID to get the correct Skeleton from the global map
    if let Some(skeleton) = skeleton_map.skeletons.get(&animator.skeleton_id) {
        // get_bone_by_name to find the "Weapon Bone"
        if let Some(hand_bone) = skeleton.get_bone_by_name("Weapon Bone") {
            //    Parent the weapon mesh to it.
            info!("Attaching weapon to bone entity: {:?}", hand_bone.entity);
        //           commands
        //             .entity(event.weapon_mesh_entity)
        //           .insert(ChildOf(hand_bone.entity));
        } else {
            warn!(
                "Could not find 'Weapon Bone' in skeleton ID {}",
                animator.skeleton_id
            );
        }
    }
}*/
