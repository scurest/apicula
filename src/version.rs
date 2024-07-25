pub fn print_version_info() {
    println!("apicula {}", env!("CARGO_PKG_VERSION"));

    // These variables can optionally be set during the build process
    if let Some(info) = option_env!("APICULA_BUILD_COMMIT_HASH") {
        println!("build commit: {}", info);
    }
    if let Some(info) = option_env!("APICULA_BUILD_COMMIT_DATE") {
        println!("build commit date: {}", info);
    }
}
