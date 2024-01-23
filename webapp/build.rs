use std::fs;
use std::process::Command;

fn main() {
    // ensure that the directory exists which needs to be embedded in our binary
    let directory_path = "./frontend/build/web";
    if fs::create_dir_all(directory_path).is_err() {
        std::process::exit(1);
    }

    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("To be able to get commit hash");
    let git_hash = String::from_utf8(output.stdout).expect("To be a valid string");
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .expect("To be able to get branch name");
    let branch_name = String::from_utf8(output.stdout).expect("To be a valid string");
    println!("cargo:rustc-env=COMMIT_HASH={}", git_hash);
    println!("cargo:rustc-env=BRANCH_NAME={}", branch_name);
}
