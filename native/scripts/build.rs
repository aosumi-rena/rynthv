use std::{env,path::PathBuf,process::Command};
pub fn build() {
    println!("cargo::rerun-if-changed=native/scripts");
    let root=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("native/scripts");
    let output=PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target=env::var("TARGET").unwrap();let windows=target.contains("msvc");
    let mut objects=Vec::new();
    let mut sources=vec!["host.c".to_owned(),"lua_bridge.c".into(),"quickjs/dtoa.c".into(),"quickjs/libregexp.c".into(),"quickjs/libunicode.c".into(),"quickjs/quickjs.c".into()];
    let mut lua=std::fs::read_dir(root.join("lua")).expect("Lua source directory").map(|entry|entry.unwrap().path())
        .filter(|path|path.extension().is_some_and(|v|v=="c")).collect::<Vec<_>>();lua.sort();
    sources.extend(lua.iter().map(|p|format!("lua/{}",p.file_name().unwrap().to_string_lossy())));
    for name in sources {
        let object=output.join(format!("script_{}.{}",PathBuf::from(&name).file_stem().unwrap().to_string_lossy(),if windows{"obj"}else{"o"}));
        let mut compiler=if windows {
            let mut command=find_msvc_tools::find(&target,"cl.exe").expect("Script engine requires MSVC");
            command.args(["/nologo","/c","/O2","/MD","/std:c11","/experimental:c11atomics","/D_CRT_SECURE_NO_WARNINGS","/DWIN32_LEAN_AND_MEAN","/D_WIN32_WINNT=0x0601","/wd4244","/wd4267"])
                .arg(format!("/Fo{}",object.display()));command
        }else{
            let mut command=Command::new(env::var_os("CC").unwrap_or_else(||"cc".into()));
            command.args(["-c","-O2","-std=c11","-fPIC","-D_GNU_SOURCE"]).arg("-o").arg(&object);command
        };
        if name.starts_with("lua/"){
            if windows{compiler.arg("/DLUA_USE_WINDOWS");}
            else if target.contains("apple"){compiler.arg("-DLUA_USE_MACOSX");}
            else if target.contains("linux"){compiler.arg("-DLUA_USE_LINUX");}
        }
        assert!(compiler.arg(root.join(&name)).status().expect("compile script engine").success(),"Script source compilation failed: {name}");objects.push(object);
    }
    if windows {
        let mut archive=find_msvc_tools::find(&target,"lib.exe").expect("Script engine requires MSVC librarian");
        assert!(archive.arg("/nologo").arg(format!("/OUT:{}",output.join("rynthv_scripts.lib").display())).args(&objects).status().unwrap().success());
    }else{
        assert!(Command::new(env::var_os("AR").unwrap_or_else(||"ar".into())).arg("crs").arg(output.join("librynthv_scripts.a")).args(&objects).status().unwrap().success());
        println!("cargo::rustc-link-lib=m");
        if target.contains("linux"){println!("cargo::rustc-link-lib=dl");}
    }
    println!("cargo::rustc-link-search=native={}",output.display());println!("cargo::rustc-link-lib=static=rynthv_scripts");
}
