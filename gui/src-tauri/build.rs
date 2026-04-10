use std::env;
use std::fs;
use std::path::Path;

fn main() {
    tauri_build::build();

    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let dll_name = "WebView2Loader.dll";
        let src_dll = Path::new(&manifest_dir).join(dll_name);

        let out_dir = env::var("OUT_DIR").unwrap();
        let out_path = Path::new(&out_dir);
        let mut release_dir = out_path.to_path_buf();
        for _ in 0..5 {
            if release_dir.join("deps").exists() {
                break;
            }
            if !release_dir.pop() {
                break;
            }
        }

        let dest_dll = release_dir.join(dll_name);
        if src_dll.exists() {
            let _ = fs::copy(&src_dll, &dest_dll);
        }
    }
}
