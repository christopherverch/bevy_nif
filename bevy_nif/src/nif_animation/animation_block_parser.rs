//! NIF text-key parsing: flatten the raw `NiTextKey` command lines, find
//! animation block boundaries, and split each block into the
//! `ProcessedAnimation` clips that `setup_animations` turns into Bevy
//! `AnimationClip`s.
//!
//! Deliberately Bevy-free: only the `nif` crate's text-key types and the
//! shared `bevy_types` (AnimType, CombatAnimData) are used here.

use crate::nif_animation::bevy_types::{AnimType, CombatAnimData};
use nif::NiTextKey;
use std::collections::{HashMap, HashSet};

/// A flattened, simplified representation of a single command from a text key.
#[derive(Debug, Clone)]
struct ParsedKeyEvent {
    time: f32,
    original_line: String,
    name: String,
    command: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ProcessedAnimation {
    pub name: String,
    pub start_time: f32,
    pub end_time: f32,
    pub events: Vec<String>,
    pub min_attack_time_relative: f32,
    pub max_attack_time_relative: f32,
    pub hit_time_relative: f32,
    pub min_hit_time_relative: f32,
    pub anim_type: AnimType,
}

/// Build the optional combat timing data for a processed clip.
///
/// `Some` when the NIF supplied at least one combat text key for this clip
/// ("min attack", "max attack", "hit", "min hit"), `None` for non-attack
/// clips (idle, equip, loops, ...). Values are clip-local seconds measured
/// from the clip's own start (same convention as the `*_relative` fields).
pub(crate) fn combat_anim_data_for(clip: &ProcessedAnimation) -> Option<CombatAnimData> {
    let min_attack_time_relative = clip.min_attack_time_relative;
    let max_attack_time_relative = clip.max_attack_time_relative;
    let hit_time_relative = clip.hit_time_relative;
    let min_hit_time_relative = clip.min_hit_time_relative;
    (min_attack_time_relative > 0.0
        || max_attack_time_relative > 0.0
        || hit_time_relative > 0.0
        || min_hit_time_relative > 0.0)
        .then_some(CombatAnimData {
            min_attack_time_relative,
            max_attack_time_relative,
            hit_time_relative,
            min_hit_time_relative,
        })
}

/// A temporary struct to hold all data about a block before the final split decision.
#[derive(Debug, Clone)]
struct AnimationBlockData {
    name: String,
    start_time: f32,
    end_time: f32,
    loop_start_time: Option<f32>,
    loop_end_time: Option<f32>,
    min_attack_time: Option<f32>,
    max_attack_time: Option<f32>,
    hit_time: Option<f32>,
    min_hit_time: Option<f32>,
}

// A list of all possible commands we need to recognize.
// Order is important for commands that are subsets of others (e.g., "shoot follow" before "shoot").
const KNOWN_COMMANDS: &[&str] = &[
    // Most specific commands first
    "large follow start",
    "large follow stop",
    "medium follow start",
    "medium follow stop",
    "small follow start",
    "small follow stop",
    "shoot follow start",
    "shoot follow stop",
    "follow start",
    "follow stop",
    // "equip start"/"equip stop" and "unequip start"/"unequip stop" are real
    // text keys (C++ character.cpp queries them for equip/unequip plays), NOT
    // bare boundary markers — list them before "start"/"stop" so parse_line
    // keeps the full command instead of collapsing it to "start"/"stop".
    "unequip start",
    "unequip stop",
    "equip start",
    "equip stop",
    "loop start",
    "loop stop",
    "min attack",
    "max attack",
    "min hit",
    "hit",
    // Round 8: per-attack-type windup start keys ("WeaponOneHand: Slash Start",
    // "HandToHand: Chop Start", ...). Listed before the bare "start" so
    // parse_line keeps the full command; these are real gameplay text keys
    // (the windup start point, C++ updateWeaponState "start" seek), NOT bare
    // boundary markers.
    "slash start",
    "chop start",
    "thrust start",
    "shoot start",
    // Less specific commands last
    "large follow",
    "medium follow",
    "small follow",
    "shoot follow",
    "start",
    "stop",
    "shoot",
    "block",
    "equip",
    "unequip",
];

fn parse_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let lower_line = line.to_lowercase();

