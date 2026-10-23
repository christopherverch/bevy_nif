use esm_parser_lib::{
    ESM3ExteriorCellRefId, EsmError, EsmReader, LandTexture, Landscape, RefId, Weapon,
    types::{
        activator::Activator, apparatus::Apparatus, armor::Armor, birthsign::BirthSign,
        bodypart::Bodypart, book::Book, cell::Cell, cell_ref::CellRef, class::Class,
        clothing::Clothing, creature::Creature, enchantment::Enchantment, enums::ApparatusType,
        gmst::GameSetting, info::DialInfo, ingredient::Ingredient, light::Light,
        lockpick::Lockpick, magic_effect::MagicEffect, miscellaneous::Miscellaneous,
        pathgrid::Pathgrid, probe::Probe, race::Race, reference::Reference, skill::Skill,
        spell::Spell, static_object::Static,
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
    macro_rules! load_rec {
        ($t:ty) => {{
            let rec = <$t>::load(reader, &mut is_deleted)?;
            on_load(name, &rec);
        }};
        (direct $t:ty) => {{
            let rec: $t = reader.load()?;
            on_load(name, &rec);
        }};
    }

    match name {
        b"LAND" => {
            let mut landscape: Landscape = reader.load()?;
            if landscape.grid_x == 0 && landscape.grid_y == 0 {
                landscape.load_detailed_data(reader)?;
                on_load(name, &landscape);
            }
        }
        b"LTEX" => load_rec!(LandTexture),
        b"WEAP" => load_rec!(direct Weapon),
        b"ARMO" => load_rec!(Armor),
        b"STAT" => load_rec!(Static),
        b"RACE" => load_rec!(Race),
        b"BODY" => load_rec!(Bodypart),
        b"MGEF" => load_rec!(MagicEffect),
        b"SKIL" => load_rec!(Skill),
        b"CREA" => load_rec!(Creature),
        b"SPEL" => load_rec!(Spell),
        b"REFR" => load_rec!(direct Reference),
        b"ACTI" => load_rec!(Activator),
        b"CLOT" => load_rec!(Clothing),
        b"BOOK" => load_rec!(Book),
        b"BSGN" => load_rec!(BirthSign),
        b"APPA" => load_rec!(Apparatus),
        b"CLAS" => load_rec!(Class),
        b"ENCH" => load_rec!(Enchantment),
        b"INFO" => load_rec!(DialInfo),
        b"INGR" => load_rec!(Ingredient),
        b"LIGH" => load_rec!(Light),
        b"LOCK" => load_rec!(Lockpick),
        b"MISC" => load_rec!(Miscellaneous),
        b"PGRD" => load_rec!(Pathgrid),
        b"PROB" => load_rec!(Probe),
        b"GMST" => {
            let gmst = GameSetting::load(reader, &mut is_deleted)?;
            if gmst.id.resolve_string().eq_ignore_ascii_case("sAbsorb") {
                on_load(name, &gmst);
            }
        }
        b"CELL" => {
            let cell = Cell::load(reader, &mut is_deleted, false)?;
            if cell.id == RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(-2, -9)) {
                on_load(name, &cell);
                let mut seyda_neen = cell;
                seyda_neen.post_load(reader).unwrap();
                seyda_neen_cell = Some(seyda_neen);
            } else {
                let mut cell_ref = CellRef::default();
                while Cell::get_next_ref(reader, &mut cell_ref, &mut is_deleted)? {}
            }
        }
        _ => {
            reader.skip_record()?;
        }
    }

    if let Some(seyda_neen) = seyda_neen_cell {
        seyda_neen.restore(reader, 0).unwrap();
        let mut cell_ref = CellRef::default();
        while Cell::get_next_ref(reader, &mut cell_ref, &mut is_deleted)? {
            on_load(b"REFR", &cell_ref);
        }
    }
    Ok(())
}
