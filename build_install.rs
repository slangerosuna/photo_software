use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=shaders");
    println!("cargo:rerun-if-changed=build.rs");

    let base_target_dir = std::env::var("CARGO_HOME").unwrap();
    let target_dir = format!("{}/bin/joyful_create_shaders", base_target_dir);
    let target_dir = Path::new(target_dir.as_str());

    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir).unwrap();
    }

    std::fs::create_dir(target_dir).unwrap();

    let shaders_dir = Path::new("shaders");
    compile_and_copy_files(shaders_dir, target_dir);
}

fn compile_and_copy_files(from: &Path, to: &Path) {
    let read_dir = std::fs::read_dir(from).unwrap();
    for entry in read_dir {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name().unwrap();
            let new_dir = to.join(dir_name);
            std::fs::create_dir(&new_dir).unwrap();
            compile_and_copy_files(&path, &new_dir);
        } else {
            let file_name = path.file_name().unwrap();

            let wgsl_code = std::fs::read_to_string(&path).unwrap();
            let module = wgsl::parse_str(&wgsl_code).unwrap();

            let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
            let module_info = validator.validate(&module).unwrap();

            let options = spv::Options::default();

            let mut spv_writer = spv::Writer::new(&options).unwrap();
            let mut spv_words = Vec::<u32>::new();
            spv_writer.write(&module, &module_info, None, &None, &mut spv_words).unwrap();

            let spv_bytes = spv_words
                .iter()
                .flat_map(|word| word.to_le_bytes().to_vec())
                .collect::<Vec<u8>>();

            let new_file = to.join(file_name);
            let new_file = new_file.with_extension("spv");

            std::fs::write(&new_file, &spv_bytes).unwrap();
        }
    }
}