    for &cmd in KNOWN_COMMANDS {
        if lower_line.ends_with(cmd) {
            let cmd_start_index = lower_line.len() - cmd.len();
            // Ensure we are matching a whole word command, not just a partial suffix.
            if cmd_start_index == 0 || lower_line.as_bytes().get(cmd_start_index - 1) == Some(&b' ')
            {
                // Trim whitespace, then remove the colon.
                let mut name_part = line[..cmd_start_index].trim().trim_end_matches(':').trim();
                // Qualified keys (e.g. "WeaponTwoHand: Slash Max Attack") must map
                // to the plain group name ("WeaponTwoHand"): each group is one
                // continuous timeline (C++ model), so middle segments belong to
                // the key, not to the name. Mirrors the fallback below.
                if let Some(pos) = name_part.find(':') {
                    name_part = name_part[..pos].trim();
                }
                if !name_part.is_empty() {
                    return Some((name_part.to_string(), cmd.to_string()));
                }
            }
        }
    }

    // Fallback for simple "Group:Action" lines like "SoundGen:Left"
    if let Some(pos) = line.find(':') {
        let name = line[..pos].trim().to_string();
        let command = line[pos + 1..].trim().to_lowercase();
        if !name.is_empty() && !command.is_empty() {
            return Some((name, command));
        }
    }

    None
}

