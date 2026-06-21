fn main() {
    // SYSTEM_ASSET_DIR será embutido aqui na Layer 7 (asset resolution).
    println!("cargo:rerun-if-changed=build.rs");
}
