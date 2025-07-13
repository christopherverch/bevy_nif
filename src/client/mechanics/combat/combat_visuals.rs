use bevy::prelude::*;
use cdda_bevy::client::actor_defs::{ActorState, UpperBodyState};

// Marker component for the weapon entity attached to the character's hand.
#[derive(Component)]
pub struct WieldedWeapon;

// Marker component for the weapon entity attached to the character's hip/back.
#[derive(Component)]
pub struct HolsteredWeapon;

pub struct CombatVisualsPlugin;

impl Plugin for CombatVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_weapon_visibility);
    }
}

/// This system reads the character's state and sets the visibility
/// of the WieldedWeapon and HolsteredWeapon entities accordingly.
fn update_weapon_visibility(
    // Query for the parent actor that has the state
    actor_query: Query<&ActorState>,
    mut wielded_query: Query<
        (&mut Visibility, &ChildOf),
        (With<WieldedWeapon>, Without<HolsteredWeapon>),
    >,
    mut holstered_query: Query<
        (&mut Visibility, &ChildOf),
        (With<HolsteredWeapon>, Without<WieldedWeapon>),
    >,
) {
    // We can iterate through wielded weapons and find their holstered siblings.
    // This is less efficient than querying actors, but works for a single player.
    for (mut wielded_vis, wielded_childof) in wielded_query.iter_mut() {
        let parent_entity = wielded_childof.0;

        // Get the ActorState from the parent entity
        if let Ok(actor_state) = actor_query.get(parent_entity) {
            // Now, find the corresponding holstered weapon that is also a child of this parent.
            for (mut holstered_vis, holstered_childof) in holstered_query.iter_mut() {
                // If this holstered weapon belongs to the same parent
                if holstered_childof.0 == parent_entity {
                    match actor_state.combat.upper_body_state {
                        // When the weapon is equipped, show the wielded model and hide the holstered one.
                        UpperBodyState::WeaponEquipped => {
                            *wielded_vis = Visibility::Visible;
                            *holstered_vis = Visibility::Hidden;
                        }
                        // When the weapon is sheathed, hide the wielded model and show the holstered one.
                        UpperBodyState::None => {
                            *wielded_vis = Visibility::Hidden;
                            *holstered_vis = Visibility::Visible;
                        }
                        // During the transition, do nothing and wait for the state to settle.
                        // The visibility will be updated once the state becomes WeaponEquipped or None.
                        _ => {}
                    }
                    // Once we've found and updated the matching holstered weapon,
                    // we can break out of the inner loop.
                    break;
                }
            }
        }
    }
}
