fn main() {
    let mut c_config = cc::Build::new();
    c_config.std("c11").include("src");

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    c_config.file("src/parser.c");
    println!("cargo:rerun-if-changed=src/parser.c");

    if std::path::Path::new("src/scanner.c").exists() {
        c_config.file("src/scanner.c");
        println!("cargo:rerun-if-changed=src/scanner.c");
    }

    c_config.compile("tree-sitter-dockerfile");
}
