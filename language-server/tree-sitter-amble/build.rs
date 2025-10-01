use std::path::PathBuf;

fn main() {
    let src_dir = PathBuf::from("../../grammars/amble/src");
    
    let mut c_config = cc::Build::new();
    c_config.include(&src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    
    c_config.file(src_dir.join("parser.c"));
    
    // Check if scanner exists
    let scanner_path = src_dir.join("scanner.c");
    if scanner_path.exists() {
        c_config.file(scanner_path);
    }
    
    c_config.compile("tree-sitter-amble");
    
    println!("cargo:rerun-if-changed=../../grammars/amble/src");
}
