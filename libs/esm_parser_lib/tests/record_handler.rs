use esm_parser_lib::{
    EsmError, EsmReader, LandTexture, Landscape, Weapon,
    types::{
        bodypart::Bodypart, cell::Cell, cell_ref::CellRef, race::Race, reference::Reference,
        static_object::Static,
    },
};

/// Main dispatch function. Determines which part to call based on the record name.
/// Returns Ok(true) if the record type was known and successfully parsed.
/// Returns Ok(false) if the record type was known but failed parsing (error logged),
/// or if the type was not handled by either part (caller should skip).
/// Returns Err(EsmError) for critical reader errors.
pub fn try_parse_and_print_record(
    reader: &mut EsmReader,
    rec_name_bytes: &[u8; 4],     // Pass the raw bytes for direct comparison
    record_id_for_messages: &str, // String version for error messages
    record_id_for_map_key: &str,  // String version for map keys (some load fns use it)
) {
    // Simple dispatch based on first letter for now, or could be more sophisticated
    // For now, just try part1, and if it doesn't handle it, try part2.
    fn print_record_details<F>(record_tag_str: &str, print_logic: F)
    where
        F: FnOnce(),
    {
        print!("  [{}] ", record_tag_str);
        print_logic();
    }
    if *rec_name_bytes == *b"LAND" {
        let mut landscape: Landscape = reader.load().unwrap();
        if landscape.grid_x == 0 && landscape.grid_y == 0 {
            landscape.load_detailed_data(reader).unwrap();
            let land = landscape.detailed_data.as_deref().unwrap();
            println!("{:?}", land.vertex_colors);
        }
    }
    if *rec_name_bytes == *b"LTEX" {
        let tex: LandTexture = reader.load().unwrap();
        println!("tex: {:?}", tex);
    }
    if *rec_name_bytes == *b"WEAP" {
        dbg!("a");
        let weap: Weapon = reader.load().unwrap();
        dbg!(weap);
    }
    if *rec_name_bytes == *b"CELL" {
        let mut cell = Cell::default();
        let mut is_deleted = false;
        let save_context = false;
        cell.load(reader, &mut is_deleted, save_context).unwrap();
        print!("cell: {:?}", cell);
        let mut cell_ref = CellRef::default();
        while Cell::get_next_ref(reader, &mut cell_ref, &mut is_deleted).unwrap() {
            dbg!(&cell_ref);
        }
    }
    if *rec_name_bytes == *b"REFR" {
        let reference: Reference = reader.load().unwrap();
        dbg!(reference);
    }
    if *rec_name_bytes == *b"STAT" {
        let static_obj: Static = reader.load().unwrap();
        dbg!(static_obj);
    }
    if *rec_name_bytes == *b"RACE" {
        let mut is_deleted = false;
        let race = Race::load(reader, &mut is_deleted).unwrap();
        dbg!(race);
    }
    if *rec_name_bytes == *b"BODY" {
        let mut is_deleted = false;
        let bodypart = Bodypart::load(reader, &mut is_deleted).unwrap();
        dbg!(bodypart);
    }
}
