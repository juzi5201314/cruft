use std::path::PathBuf;

fn main() {
    println!("Exporting procedural textures...");
    let source_path = PathBuf::from("assets/texture_data/blocks.texture.json");
    let compiled = cruft_proc_textures::load_and_compile_texture_set(&source_path).unwrap();
    let output_dir = PathBuf::from("exported_textures");
    cruft_proc_textures::export_compiled_texture_set_to_dir(&compiled, &output_dir).unwrap();
    println!("Done!");
}
