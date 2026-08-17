fn main() {
    // Link the C++ standard library required by intellectual texture compressor / ispc code
    println!("cargo:rustc-link-lib=stdc++");
}
