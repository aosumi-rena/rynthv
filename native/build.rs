use std::{env, fs, path::PathBuf, process::Command};
pub fn build() {
    let root=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source=root.join("native/r1");
    let output=PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target=env::var("TARGET").unwrap();let windows=target.contains("msvc");
    let compiler=||find_msvc_tools::find(&target,"cl.exe").expect("R1 native renderer requires MSVC");
    let cc=||Command::new(env::var_os("CC").unwrap_or_else(||"cc".into()));
    let mut names=vec!["r1_llsm2.c".to_owned()];
    if windows {names.push("r1_math.c".to_owned());}
    for name in ["container","frame","dsputils","llsmutils","layer0","layer1","coder","llsmrt"] {
        names.push(format!("vendor/libllsm2/{name}.c"));
    }
    names.extend(["vendor/external/ciglet/ciglet.c","vendor/external/ciglet/external/fftsg_h.c",
        "vendor/external/ciglet/external/fast_median.c"].map(String::from));
    println!("cargo::rerun-if-changed=native/r1");
    println!("cargo::rerun-if-changed=native/build.rs");
    let mut objects=Vec::new();
    for name in names {
        let file=source.join(&name);
        let object=output.join(format!("r1_{}.{}",file.file_stem().unwrap().to_string_lossy(),if windows{"obj"}else{"o"}));
        let mut command=if windows {
            let mut command=compiler();
            command.args(["/nologo","/c","/O2","/MT","/std:c11","/fp:strict",
                "/DFP_TYPE=float","/D_USE_MATH_DEFINES","/DNOMINMAX","/D_CRT_SECURE_NO_WARNINGS","/U__SSE2__"])
                .arg(format!("/I{}",source.join("vendor/external").display()))
                .arg(format!("/Fo{}",object.display()));command
        }else{
            let mut command=cc();
            command.args(["-c","-O2","-std=c11","-fPIC","-ffp-contract=off","-DFP_TYPE=float","-D_GNU_SOURCE","-U__SSE2__"])
                .arg(format!("-I{}",source.join("vendor/external").display())).arg("-o").arg(&object);command
        };
        assert!(command.arg(file).status().expect("compile R1 renderer").success(),"R1 native source compilation failed: {name}");
        objects.push(object);
    }
    let name=if windows{"r1_llsm2.dll"}else if target.contains("apple"){"libr1_llsm2.dylib"}else{"libr1_llsm2.so"};
    let library=output.join(name);
    let status=if windows {
        compiler().args(["/nologo","/LD","/MT"]).args(objects)
            .arg(format!("/Fe:{}",library.display())).args(["/link","/INCREMENTAL:NO","/OPT:REF","/OPT:ICF"]).status()
    }else{
        cc().arg("-shared").arg("-o").arg(&library).args(objects).arg("-lm").status()
    }.expect("link R1 renderer");
    assert!(status.success(),"R1 native link failed");
    let profile=output.ancestors().nth(3).expect("Cargo profile output directory");
    let destination=profile.join("std-resources/r1").join(name);
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::copy(library,destination).expect("install R1 renderer beside fixed-target application");
}
