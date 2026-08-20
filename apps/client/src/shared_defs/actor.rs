use esm_parser_lib::types::magic_effect::MagicEffect;

use crate::{
    defs::WorldSettings, mechanics::creature_stats::CreatureStats,
    shared_defs::container::Container,
};

pub struct Actor;
impl Actor {
    pub fn get_encumbrance(
        creature_stats: &CreatureStats,
        actor_container: &mut Container,
        is_player: bool,
        world_settings: &WorldSettings,
    ) -> f32 {
        let mut weight = actor_container.get_weight();
        weight -= creature_stats
            .magic_effects
            .get_or_default(MagicEffect::FEATHER)
            .get_magnitude();
        if !is_player && !world_settings.god_mode {
            weight += creature_stats
                .magic_effects
                .get_or_default(MagicEffect::BURDEN)
                .get_magnitude();
        }
        if weight < 0.0 { 0.0 } else { weight }
    }
}
