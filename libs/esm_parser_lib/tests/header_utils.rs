//header_utils.rs
use esm_parser_lib::Header;

pub fn print_header_information(header: &Header) {
    println!("\n--- Header Information ---");
    println!("  File Version (float): {}", header.esm_version_f());
    println!("  Author: '{}'", header.get_author());
    let desc_preview = header.data.description.chars().take(70).collect::<String>();
    println!(
        "  Description: '{}'{}",
        desc_preview,
        if header.data.description.len() > 70 {
            "..."
        } else {
            ""
        }
    );
    println!("  Number of Records: {}", header.get_record_count());

    if !header.master_data.is_empty() {
        println!("  Master Files:");
        for master in &header.master_data {
            println!("    - Name: {}, Size: {}", master.name, master.size);
        }
    }
}
