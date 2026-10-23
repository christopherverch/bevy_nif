# AGENTS.md — ESM Parser Library (`esm_parser_lib`)

## Overview
This crate parses Morrowind ESM/ESP binary files into strongly-typed Rust structures. It avoids unsafe raw struct pointer casts, relying instead on clean trait implementations, `bytemuck::Pod` for safe primitive reads, and explicit subrecord iteration.

---

## Core Architecture & Conventions

- **Stream Reading (`src/esm_reader.rs`)**: 
  - Manages file buffers, stream positions, and contexts via `EsmReader`.
  - Uses `reader.read_typed::<T>()` (via `bytemuck::Pod`) for safe primitive deserialization.
  - Handles string decoding using `Utf8Encoder` (`read_header_decode_string`, `read_decode_str`).
- **The `Load` Trait (`src/traits.rs`)**: 
  - Composite subrecord payloads and data structures implement the `Load` trait (e.g., `impl Load for BodypartData`).
  - Arrays or sequential bytes can be loaded via pattern destructuring (e.g., `let [a, b, c, d] = reader.load()?`).
- **Record Parsing Loop Pattern**: 
  - Records and cell references iterate over subrecords using a `while reader.has_more_subs()` loop matched against byte slices:
    ```rust
    while reader.has_more_subs() {
        let sub_name = reader.get_or_read_sub_name()?;
        match sub_name.as_bytes() {
            b"NAME" => { self.id = reader.read_refid()?; }
            b"MODL" => { self.model = reader.read_header_decode_string()?; }
            _ => {
                reader.cache_sub_name();
                break; // or return error depending on record strictness
            }
        }
    }
    ```

---

## Guidelines for Adding New Record Types

1. **Subrecord Mapping**: Follow OpenMW C++ reference specifications for subrecord tags (`NAME`, `MODL`, `DATA`, etc.) and handle missing or optional fields gracefully.
2. **Type Safety**: 
   - Use `RefId` for record references.
   - Use `Name4` for 4-byte subrecord tags.
   - Implement custom enums with bounds checking (`from_u8_unchecked`) where appropriate.
3. **Module Exports**: Ensure every new type file under `src/types/` is registered in `src/types/mod.rs` and re-exported in `src/lib.rs` if needed by downstream crates (like `client`).

---
## Strict Import Rules
- NEVER invent modules like `util`, `unified`, or `load`. They do not exist.
- Use root crate re-exports for common traits/types:
  `use crate::{EsmError, EsmReader, Load, RefId, esm_record::EsmRecord};`
- For flags or enums, import from `crate::types::{enums::*, flags::*}`.

## Verification
- Always run `cargo test` after modifying or adding record types to ensure full parser compatibility with test files.