/// Flatten every text-key line, group lines into animation blocks by their
/// clip-defining boundary keys, and split each block into final clips:
/// loops (base/_loop/_outro) and attacks (base/_windup/_release/_follow).
pub(crate) fn parse_and_split_animation_blocks(
    nif_keys: &[NiTextKey],
) -> Vec<ProcessedAnimation> {
    // --- Pass 1: Flatten all text key lines into a single, time-sorted list ---
    let mut all_events: Vec<ParsedKeyEvent> = Vec::new();
    for key in nif_keys {
        for line in key.value.lines() {
            if let Some((name, command)) = parse_line(line) {
                all_events.push(ParsedKeyEvent {
                    time: key.time,
                    original_line: line.trim().to_string(),
                    name,
                    command,
                });
            }
        }
    }
    all_events.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

    // --- Pass 2: Identify "real" animation clips ---
    let mut clip_defining_names = HashSet::new();
    for event in &all_events {
        // Equip/unequip start/stop are real keys (kept queryable by parse_line)
        // but also serve as the block's defining boundary lines.
        if event.command == "start"
            || event.command == "stop"
            || event.command == "equip start"
            || event.command == "equip stop"
            || event.command == "unequip start"
            || event.command == "unequip stop"
            || event.command == "slash start"
            || event.command == "chop start"
            || event.command == "thrust start"
            || event.command == "shoot start"
        {
            clip_defining_names.insert(event.name.clone());
        }
    }

    // --- Pass 3: Determine the boundaries for only the real clips ---
    let mut temp_blocks: HashMap<String, AnimationBlockData> = HashMap::new();
    for event in &all_events {
        let parent_clip_name = clip_defining_names
            .iter()
            .filter(|&def_name| event.name.starts_with(def_name))
            .max_by_key(|def_name| def_name.len());

        if let Some(clip_name) = parent_clip_name {
            let block =
                temp_blocks
                    .entry(clip_name.clone())
                    .or_insert_with(|| AnimationBlockData {
                        name: clip_name.clone().to_lowercase(),
                        start_time: f32::MAX,
                        end_time: f32::MIN,
                        loop_start_time: None,
                        loop_end_time: None,
                        min_attack_time: None,
                        max_attack_time: None,
                        hit_time: None,
                        min_hit_time: None,
                    });

            block.start_time = block.start_time.min(event.time);
            block.end_time = block.end_time.max(event.time);

            match event.command.as_str() {
                "loop start" => block.loop_start_time = Some(event.time),
                "loop stop" => block.loop_end_time = Some(event.time),
                "min attack" => block.min_attack_time = Some(event.time),
                "max attack" => block.max_attack_time = Some(event.time),
                "hit" => block.hit_time = Some(event.time),
                "min hit" => block.min_hit_time = Some(event.time),
                // Follow start/stop keys are deliberately not collected: the
                // per-attack-type follow phases are seek targets in the base
                // clip's text-key events (see pass 4 comment).
                _ => {} // Ignore other commands like "start", "stop" here
            }
        }
    }

    // --- Pass 4: Split clips ---
    let mut final_animations: Vec<ProcessedAnimation> = Vec::new();
    let temp_blocks_vec: Vec<AnimationBlockData> = temp_blocks.into_values().collect();

    for block_data in temp_blocks_vec {
        // If it's a loop, split it into base animation, loop, and outro
        // _loop appended to loop animations, _outro appended to outro animations
        if let (Some(loop_start), Some(loop_end)) =
            (block_data.loop_start_time, block_data.loop_end_time)
        {
            if loop_end >= loop_start {
                if loop_start > block_data.start_time {
                    final_animations.push(ProcessedAnimation {
                        name: block_data.name.clone(),
                        start_time: block_data.start_time,
                        end_time: loop_start,
                        events: Vec::new(),
                        min_attack_time_relative: 0.0,
                        max_attack_time_relative: 0.0,
                        hit_time_relative: 0.0,
                        min_hit_time_relative: 0.0,
                        anim_type: AnimType::OneShot,
                    });
                }
                final_animations.push(ProcessedAnimation {
                    name: format!("{}_loop", block_data.name),
                    start_time: loop_start,
                    end_time: loop_end,
                    events: Vec::new(),
                    min_attack_time_relative: 0.0,
                    max_attack_time_relative: 0.0,
                    hit_time_relative: 0.0,
                    min_hit_time_relative: 0.0,
                    anim_type: AnimType::Loop,
                });
                if block_data.end_time > loop_end {
                    final_animations.push(ProcessedAnimation {
                        name: format!("{}_outro", block_data.name),
                        start_time: loop_end,
                        end_time: block_data.end_time,
                        events: Vec::new(),
                        min_attack_time_relative: 0.0,
                        max_attack_time_relative: 0.0,
                        hit_time_relative: 0.0,
                        min_hit_time_relative: 0.0,
                        anim_type: AnimType::Outro,
                    });
                }
                continue;
            }
        } else if block_data.min_attack_time.is_some() {
            let base_name = &block_data.name;

            // --- 0. Full-block base clip (start -> stop) ---
            // Mirrors OpenMW semantics: the game plays a weapon animation as one
            // continuous timeline (e.g. "sword1"), seeking by text key times
            // (updateWeaponState: windup from "start", release replay from
            // "max attack"). Sub-clips below are kept for segmented consumers.
            final_animations.push(ProcessedAnimation {
                name: base_name.clone(),
                start_time: block_data.start_time,
                end_time: block_data.end_time,
                events: Vec::new(),
                min_attack_time_relative: block_data
                    .min_attack_time
                    .map_or(0.0, |t| t - block_data.start_time),
                max_attack_time_relative: block_data
                    .max_attack_time
                    .map_or(0.0, |t| t - block_data.start_time),
                hit_time_relative: block_data
                    .hit_time
                    .map_or(0.0, |t| t - block_data.start_time),
                min_hit_time_relative: block_data
                    .min_hit_time
                    .map_or(0.0, |t| t - block_data.start_time),
                anim_type: AnimType::OneShot,
            });

            // --- 1. Create Windup Clip (if it exists) ---
            let release_start_time = if let (Some(min_attack), Some(max_attack)) =
                (block_data.min_attack_time, block_data.max_attack_time)
            {
                final_animations.push(ProcessedAnimation {
                    name: format!("{}_windup", base_name),
                    start_time: block_data.start_time,
                    end_time: max_attack,
                    events: Vec::new(),
                    min_attack_time_relative: min_attack - block_data.start_time,
                    max_attack_time_relative: max_attack - block_data.start_time,
                    hit_time_relative: 0.0,
                    min_hit_time_relative: 0.0,
                    anim_type: AnimType::Windup,
                });
                max_attack
            } else {
                block_data.start_time
            };

            // --- 2. Create Release/Shoot Clip ---
            // The release ends at the hit; without a hit key it runs to the
            // block end. Per-attack-type follow phases (large/medium/small
            // "follow start/stop" text keys) are deliberately NOT split into
            // sub-clips: those keys are not prefixed by attack type, so a
            // weapon with several attack cycles collides across cycles and
            // produced degenerate 0-duration clips (e.g. the follow START of
            // the last cycle paired with the follow STOP of the first). They
            // remain seek targets in the base OneShot's text-key events; the
            // attack-release logic picks the follow by attack type at release.
            let release_end_time = block_data.hit_time.unwrap_or(block_data.end_time);

            // Calculate relative times and add them to the _release clip.
            let mut hit_time_rel = 0.0;
            let mut min_hit_time_rel = 0.0;
            if let Some(hit_time) = block_data.hit_time {
                hit_time_rel = hit_time - release_start_time;
                if let Some(min_hit) = block_data.min_hit_time {
                    if min_hit < hit_time {
                        min_hit_time_rel = min_hit - release_start_time;
                    }
                }
            }

            final_animations.push(ProcessedAnimation {
                name: format!("{}_release", base_name),
                start_time: release_start_time,
                end_time: release_end_time,
                events: Vec::new(),
                min_attack_time_relative: 0.0,
                max_attack_time_relative: 0.0,
                hit_time_relative: hit_time_rel, // Add calculated value
                min_hit_time_relative: min_hit_time_rel, // Add calculated value
                anim_type: AnimType::Release,
            });

            // --- 3. Create Follow clip (post-hit settle + return to ready) ---
            // Covers everything after the release so the segmented chain
            // windup -> release -> follow partitions the full timeline with
            // no gaps. The per-attack-type follow phases live inside it and
            // are reachable via the base clip's text-key events.
            if block_data.end_time > release_end_time + 1e-4 {
                final_animations.push(ProcessedAnimation {
                    name: format!("{}_follow", base_name),
                    start_time: release_end_time,
                    end_time: block_data.end_time,
                    events: Vec::new(),
                    min_attack_time_relative: 0.0,
                    max_attack_time_relative: 0.0,
                    hit_time_relative: 0.0,
                    min_hit_time_relative: 0.0,
                    anim_type: AnimType::Follow,
                });
            }

            continue; // Go to the next animation block
        }
        // If it's not a looping anim or a main attack anim, treat it as a single clip.
        // This will handle animations like "equip", "unequip", "hit", "death"
        final_animations.push(ProcessedAnimation {
            name: block_data.name,
            start_time: block_data.start_time,
            end_time: block_data.end_time,
            events: Vec::new(),
            min_attack_time_relative: 0.0,
            max_attack_time_relative: 0.0,
            hit_time_relative: 0.0,
            min_hit_time_relative: 0.0,
            anim_type: AnimType::OneShot,
        });
    }
    all_events.sort_unstable_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    final_animations.sort_unstable_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    let mut anim_index = 0;
    // --- Pass 5: Populate the final animation with the events that actually correspond to it
    for event in &all_events {
        // Skip boundary markers, as they are not gameplay events.
        if event.command == "start"
            || event.command == "stop"
            || event.command == "loop start"
            || event.command == "loop stop"
        {
            continue;
        }

        let mut event_placed = false;
        // Advance past any animations that end before this event even starts
        while anim_index < final_animations.len()
            && final_animations[anim_index].end_time < event.time - 1e-4
        {
            anim_index += 1;
        }
        // Stage 1: Try to find a parent clip by name.
        if let Some(parent_clip_name) = clip_defining_names
            .iter()
            .filter(|&def_name| event.name.starts_with(def_name))
            .max_by_key(|def_name| def_name.len())
        {
            for i in anim_index..final_animations.len() {
                let anim = &mut final_animations[i];
                // Break early if the animation starts after the event time
                if anim.start_time > event.time + 1e-4 {
                    break;
                }
                // A clip belongs to the block if it is the base clip or a
                // sub-clip (e.g. "sword1", "sword1_loop", "sword1_windup",
                // "sword1_release"). Assign to EVERY
                // matching clip in the event's time window so the full-block
                // base clip and its sub-clips both carry the events.
                // Case-insensitive: anim.name is lowercased (Pass 1) while
                // clip_defining_names keeps the NIF's original case.
                let parent_lower = parent_clip_name.to_lowercase();
                let in_block =
                    anim.name == parent_lower || anim.name.starts_with(&format!("{parent_lower}_"));
                if in_block
                    && event.time >= anim.start_time - 1e-4
                    && event.time <= anim.end_time + 1e-4
                {
                    let event_string = format!("'{}' @ {:.3}", event.original_line, event.time);
                    anim.events.push(event_string);
                    event_placed = true;
                }
            }
        }

        // Stage 2: Fallback for events with no name-based parent (e.g., "SoundGen").
        if !event_placed {
            for i in anim_index..final_animations.len() {
                let anim = &mut final_animations[i];
                // Break early if the animation starts after the event time
                if anim.start_time > event.time + 1e-4 {
                    break;
                }
                if event.time >= anim.start_time - 1e-4 && event.time <= anim.end_time + 1e-4 {
                    let event_string = format!("'{}' @ {:.3}", event.original_line, event.time);
                    anim.events.push(event_string);
                    break;
                }
            }
        }
    }

    // Final sorting for consistency.
    for anim in &mut final_animations {
        anim.events.sort();
    }

    final_animations
}

