fn main() {
    const CREATED_QUARKS: usize = 2_500;

    for index in 0..CREATED_QUARKS {
        let name = format!("zentty-valgrind-quark-growth-{index:04}");
        let _ = gtk::glib::Quark::from_str(&name);
    }

    println!("glib-quark-growth-reproducer: PASS created={CREATED_QUARKS}");
}
