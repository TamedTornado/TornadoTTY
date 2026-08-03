fn main() {
    println!("cargo:rustc-link-arg=-Wl,--disable-new-dtags,-rpath,$ORIGIN/../lib");
}