#[cfg(test)]
mod weapon_group_parse_tests {
    use super::parse_line;

    #[test]
    fn qualified_weapon_group_commands_map_to_group_name() {
        // P5: qualified per-attack keys ("WeaponTwoHand: Slash Max Attack")
        // must map to the plain group name so pass 3 groups them into one
        // continuous clip (C++ model).
        assert_eq!(
            parse_line("WeaponTwoHand: Equip Start"),
            Some(("WeaponTwoHand".to_string(), "equip start".to_string()))
        );
        assert_eq!(
            parse_line("WeaponTwoHand: Slash Max Attack"),
            Some(("WeaponTwoHand".to_string(), "max attack".to_string()))
        );
        assert_eq!(
            parse_line("WeaponOneHand: Slash Min Hit"),
            Some(("WeaponOneHand".to_string(), "min hit".to_string()))
        );
        assert_eq!(
            parse_line("WeaponOneHand: Slash Start"),
            Some(("WeaponOneHand".to_string(), "slash start".to_string()))
        );
        assert_eq!(
            parse_line("HandToHand: Chop Start"),
            Some(("HandToHand".to_string(), "chop start".to_string()))
        );
        assert_eq!(
            parse_line("WeaponTwoHand: Slash Follow Stop"),
            Some(("WeaponTwoHand".to_string(), "follow stop".to_string()))
        );
        assert_eq!(
            parse_line("WeaponBow: Bow Follow Start"),
            Some(("WeaponBow".to_string(), "follow start".to_string()))
        );
        assert_eq!(
            parse_line("WeaponShield: Unequip Stop"),
            Some(("WeaponShield".to_string(), "unequip stop".to_string()))
        );
    }

