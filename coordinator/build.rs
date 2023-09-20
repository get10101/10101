use std::process::Command;
fn main() {
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
