use std::{env, path::PathBuf};
#[path="native/build.rs"]
mod standard_native;

fn main() {
    println!("cargo::rerun-if-changed=assets/branding/app-icon.ico");
    println!("cargo::rerun-if-changed=assets/branding/app-icon.rc");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    standard_native::build();

    let target = env::var("TARGET").expect("Cargo must provide TARGET");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"));
    let resource = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build output directory"))
        .join("app-icon.res");
    let mut compiler = find_msvc_tools::find(&target, "rc.exe")
        .expect("building the Windows app icon requires the Windows SDK resource compiler");
    let output = compiler
        .current_dir(root)
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg("assets/branding/app-icon.rc")
        .output()
        .expect("could not run the Windows SDK resource compiler");
    assert!(
        output.status.success(),
        "app icon resource compilation failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    println!("cargo::rustc-link-arg-bin=rynthv={}", resource.display());
}
