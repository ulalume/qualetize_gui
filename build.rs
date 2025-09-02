use std::env;
use std::fs;
use std::path::Path;

fn main() {
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
        .flag("-std=c99")
        .flag("-O3")
        .flag("-ffast-math")
        .flag("-funroll-loops")
        .flag("-w")
        .warnings(false);

    let host = std::env::var("HOST").unwrap();
    let target = std::env::var("TARGET").unwrap();

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
    if target.contains("windows") {
        build.flag("-D_USE_MATH_DEFINES");

        let version = env::var("CARGO_PKG_VERSION").unwrap();
        let parts: Vec<&str> = version.split('.').collect();
        let major = parts.get(0).unwrap_or(&"0");
        let minor = parts.get(1).unwrap_or(&"0");
        let patch = parts.get(2).unwrap_or(&"0");
        let build_num = parts.get(3).unwrap_or(&"0");
        let file_version = format!("{},{},{},{}", major, minor, patch, build_num);

        let rc = format!(
            r#"IDI_ICON1 ICON "assets/icon.ico"

1 VERSIONINFO
FILEVERSION {version_comma}
PRODUCTVERSION {version_comma}
{{
  BLOCK "StringFileInfo"
  {{
    BLOCK "040904b0"
    {{
      VALUE "CompanyName", "ulalume"
      VALUE "FileDescription", "Qualetize GUI Application"
      VALUE "FileVersion", "{version_dot}"
      VALUE "InternalName", "Qualetize GUI"
      VALUE "LegalCopyright", "ulalume"
      VALUE "OriginalFilename", "QualetizeGUI.exe"
      VALUE "ProductName", "Qualetize GUI"
      VALUE "ProductVersion", "{version_dot}"
    }}
  }}

  BLOCK "VarFileInfo"
  {{
    VALUE "Translation", 0x409, 1200
  }}
}}
"#,
            version_comma = file_version,
            version_dot = version
        );

        let out_dir = env::var("OUT_DIR").unwrap();
        let rc_path = Path::new(&out_dir).join("version_info.rc");
        fs::write(&rc_path, rc).unwrap();

        embed_resource::compile(rc_path, embed_resource::NONE);
    }

    build.compile("qualetize_c");
}