    #[test]
    fn simple_and_unknown_lines_unchanged() {
        assert_eq!(
            parse_line("Idle: Loop Start"),
            Some(("Idle".to_string(), "loop start".to_string()))
        );
        assert_eq!(
            parse_line("SoundGen:Left"),
            Some(("SoundGen".to_string(), "left".to_string()))
        );
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("NiTextKeyExtraData"), None);
    }
}

#[cfg(test)]
mod combat_anim_data_tests {
    use super::{combat_anim_data_for, parse_and_split_animation_blocks, ProcessedAnimation};
    use crate::nif_animation::bevy_types::AnimType;
    use nif::NiTextKey;

    fn clip(times: (f32, f32, f32, f32)) -> ProcessedAnimation {
        ProcessedAnimation {
            name: "test".to_string(),
            start_time: 0.0,
            end_time: 1.0,
            events: Vec::new(),
            min_attack_time_relative: times.0,
            max_attack_time_relative: times.1,
            hit_time_relative: times.2,
            min_hit_time_relative: times.3,
            anim_type: AnimType::OneShot,
        }
    }

    #[test]
    fn attack_clip_produces_some_combat_data() {
        let data = combat_anim_data_for(&clip((0.3, 0.7, 1.1, 0.5))).unwrap();
        assert_eq!(
            (data.min_attack_time_relative, data.max_attack_time_relative),
            (0.3, 0.7)
        );
        assert_eq!((data.hit_time_relative, data.min_hit_time_relative), (1.1, 0.5));
    }

