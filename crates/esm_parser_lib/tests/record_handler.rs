use esm_parser_lib::{
    ESM3ExteriorCellRefId, EsmError, EsmReader, LandTexture, Landscape, RefId, Weapon,
    types::{
        bodypart::Bodypart, cell::Cell, cell_ref::CellRef, creature::Creature, gmst::GameSetting,
        magic_effect::MagicEffect, race::Race, reference::Reference, skill::Skill, spell::Spell,
        static_object::Static,
    },
};

// Loads the record and tires to print its data
use std::fmt::Debug;

pub fn process_record<F>(
    reader: &mut EsmReader,
    name: &[u8; 4],
    mut on_load: F,
) -> Result<(), EsmError>
where
    F: FnMut(&[u8; 4], &dyn Debug),
{
    let mut seyda_neen_cell = None;
    let mut is_deleted = false;

    match name {
        b"LAND" => {
            let mut landscape: Landscape = reader.load()?;
            // If it's the center cell, load the heavy vertex data
            if landscape.grid_x == 0 && landscape.grid_y == 0 {
                landscape.load_detailed_data(reader)?;
                on_load(name, &landscape);
            }
        }
        b"LTEX" => {
            let tex: LandTexture = LandTexture::load(reader, &mut is_deleted)?;
            on_load(name, &tex);
        }
        b"WEAP" => {
            let weap: Weapon = reader.load()?;
            on_load(name, &weap);
        }
        b"CELL" => {
            let mut cell = Cell::load(reader, &mut is_deleted, false)?;
            let mut cell_ref = CellRef::default();
            if cell.id == RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(-2, -9)) {
                on_load(name, &cell);
                cell.post_load(reader).unwrap();
                seyda_neen_cell = Some(cell);
                dbg!(&seyda_neen_cell);
            } else {
                while Cell::get_next_ref(reader, &mut cell_ref, &mut is_deleted)? {}
            }
        }
        b"REFR" => {
            let reference: Reference = reader.load()?;
            on_load(name, &reference);
        }
        b"STAT" => {
            let static_obj: Static = Static::load(reader, &mut is_deleted)?;
            on_load(name, &static_obj);
        }
        b"RACE" => {
            let race = Race::load(reader, &mut is_deleted)?;
            on_load(name, &race);
        }
        b"BODY" => {
            let bodypart = Bodypart::load(reader, &mut is_deleted)?;
            on_load(name, &bodypart);
        }
        b"MGEF" => {
            let mgef = MagicEffect::load(reader, &mut is_deleted)?;
            on_load(name, &mgef);
        }
        b"GMST" => {
            let gmst = GameSetting::load(reader, &mut is_deleted)?;
            // Keep your specific filter logic
            if gmst.id.resolve_string().eq_ignore_ascii_case("sAbsorb") {
                on_load(name, &gmst);
            }
        }
        b"SKIL" => {
            let skill = Skill::load(reader, &mut is_deleted)?;
            on_load(name, &skill);
        }
        b"CREA" => {
            let creature = Creature::load(reader, &mut is_deleted)?;
            on_load(name, &creature);
        }
        b"SPEL" => {
            let spell = Spell::load(reader, &mut is_deleted)?;
            on_load(name, &spell);
        }
        _ => {
            reader.skip_record()?;
        }
    }
    if let Some(seyda_neen) = seyda_neen_cell {
        seyda_neen.restore(reader, 0).unwrap();
        let mut cell_ref = CellRef::default();
        while Cell::get_next_ref(reader, &mut cell_ref, &mut is_deleted)? {
            on_load(b"REFR", &cell_ref); // Sub-records are debugged here
        }
    }
    Ok(())
}
