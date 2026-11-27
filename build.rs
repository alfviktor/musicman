use std::{env, path::PathBuf, process::Command};
fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let sources = [
        "native/AppleAudio.swift",
        "native/FileEqualizer.swift",
        "native/Library.swift",
        "native/Quality.swift",
        "native/SongResolver.swift",
    ];
    for source in sources {
        println!("cargo:rerun-if-changed={source}");
    }
    let arch = if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    let platform = format!("{arch}-apple-macos14.0");
    let status = Command::new("xcrun")
        .args([
            "swiftc",
            "-O",
            "-swift-version",
            "5",
            "-emit-library",
            "-parse-as-library",
            "-module-name",
            "PocketAudio",
            "-Xlinker",
            "-install_name",
            "-Xlinker",
            "@rpath/libPocketAudio.dylib",
        ])
        .args(["-target", &platform])
        .args(sources)
        .arg("-o")
        .arg(out.join("libPocketAudio.dylib"))
        .status()
        .expect("Xcode with Swift is required");
    assert!(status.success(), "Apple audio adapter compilation failed");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=dylib=PocketAudio");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
}
