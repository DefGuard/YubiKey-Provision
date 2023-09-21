use std::path::PathBuf;
use std::time::Duration;
use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
};

use log::{debug, info};
use serde::Serialize;
use tokio::time::interval;
use which::which;

use crate::config::Config;
use crate::error::WorkerError;
use crate::proto;

pub const ADMIN_PIN: &str = "12345678";

pub fn get_gpg_command() -> &'static str {
    if which("gpg").is_err() {
        if which("gpg2").is_err() {
            panic!("gpg not found");
        } else {
            return "gpg2";
        }
    }
    "gpg"
}

pub fn card_info_args(name: &str, email: &str) -> String {
    format!(
        r"
    %no-protection
    Key-Type: RSA
    Key-Length: 4096
    Name-Real: {0}
    Name-Email: {1}
    Expire-Date: 0
    Subkey-Type: RSA
    Subkey-Length: 4096
    Subkey-Usage: sign, encrypt, auth
    %commit
    ",
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

#[cfg(unix)]
pub fn set_permissions(dir_path: &PathBuf) -> Result<(), WorkerError> {
    use std::os::unix::prelude::PermissionsExt;

    let permissions = fs::Permissions::from_mode(0o700);
    fs::set_permissions(dir_path, permissions)?;
    Ok(())
}

pub fn init_gpg() -> Result<(String, Child), WorkerError> {
    let mut temp_path = env::temp_dir();
    temp_path.push("yubikey-provision");
    if cfg!(unix) {
        set_permissions(&temp_path)?;
    }
    let temp_path_str = temp_path.to_str().ok_or(WorkerError::Gpg)?;

    {
        let res = Command::new("gpgconf")
            .args(["--kill", "gpg-agent"])
            .status()?;

        if !res.success() {
            return Err(WorkerError::Gpg);
        }
    }

    // init temp
    if Path::new(&temp_path).is_dir() {
        fs::remove_dir_all(&temp_path)?;
    }
    fs::create_dir_all(&temp_path)?;

    // init local temp gpg home

    let gpg_agent = Command::new("gpg-agent")
        .args(["--homedir", temp_path_str, "--daemon"])
        .spawn()?;

    Ok((temp_path_str.to_string(), gpg_agent))
}

pub fn gen_key(
    gpg_command: &str,
    gpg_home: &str,
    full_name: &str,
    email: &str,
) -> Result<(), WorkerError> {
    let mut child = Command::new(gpg_command)
        .args([
            "--homedir",
            gpg_home,
            "--batch",
            "--command-fd",
            "0",
            "--full-gen-key",
        ])
        .stdin(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or(WorkerError::Gpg)?;
    let info_args = card_info_args(full_name, email);
    std::thread::spawn(move || {
        let _ = stdin.write_all(info_args.as_bytes());
    });
    child.wait()?;
    Ok(())
}

pub fn key_to_card(gpg_command: &str, gpg_home: &str, email: &str) -> Result<(), WorkerError> {
    let mut child = Command::new(gpg_command)
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
            email,
        ])
        .env("LANG", "en")
        .stdin(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    std::thread::spawn(move || {
        let input = key_to_card_args();
        let _ = stdin.write_all(input.as_bytes());
    });
    child.wait()?;
    Ok(())
}

pub fn export_public(
    gpg_command: &str,
    gpg_home: &str,
    email: &str,
) -> Result<String, WorkerError> {
    let out = Command::new(gpg_command)
        .args(["--homedir", gpg_home, "--armor", "--export", email])
        .stdout(Stdio::piped())
        .output()?;
    let out_str = String::from_utf8(out.stdout)?;
    Ok(out_str)
}

pub fn export_ssh(gpg_command: &str, gpg_home: &str, email: &str) -> Result<String, WorkerError> {
    let out = Command::new(gpg_command)
        .args(["--homedir", gpg_home, "--export-ssh-key", email])
        .output()?;
    let out_str = String::from_utf8(out.stdout)?;
    Ok(out_str)
}

pub fn factory_reset_key() -> Result<(), WorkerError> {
    let status = Command::new("ykman")
        .args(["openpgp", "reset", "-f"])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(WorkerError::YubikeyManager)
    }
}

pub fn check_card() -> Result<(), WorkerError> {
    let out = Command::new("ykman")
        .args(["list"])
        .output()
        .expect("Failed to call ykman");
    if !out.status.success() {
        return Err(WorkerError::YubikeyManager);
    }
    let out_str =
        String::from_utf8(out.stdout).expect("Failed to read output from ykman openpgp list");
    let lines: Vec<String> = out_str
        .split("\r\n")
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();
    let keys_found = lines.len();
    if keys_found == 0 {
        return Err(WorkerError::NoKeysFound);
    }
    if keys_found != 1 {
        return Err(WorkerError::MultipleKeysPresent);
    }
    Ok(())
}

pub fn get_fingerprint() -> Result<String, WorkerError> {
    let out = Command::new("gpg")
        .args(["--list-keys"])
        .env("LANG", "en")
        .output()
        .expect("Failed to call gpg");
    let out_str = String::from_utf8(out.stdout)?;
    let lines: Vec<String> = out_str
        .split("\r\n")
        .filter(|line| !line.is_empty())
        .map(|line| line.trim().replace(' ', "").to_string())
        .collect();
    if let Some(index) = lines.iter().position(|l| l.starts_with("pub")) {
        return match lines.get(index + 1) {
            Some(fingerprint) => Ok(fingerprint.to_string()),
            None => Err(WorkerError::Gpg),
        };
    } else {
        Err(WorkerError::Gpg)
    }
}

#[derive(Serialize, Debug)]
pub struct ProvisioningInfo {
    pub pgp: String,
    pub ssh: String,
    pub fingerprint: String,
}

pub async fn provision_key(
    config: &Config,
    job: &proto::GetJobResponse,
    gpg_command: &str,
) -> Result<ProvisioningInfo, WorkerError> {
    let full_name = format!("{} {}", job.first_name, job.last_name);
    info!("Provisioning start for: {}", &job.email);
    let check_duration = Duration::from_secs(config.smartcard_retry_interval);
    let mut check_interval = interval(check_duration);
    let mut fail_counter = 0;
    loop {
        check_interval.tick().await;
        match check_card() {
            Ok(_) => break,
            Err(e) => match e {
                WorkerError::NoKeysFound => {
                    info!(
                        "No keys found, retry in {} seconds",
                        check_duration.as_secs()
                    );
                    if fail_counter >= config.smartcard_retries {
                        return Err(WorkerError::NoKeysFound);
                    }
                    fail_counter += 1;
                }
                _ => return Err(e),
            },
        }
    }
    debug!("Key found");
    let (gpg_home, mut gpg_process) = init_gpg()?;
    debug!("Temporary GPG session crated");
    factory_reset_key()?;
    debug!("OpenPGP Key app restored to factory.");
    gen_key(gpg_command, &gpg_home, &full_name, &job.email)?;
    debug!("OpenPGP key for {} created", &job.email);
    let fingerprint = get_fingerprint()?;
    let pgp = export_public(gpg_command, &gpg_home, &job.email)?;
    let ssh = export_ssh(gpg_command, &gpg_home, &job.email)?;
    key_to_card(gpg_command, &gpg_home, &job.email)?;
    debug!("Subkeys saved in yubikey");
    // cleanup after provisioning
    if gpg_process.kill().is_err() {
        return Err(WorkerError::GPGSessionEnd);
    }
    if fs::remove_dir_all(&gpg_home).is_err() {
        return Err(WorkerError::GPGSessionEnd);
    }
    debug!("Temporary GPG session cleared and closed");
    info!("Yubikey openpgp provisioning completed.");
    Ok(ProvisioningInfo {
        pgp,
        ssh,
        fingerprint,
    })
}
