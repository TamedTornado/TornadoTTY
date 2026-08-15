fn main() {
    println!("cargo:rerun-if-env-changed=ZENTTY_BUILD_COMMIT");
}
