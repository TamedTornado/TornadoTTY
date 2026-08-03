use std::env;
use std::path::PathBuf;

fn main() {
    let workspace =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory")).join("../..");
    let library_directory = env::var_os("GHOSTTY_LIB_DIR").map_or_else(
        || workspace.join("build/linux-deps/ghostty/zig-out/lib"),
        PathBuf::from,
    );
    let library = library_directory.join("libghostty-gtk-embed.so");
    assert!(
        library.is_file(),
        "pinned Ghostty GTK embedding library is missing: {}",
        library.display()
    );

    println!("cargo:rerun-if-env-changed=GHOSTTY_LIB_DIR");
    println!("cargo:rerun-if-changed={}", library.display());
    println!(
        "cargo:rustc-link-search=native={}",
        library_directory.display()
    );
    println!("cargo:rustc-link-lib=dylib=ghostty-gtk-embed");
}
