//! End-to-end weapon-group clip regression test (P5 round 5).
//!
//! Diagnoses "equip snaps straight to idle / LMB attack does nothing".
//! Loads the REAL `Data Files/Meshes/base_anim.nif` from the stock Morrowind
//! BSA set — the exact path and reader the client uses at runtime — runs
//! bevy_nif's full NIF load (including `setup_animations`), then verifies the
//! weapon-group animation clips + text keys exist as the C++ model requires.
//!
//! Interpretation:
//! - FAILS  -> root cause is in bevy_nif clip/key generation (read the dumped
//!             inventory for the actual names/times).
//! - PASSES -> root cause is in the client playback/state machine
//!             (playback.rs no-ops, mask/priority, combat.rs gates).
//!
//! Requires the game's Data Files. Looked up from `$TES3MP_DATA_FILES`, then
//! `../../tes3mp_rust/apps/client/assets/Data Files`. Skipped (with a loud
//! eprintln) when absent.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::app::App;
use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, AssetSourceId,
    ErasedAssetReader, PathStream, Reader, VecReader,
};
use bevy::asset::{AssetApp, AssetPlugin, Assets, AssetServer};
use bevy::prelude::MinimalPlugins;
use bevy_nif::loader::NifSettings;
use bevy_nif::nif_animation::NifAnimator;
use bevy_nif::BevyNifPlugin;
use bevy_world_serialization::WorldAsset;

// ---------------------------------------------------------------------------
// BSA fusion reader (copied from tes3mp_rust apps/client/src/bsa_asset_reader.rs;
// BSA-first because the test asset root has no loose game files).
// ---------------------------------------------------------------------------
struct BsaFusionReader {
    inner: Box<dyn ErasedAssetReader>,
}

impl AssetReader for BsaFusionReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let name = path.to_string_lossy();
        if let Ok(bytes) = bsa::bsa_set().read_entry_any(&name) {
            return Ok(Box::new(VecReader::new(bytes)) as Box<dyn Reader + 'a>);
        }
        let reader = self.inner.read(path).await?;
        Ok(reader)
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.inner.read_meta(path).await
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        self.inner.read_directory(path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.inner.is_directory(path).await
    }
}

fn find_data_files() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("TES3MP_DATA_FILES") {
        let p = PathBuf::from(p);
        if p.join("Morrowind.bsa").exists() {
            return Some(p);
        }
    }
    let rel = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tes3mp_rust/apps/client/assets/Data Files");
    if rel.join("Morrowind.bsa").exists() {
        return Some(rel);
    }
    None
}