    #[test]
    fn release_clip_with_only_hit_is_some() {
        // _release clips carry hit/min_hit only (attack times live on the
        // full-block and _windup clips).
        let data = combat_anim_data_for(&clip((0.0, 0.0, 0.25, 0.1))).unwrap();
        assert_eq!(data.hit_time_relative, 0.25);
        assert_eq!(data.min_hit_time_relative, 0.1);
        assert_eq!(data.min_attack_time_relative, 0.0);
    }

    #[test]
    fn non_attack_clip_is_none() {
        assert!(combat_anim_data_for(&clip((0.0, 0.0, 0.0, 0.0))).is_none());
    }

    #[test]
    fn parse_extracts_combat_times_into_full_block_clip() {
        fn tk(time: f32, line: &str) -> NiTextKey {
            NiTextKey {
                time,
                value: line.to_string(),
            }
        }
        let keys = vec![
            tk(0.0, "WeaponOneHand: Slash Start"),
            tk(0.4, "WeaponOneHand: Slash Min Attack"),
            tk(0.9, "WeaponOneHand: Slash Max Attack"),
            tk(1.0, "WeaponOneHand: Slash Min Hit"),
            tk(1.2, "WeaponOneHand: Slash Hit"),
            tk(1.8, "WeaponOneHand: Stop"),
        ];
        let anims = parse_and_split_animation_blocks(&keys);
        let base = anims
            .iter()
            .find(|a| a.name == "weapononehand")
            .expect("full-block clip exists");
        assert!((base.min_attack_time_relative - 0.4).abs() < 1e-4);
        assert!((base.max_attack_time_relative - 0.9).abs() < 1e-4);
        assert!((base.hit_time_relative - 1.2).abs() < 1e-4);
        assert!((base.min_hit_time_relative - 1.0).abs() < 1e-4);

        // And the Some/None mapping for the sub-clips.
        assert!(combat_anim_data_for(base).is_some());
        let windup = anims
            .iter()
            .find(|a| a.name == "weapononehand_windup")
            .expect("windup clip exists");
        assert!(combat_anim_data_for(windup).is_some());
        let follow = anims
            .iter()
            .find(|a| a.name == "weapononehand_follow")
            .expect("follow clip exists");
        assert!(combat_anim_data_for(follow).is_none());
    }
}
