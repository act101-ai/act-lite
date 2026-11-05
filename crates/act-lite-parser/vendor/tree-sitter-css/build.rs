fn main() {
    let src_dir = std::path::Path::new("src");
    let parser_path = src_dir.join("parser.c");

    let mut c_config = cc::Build::new();
    c_config
        .std("c11")
        .include(src_dir)
        .file(&parser_path)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable");

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    let scanner_path = src_dir.join("scanner.c");
    if scanner_path.exists() {
        c_config.file(&scanner_path);
    }

    c_config.compile("tree-sitter-css");

    println!("cargo:rerun-if-changed=src");
}
