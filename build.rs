fn main() {
    // Ensure adding or editing an embedded migration triggers a rebuild.
    println!("cargo:rerun-if-changed=migrations");
}
