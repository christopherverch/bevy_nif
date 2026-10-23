#!/usr/bin/env bash
set -euo pipefail

# Base directories
OPENMW_DIR="/home/cv/Downloads/openmw-master/components/esm3"
CRATE_DIR="/home/cv/tes3mp_rust/crates/esm_parser_lib"
TRANS_FILE="${CRATE_DIR}/c_to_rust_translations_esm"

MAPPINGS=(
    "loadregn:region.rs"
    "loadrepa:repair.rs"
    "loadscpt:script.rs"
    "loadsndg:sound_gen.rs"
    "loadsoun:sound.rs"
    "loadsscr:start_script.rs"
)

for entry in "${MAPPINGS[@]}"; do
    cpp_prefix="${entry%%:*}"
    rust_filename="${entry##*:}"
    
    hpp_file="${OPENMW_DIR}/${cpp_prefix}.hpp"
    cpp_file="${OPENMW_DIR}/${cpp_prefix}.cpp"
    rust_file_path="${CRATE_DIR}/src/types/${rust_filename}"

    if [[ ! -f "$hpp_file" || ! -f "$cpp_file" ]]; then
        echo "[SKIP] Source files for ${cpp_prefix} not found. Skipping."
        continue
    fi

    echo "============================================================"
    echo "Processing: ${cpp_prefix} -> ${rust_filename}"
    echo "============================================================"
    touch "${rust_file_path}"

    prompt_text="Create a rust version of the C++ ${cpp_prefix} file, named ${rust_filename}. It should use the same loading and coding style as clothing.rs. Make sure to only use imports that actually exist in the project, and ask if you need something that doesn't exist. Make sure to not lose any safety checks the C++ has, such as checking if a field is not loaded, and reference 'c to rust translations esm' for the translations of function names, but use clothing.rs as the base truth for the function names and coding style. Use rust naming conventions, replace 'm_xyz' with 'xyz'. Also make sure to use the LoadSave derive as much as possible, "

aider --subtree-only \
        --read "${hpp_file}" \
        --read "${cpp_file}" \
        --read "${TRANS_FILE}" \
        --read "${CRATE_DIR}/src/types/enums.rs" \
        --read "${CRATE_DIR}/src/types/variant.rs" \
        --read "${CRATE_DIR}/src/types/clothing.rs" \
        --read "${CRATE_DIR}/src/esm_reader.rs" \
        --read "${CRATE_DIR}/src/traits.rs" \
        --file "${rust_file_path}" \
        --file "${CRATE_DIR}/src/lib.rs" \
        --file "${CRATE_DIR}/src/types/mod.rs" \
        --file "${CRATE_DIR}/src/record_handler.rs" \
        --file "${CRATE_DIR}/src/tests.rs" \
        --model "ollama_chat/qwen2.5-coder:32b" \
        --edit-format diff \
        --lint-cmd "cargo test -p esm_parser_lib -- --nocapture" \
        --no-auto-commits \
        --yes-always \
        --no-check-update \
        --no-show-model-warnings \
        --message "${prompt_text}"

    echo "Completed processing for ${rust_filename}."
    echo ""
done
