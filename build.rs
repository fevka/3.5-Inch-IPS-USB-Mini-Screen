fn main() {
    compile_lhm_reader();
    copy_assets();
}

fn copy_assets() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // Navigate from OUT_DIR to the profile dir (target/debug/ or target/release/)
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let profile_dir = std::path::Path::new(&out_dir)
        .parent()      // .../build/mini-system-monitor-<hash>
        .and_then(|p| p.parent())  // .../build
        .and_then(|p| p.parent())  // .../debug or .../release
        .expect("cannot determine profile dir from OUT_DIR");

    // Copy config.yaml
    let src_cfg = manifest_dir.join("config.yaml");
    let dst_cfg = profile_dir.join("config.yaml");
    if src_cfg.exists() {
        std::fs::copy(&src_cfg, &dst_cfg).ok();
        println!("cargo:warning=config.yaml copied to {}", dst_cfg.display());
    }

    // Copy res/ recursively
    let src_res = manifest_dir.join("res");
    let dst_res = profile_dir.join("res");
    if src_res.exists() {
        copy_dir_recursive(&src_res, &dst_res);
        println!("cargo:warning=res/ copied to {}", dst_res.display());
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
    if dst.exists() {
        std::fs::remove_dir_all(dst).ok();
    }
    std::fs::create_dir_all(dst).ok();
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir_recursive(&src_path, &dst_path);
            } else {
                std::fs::copy(&src_path, &dst_path).ok();
            }
        }
    }
}

fn compile_lhm_reader() {
    let lhm_dir = std::path::Path::new("LibreHardwareMonitor");
    if !lhm_dir.join("LibreHardwareMonitorLib.dll").exists() {
        println!("cargo:warning=LibreHardwareMonitorLib.dll not found, skipping LHM reader");
        return;
    }

    let out_exe = lhm_dir.join("lhm_reader.exe");
    let cs_file = std::path::Path::new("lhm_reader").join("Program.cs");
    if !cs_file.exists() {
        println!("cargo:warning=lhm_reader/Program.cs not found, skipping LHM reader");
        return;
    }

    // Find csc.exe via vswhere
    let vswhere = r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe";
    let csc_path = if std::path::Path::new(vswhere).exists() {
        let output = std::process::Command::new(vswhere)
            .args(["-latest", "-products", "*", "-requires", "Microsoft.Component.MSBuild", "-find", "MSBuild\\**\\Bin\\Roslyn\\csc.exe"])
            .output();
        if let Ok(out) = output {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && std::path::Path::new(&s).exists() {
                Some(s)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let csc = csc_path.unwrap_or_else(|| {
        "C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\MSBuild\\Current\\Bin\\Roslyn\\csc.exe".to_string()
    });

    let status = std::process::Command::new(&csc)
        .args([
            "-nologo",
            "-target:exe",
            "-reference:LibreHardwareMonitor\\LibreHardwareMonitorLib.dll",
            &format!("-out:{}", out_exe.display()),
            &cs_file.display().to_string(),
        ])
        .status()
        .expect("failed to compile lhm_reader");

    if !status.success() {
        println!("cargo:warning=lhm_reader compilation failed");
    } else {
        println!("cargo:warning=lhm_reader compiled successfully");
    }
}
