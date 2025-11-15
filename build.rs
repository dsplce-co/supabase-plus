fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    patch_crate::run().expect("Failed while patching");
}
