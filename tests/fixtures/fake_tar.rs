use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

fn main() {
    if env::var_os("ZT_FAKE_TAR_ALWAYS_FAIL").is_some() {
        process::exit(31);
    }
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(index) = args.iter().position(|arg| arg == "-cf") else {
        process::exit(2);
    };
    let Some(archive) = args.get(index + 1).map(PathBuf::from) else {
        process::exit(2);
    };
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent).ok();
    }
    if archive
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("probe"))
    {
        // A probe archive carries the signature of the format its name asks for.
        let name = archive.to_string_lossy().into_owned();
        let signature: &[u8] = if name.ends_with(".zip") {
            b"PK\x03\x04probe"
        } else if name.ends_with(".tar.gz") {
            &[0x1f, 0x8b, 0x08]
        } else if name.ends_with(".tar.zst") {
            &[0x28, 0xb5, 0x2f, 0xfd]
        } else {
            b"probe archive"
        };
        fs::write(archive, signature).unwrap();
        return;
    }
    fs::write(archive, b"partial archive").unwrap();
    eprintln!("intentional compression failure");
    process::exit(23);
}
