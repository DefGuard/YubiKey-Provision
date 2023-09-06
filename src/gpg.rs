use std::{
    env, fs,
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
};

pub const ADMIN_PIN: &str = "12345678";
pub const USER_PIN: &str = "123456";
pub const EMAIL: &str = "test@teonite.com";
pub const USERNAME: &str = "test";
pub const FULL_NAME: &str = "Test Test";

pub fn card_info_args(name: &str, email: &str) -> String {
    format!(
        r#"
    %no-protection
    Key-Type: RSA
    Key-Length: 2048
    Name-Real: {}
    Name-Email: {}
    Expire-Date: 0
    Subkey-Type: RSA
    Subkey-Length: 2048
    Subkey-Usage: sign, encrypt, auth
    %commit
    "#,
        name, email
    )
}

pub fn key_to_card_args() -> String {
    format!(
        r#"{0}
key 1
keytocard
1
keytocard
2
keytocard
3
save"#,
        ADMIN_PIN
    )
}

pub fn init_gpg() -> (String, Child) {
    let working_dir = env::current_dir().expect("Failed to get current directory");

    let temp_path = working_dir
        .to_str()
        .expect("Failed to transform working dir path into string")
        .to_owned()
        + "/temp";

    {
        let res = Command::new("gpgconf")
            .args(["--kill", "gpg-agent"])
            .status()
            .expect("Failed to execute gpgconf");

        if !res.success() {
            panic!("Failed to kill currently running gpg-agent.");
        }
    }

    // init temp
    if Path::new(&temp_path).is_dir() {
        fs::remove_dir_all(&temp_path).expect("Failed to clean temp");
    }
    fs::create_dir_all(&temp_path).expect("Failed to create temp dir");

    // init local temp gpg home

    let gpg_agent = Command::new("gpg-agent")
        .args(["--homedir", &temp_path.clone(), "--daemon"])
        .spawn()
        .expect("Failed to spawn new gpg-agent");

    return (temp_path, gpg_agent);
}

pub fn gen_key(gpg_home: &str) {
    let mut child = Command::new("gpg")
        .args([
            "--homedir",
            gpg_home,
            "--batch",
            "--command-fd",
            "0",
            "--full-gen-key",
        ])
        .stdin(Stdio::piped())
        .spawn()
        .expect("Spawning GPG command failed");
    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    std::thread::spawn(move || {
        let info = card_info_args(&FULL_NAME, &EMAIL);
        stdin.write_all(info.as_bytes()).expect("Failed to write");
    });
    child.wait().expect("Failed to wait on child");
}

pub fn key_to_card(gpg_home: &str) {
    let mut child = Command::new("gpg")
        .args([
            "--homedir",
            gpg_home,
            "--command-fd=0",
            "--status-fd=1",
            "--passphrase-fd=0",
            "--batch",
            "--yes",
            "--pinentry-mode=loopback",
            "--edit-key",
            "--no-tty",
            EMAIL,
        ])
        .env("LANG", "en")
        .stdin(Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");
    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    std::thread::spawn(move || {
        let input = key_to_card_args();
        stdin
            .write_all(input.as_bytes())
            .expect("Failed to write into stdin");
    });
    child.wait().expect("Failed to wait on child");
}

pub fn export_public(gpg_home: &str) -> String {
    let out = Command::new("gpg")
        .args(["--homedir", gpg_home, "--armor", "--export", EMAIL])
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to spawn process");
    let out_str = String::from_utf8(out.stdout).expect("Failed to parse output");
    out_str
}

pub fn export_ssh(gpg_home: &str) -> String {
    let out = Command::new("gpg")
        .args(["--homedir", gpg_home, "--export-ssh-key", EMAIL])
        .output()
        .expect("Failed to execute gpg ssh export");
    let out_str = String::from_utf8(out.stdout).expect("Failed to parse output");
    out_str
}

pub fn factory_reset_key() {
    let status = Command::new("ykman")
        .args(["openpgp", "reset", "-f"])
        .status()
        .expect("Failed to spawn process");
    if !status.success() {
        panic!("Failed to restore card to factory");
    }
}

pub fn check_card() {
    let out = Command::new("ykman")
        .args(["list"])
        .output()
        .expect("Failed to call ykman");
    if !out.status.success() {
        panic!("Failed to list yubikeys");
    }
    let out_str =
        String::from_utf8(out.stdout).expect("Failed to read output from ykman openpgp list");
    let lines: Vec<String> = out_str
        .split("\r\n")
        .into_iter()
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();
    println!("{:?}", &lines);
    let keys_found = lines.len();
    if keys_found == 0 {
        panic!("No yubikeys detected");
    }
    if keys_found != 1 {
        panic!("Only one yubikey can be connected");
    }
}

pub fn get_fingerprint() -> String {
    let out = Command::new("gpg")
        .args(["--list-keys"])
        .env("LANG", "en")
        .output()
        .expect("Failed to call gpg");
    let out_str = String::from_utf8(out.stdout).expect("Failed to parse stdout");
    let lines: Vec<String> = out_str
        .split("\r\n")
        .filter(|line| !line.is_empty())
        .map(|line| line.trim().replace(" ", "").to_string())
        .collect();
    if let Some(index) = lines.iter().position(|l| l.starts_with("pub")) {
        match lines.get(index + 1) {
            Some(fingerprint) => return fingerprint.to_string(),
            None => panic!("Fingerpirnt not found"),
        }
    } else {
        panic!("Fingerprint not found")
    }
}
