fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    c_config.file(src_dir.join("parser.c"));
    println!("cargo:rerun-if-changed=src/parser.c");
    c_config.compile("tree-sitter-markdown-parser");

    let mut cpp_config = cc::Build::new();
    cpp_config.cpp(true).include(src_dir);
    cpp_config
        .define("TREE_SITTER_MARKDOWN_AVOID_CRASH", None)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable");
    cpp_config.file(src_dir.join("scanner.cc"));
    println!("cargo:rerun-if-changed=src/scanner.cc");
    cpp_config.compile("tree-sitter-markdown-scanner");
}
