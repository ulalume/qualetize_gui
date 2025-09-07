use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let host = env::var("HOST").unwrap();
    let target = env::var("TARGET").unwrap();

    let is_msvc = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "msvc";
    let is_windows = target.contains("windows");

    if is_windows && is_msvc {
        panic!("MSVC compilation is not supported. Use MinGW target: x86_64-pc-windows-gnu");
    }

    let mut build = cc::Build::new();
    build
        .files([
            "external/qualetize/source/Qualetize.c",
            "external/qualetize/source/qualetize-cli.c",
            "external/qualetize/source/Bitmap.c",
            "external/qualetize/source/Cluster.c",
            "external/qualetize/source/Cluster_Vec4f.c",
            "external/qualetize/source/DitherImage.c",
        ])
        .include("external/qualetize/include")
        .include("external/qualetize/source")
        .warnings(false);

    // GCC/Clang/MinGW flags
    build.flag("-std=c99");
    build.flag("-O3");
    build.flag("-ffast-math");
    build.flag("-funroll-loops");

    if host != target {
        if target.contains("x86_64") {
            build.flag("-march=x86-64");
        } else if target.contains("aarch64") {
            build.flag("-march=armv8-a");
        }
    } else {
        build.flag("-march=native");
    }
    if target.contains("linux") {
        build.define("M_PI", "3.14159265358979323846");
    }

    if is_windows {
        build.define("_USE_MATH_DEFINES", None);

        let version = env::var("CARGO_PKG_VERSION").unwrap();
        let parts: Vec<&str> = version.split('.').collect();
        let major = parts.first().unwrap_or(&"0");
        let minor = parts.get(1).unwrap_or(&"0");
        let patch = parts.get(2).unwrap_or(&"0");
        let build_num = parts.get(3).unwrap_or(&"0");
        let file_version = format!("{major},{minor},{patch},{build_num}");

        let rc = format!(
            r#"IDI_ICON1 ICON "assets/icon.ico"

1 VERSIONINFO
FILEVERSION {file_version}
PRODUCTVERSION {file_version}
{{
  BLOCK "StringFileInfo"
  {{
    BLOCK "040904b0"
    {{
      VALUE "CompanyName", "ulalume"
      VALUE "FileDescription", "Qualetize GUI Application"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "Qualetize GUI"
      VALUE "LegalCopyright", "ulalume"
      VALUE "OriginalFilename", "QualetizeGUI.exe"
      VALUE "ProductName", "Qualetize GUI"
      VALUE "ProductVersion", "{version}"
    }}
  }}

  BLOCK "VarFileInfo"
  {{
    VALUE "Translation", 0x409, 1200
  }}
}}
"#
        );

        let out_dir = env::var("OUT_DIR").unwrap();
        let rc_path = Path::new(&out_dir).join("version_info.rc");
        fs::write(&rc_path, rc).unwrap();

        // Use windres for MinGW/GNU toolchain
        use std::process::Command;

        let obj_path = Path::new(&out_dir).join("version_info.o");
        let windres_result = Command::new("windres")
            .arg(&rc_path)
            .arg("-o")
            .arg(&obj_path)
            .output();

        match windres_result {
            Ok(output) => {
                if output.status.success() {
                    println!("cargo:rustc-link-arg={}", obj_path.display());
                } else {
                    eprintln!(
                        "windres failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    eprintln!("Continuing without Windows resources...");
                }
            }
            Err(_) => {
                // windres not available, continue without resources
                eprintln!("windres not found, continuing without Windows resources...");
            }
        }
    }

    build.compile("qualetize_c");
}
