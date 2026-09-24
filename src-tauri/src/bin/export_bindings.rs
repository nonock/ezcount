fn main() {
    let builder = ezcount_lib::create_specta_builder();
    let path =
        ezcount_lib::export_bindings(&builder).expect("Failed to export typescript bindings");
    println!(
        "Successfully exported TypeScript bindings to {}",
        path.display()
    );
}
