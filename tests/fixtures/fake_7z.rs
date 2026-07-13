use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};

fn main() {
    let executable = env::current_exe().unwrap();
    let name = executable.file_stem().unwrap().to_string_lossy();
    if env::var_os("ZT_FAKE_7Z_ALL_FAIL").is_some() || name == "7z" {
        process::exit(32);
    }
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(archive) = args
        .iter()
        .find(|arg| arg.ends_with(".7z"))
        .map(PathBuf::from)
    else {
        process::exit(2);
    };
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent).ok();
    }
    let archive_index = args.iter().position(|arg| PathBuf::from(arg) == archive).unwrap();
    let tar = PathBuf::from(env::var_os("SystemRoot").unwrap())
        .join("System32")
        .join("tar.exe");
    let status = Command::new(tar)
        .args(["-a", "-cf"])
        .arg(&archive)
        .arg("-C")
        .arg(env::current_dir().unwrap())
        .args(&args[archive_index + 1..])
        .status()
        .unwrap();
    if !status.success() {
        process::exit(33);
    }
}