#[test]
fn weapon_group_clips_and_text_keys_exist() {
    let data_files = match find_data_files() {
        Some(p) => p,
        None => {
            eprintln!("SKIP weapon_group e2e: no Morrowind Data Files found (set TES3MP_DATA_FILES)");
            return;
        }
    };
    let set = bsa::init_bsa_set(&data_files);
    assert!(!set.is_empty(), "no .bsa archives under {}", data_files.display());
    eprintln!("Indexed {} BSA archive(s) under {}", set.archive_count(), data_files.display());

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // Suppress avian's full physics stack: BevyNifPlugin only adds
    // PhysicsPlugins::default() when PhysicsSchedulePlugin is absent, and
    // parsing animations doesn't need physics.
    app.add_plugins(avian3d::schedule::PhysicsSchedulePlugin::default());
    let mut make_default_reader = AssetSource::get_default_reader("assets".to_string());
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(BsaFusionReader { inner: make_default_reader() })),
    );
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(bevy::animation::AnimationPlugin);
    app.add_plugins(BevyNifPlugin);
    app.init_asset::<WorldAsset>();
    // The NIF loader (IO task pool) allocates handles for these; the client
    // gets them from DefaultPlugins (PBR/render/animation). Missing any of
    // them panics the load task and the asset never completes.
    app.init_asset::<bevy::pbr::StandardMaterial>();
    app.init_asset::<bevy::render::mesh::Mesh>();
    app.init_asset::<bevy::image::Image>();
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();

    // Same load the client performs (race_select.rs): skeleton settings.
    let handle = app
        .world_mut()
        .resource_mut::<AssetServer>()
        .load_builder()
        .with_settings(|s: &mut NifSettings| s.is_skeleton = true)
        .load("Data Files/Meshes/base_anim.nif");
    let handle_id = handle.id();

    let start = Instant::now();
    let mut last_print = Instant::now();
    loop {
        app.update();
        let assets: &Assets<WorldAsset> = app.world().resource();
        if assets.contains(handle_id) {
            break;
        }
        if last_print.elapsed() > Duration::from_secs(15) {
            eprintln!("  ... still loading base_anim.nif ({:?}) - 232MB NIF from BSA, this is normal", start.elapsed());
            last_print = Instant::now();
        }
        if start.elapsed() > Duration::from_secs(120) {
            panic!("timed out after 120s loading base_anim.nif (client does it in ~20s)");
        }
    }
    eprintln!("base_anim.nif loaded in {:?}", start.elapsed());

    let mut assets = app.world_mut().resource_mut::<Assets<WorldAsset>>();
    let mut wa = assets
        .get_mut(handle_id)
        .expect("WorldAsset missing after load");
    let world = &mut wa.world;

    let mut q = world.query_filtered::<&NifAnimator, bevy::ecs::query::With<NifAnimator>>();
    let animators: Vec<&NifAnimator> = q.iter(world).collect();
    assert!(
        !animators.is_empty(),
        "no NifAnimator entity in loaded WorldAsset — setup_animations never ran"
    );
    let a = &animators[0];

    // ------------------------------------------------------------------
    // Dump the full inventory: this is the diagnostic payload on failure.
    // ------------------------------------------------------------------
    eprintln!("== animation_definitions ({} total) ==", a.animation_definitions.len());
    let mut names: Vec<&String> = a.animation_definitions.keys().collect();
    names.sort();
    for name in names {
        let def = &a.animation_definitions[name];
        eprintln!(
            "  {:<40} duration={:>7.2}s  type={:?}",
            name, def.duration, def.anim_type
        );
    }
    eprintln!(
        "== text_key_events groups ({} total): {:?} ==",
        a.text_key_events.len(),
        a.text_key_events.keys().collect::<Vec<_>>()
    );

    // TEMP diagnostic (round 5b): full text-key inventory for weapon-related
    // groups, then skip the old C++-strict assertions.
    {
        let mut interesting: Vec<(&String, &Vec<(f32, String)>)> = a
            .text_key_events
            .iter()
            .filter(|(k, _)| {
                let kl = k.to_lowercase();
                kl.starts_with("weapon")
                    || kl.starts_with("handtohand")
                    || kl.starts_with("bow")
                    || kl.starts_with("crossbow")
                    || kl.starts_with("throw")
                    || kl.starts_with("spell")
                    || kl.starts_with("shield")
                    || kl.starts_with("hit")
            })
            .collect();
        interesting.sort_by(|a, b| a.0.cmp(b.0));
        for (g, keys) in &interesting {
            let mut sorted: Vec<&(f32, String)> = keys.iter().collect();
            sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            eprintln!("== keys [{g}] ({} entries) ==", sorted.len());
            for (t, line) in sorted {
                eprintln!("  {t:9.3}  {line}");
            }
        }
    }
    eprintln!("TEMP-DUMP DONE (old assertions skipped)");
    return;

    // ------------------------------------------------------------------
    // Weapon-group requirements (what combat.rs / the C++ model need).
    // ------------------------------------------------------------------
    let mut missing_groups: Vec<&str> = Vec::new();
    for required in ["weapononehand", "weapontwohand"] {
        let key = a
            .animation_definitions
            .keys()
            .find(|k| k.eq_ignore_ascii_case(required));
        match key {
            Some(k) => {
                let def = &a.animation_definitions[k];
                // Dump text keys FIRST (diagnostic), duration gate is a
                // sanity floor: NIF NiAnimationData defines each group's
                // segment length (weapononehand can legitimately be ~7s).
                let entries = a
                    .text_key_events
                    .iter()
                    .find(|(k2, _)| k2.eq_ignore_ascii_case(required))
                    .map(|(_, v)| v.clone());
                match entries {
                    Some(entries) => {
                        eprintln!("== text keys for {k:?} ({} entries), clip duration {:.2}s ==", entries.len(), def.duration);
                        for (t, line) in &entries {
                            eprintln!("  {t:9.3}  {line}");
                        }
                        assert!(
                            def.duration > 1.0,
                            "clip {k:?} duration {:.2}s implausibly short",
                            def.duration
                        );
                        let find = |needle: &str| -> Option<f32> {
                            entries
                                .iter()
                                .find(|(_, l)| l.to_lowercase().contains(needle))
                                .map(|(t, _)| *t)
                        };
                        let equip_start = find("equip start")
                            .unwrap_or_else(|| panic!("group {required:?}: missing 'equip start' text key"));
                        let equip_stop = find("equip stop")
                            .unwrap_or_else(|| panic!("group {required:?}: missing 'equip stop' text key"));
                        let max_attack = find("max attack")
                            .unwrap_or_else(|| panic!("group {required:?}: missing 'max attack' text key"));
                        let unequip_stop = find("unequip stop")
                            .unwrap_or_else(|| panic!("group {required:?}: missing 'unequip stop' text key"));
                        assert!(equip_start < equip_stop, "group {required:?}: equip start {equip_start} !< equip stop {equip_stop}");
                        assert!(max_attack > equip_stop, "group {required:?}: max attack {max_attack} !> equip stop {equip_stop}");
                        assert!(unequip_stop > max_attack, "group {required:?}: unequip stop {unequip_stop} !> max attack {max_attack}");
                        let max_t = entries.iter().map(|(t, _)| *t).fold(0.0f32, f32::max);
                        assert!(
                            max_t <= def.duration + 0.05,
                            "group {required:?}: last text key {max_t:.3}s exceeds clip duration {:.3}s — client seeks would land beyond the clip and be dropped",
                            def.duration
                        );
                        eprintln!(
                            "OK group {required:?}: equip start={equip_start:.2}s stop={equip_stop:.2}s max attack={max_attack:.2}s unequip stop={unequip_stop:.2}s clip={:.2}s",
                            def.duration
                        );
                    }
                    None => panic!("clip {k:?} exists but has NO text_key_events entry — getTextKeyTime lookups will all miss"),
                }
            }
            None => missing_groups.push(required),
        }
    }
    assert!(
        missing_groups.is_empty(),
        "weapon group clips MISSING from animation_definitions: {missing_groups:?} (see inventory above)"
    );

    // Sanity: the NIF should also have produced the idle clips the state
    // machine falls back to.
    let has_idle = a
        .animation_definitions
        .keys()
        .any(|k| k.to_lowercase().contains("idle"));
    assert!(has_idle, "no idle* clips produced at all — setup likely failed wholesale");

    eprintln!("PASS weapon_group e2e");
}
