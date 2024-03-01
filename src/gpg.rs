use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
#[cfg(target_family = "unix")]
use std::{os::unix::fs::PermissionsExt, path::PathBuf};

#[cfg(target_family = "unix")]
use log::error;
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
    Name-Real: {name}
    Name-Email: {email}
    Expire-Date: 0
    Subkey-Type: RSA
    Subkey-Length: 4096
    Subkey-Usage: sign, encrypt, auth
    %commit
    "
    )
}

pub fn key_to_card_args() -> String {
    format!(
        r#"{ADMIN_PIN}
key 1
keytocard
1
keytocard
2
keytocard
3
save"#
    )
}

#[cfg(target_family = "unix")]
pub fn set_permissions(dir_path: &PathBuf) {
    debug!("Setting permissions 700 for gpg temp folder.");
    let dir_string = dir_path.to_string_lossy();
    debug!("GPG temp folder set to {dir_string}");
    let permissions = fs::Permissions::from_mode(0o700);
    match fs::set_permissions(dir_path, permissions) {
        Ok(()) => {
            debug!("Permissions set");
        }
        Err(e) => {
            error!(
                "Failed to set permissions for GPG TEMP Home! \
            Location: {dir_string} \n \
            Error: {0}\n Program will proceed with default permissions.",
                e.to_string()
            );
        }
    }
}

#[allow(unused_variables)]
pub fn init_gpg(config: &Config) -> Result<(String, Child), WorkerError> {
    debug!("Initiating new gpg session.");
    let mut temp_path = env::temp_dir();
    temp_path.push("yubikey-provision");

    #[cfg(target_family = "unix")]
    if !config.skip_gpg_permissions {
        // ignore permissions error, just warn the user and proceed. Default permissions still allow for provisioning to work.
        set_permissions(&temp_path);
    }

    let temp_path_str = temp_path.to_str().ok_or(WorkerError::Gpg)?;

    {
        let res = Command::new("gpgconf")
            .args(["--kill", "gpg-agent"])
            .status()?;

        if !res.success() {
            debug!("Failed to Kill current gpg agent via gpgconf --kill gpg-agent");
            return Err(WorkerError::Gpg);
        }
        debug!("User gpg agent session killed");
    }

    debug!("gpg temporary home: {}", &temp_path_str);

    // init temp
    if Path::new(&temp_path).is_dir() {
        fs::remove_dir_all(&temp_path)?;
    }
    fs::create_dir_all(&temp_path)?;
    debug!("gpg home created");

    // init local temp gpg home

    let gpg_agent = Command::new("gpg-agent")
        .args(["--homedir", temp_path_str, "--daemon"])
        .spawn()?;

    debug!("gpg agent alive");

    Ok((temp_path_str.to_string(), gpg_agent))
}

pub fn gen_key(
    gpg_command: &str,
    gpg_debug_level: &str,
    gpg_home: &str,
    full_name: &str,
    email: &str,
) -> Result<(), WorkerError> {
    let command_args = [
        "--debug-level",
        gpg_debug_level,
        "--homedir",
        gpg_home,
        "--batch",
        "--command-fd",
        "0",
        "--full-gen-key",
    ];
    debug!(
        "Generating key via {} with args: {}",
        gpg_command,
        command_args.join(" ")
    );
    let mut child = Command::new(gpg_command)
        .args(command_args)
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

pub fn key_to_card(
    gpg_command: &str,
    gpg_debug_level: &str,
    gpg_home: &str,
    email: &str,
) -> Result<(), WorkerError> {
    let command_args = [
        "--debug-level",
        gpg_debug_level,
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
    ];
    debug!(
        "Transferring keys via {} with args: {}",
        gpg_command,
        &command_args.join(" ")
    );
    let mut child = Command::new(gpg_command)
        .args(command_args)
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

// returns serial number of yubikey if detected
pub fn check_card() -> Result<String, WorkerError> {
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
    let out_split: Vec<&str> = out_str.split_whitespace().collect();
    for (i, &item) in out_split.iter().enumerate() {
        if item == "Serial:" {
            if let Some(serial) = out_split.get(i + 1) {
                return Ok((*serial).to_string());
            }

            return Err(WorkerError::SerialNotFound);
        }
    }
    Err(WorkerError::SerialNotFound)
}

#[allow(dead_code)]
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
    }
    Err(WorkerError::Gpg)
}

#[derive(Serialize, Debug)]
pub struct ProvisioningInfo {
    pub pgp: String,
    pub ssh: String,
    pub serial: String,
}

pub async fn provision_key(
    config: &Config,
    job: &proto::GetJobResponse,
    gpg_command: &str,
) -> Result<ProvisioningInfo, WorkerError> {
    let full_name = format!("{} {}", job.first_name, job.last_name);
    debug!("Provisioning start for: {}", &job.email);
    let check_duration = Duration::from_secs(config.smartcard_retry_interval);
    let mut check_interval = interval(check_duration);
    let mut fail_counter = 0;
    let serial: String;
    loop {
        check_interval.tick().await;
        match check_card() {
            Ok(res) => {
                serial = res;
                break;
            }
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
    debug!("Key with serial ({serial}) found");
    let (gpg_home, mut gpg_process) = init_gpg(config)?;
    debug!("Temporary GPG session crated");
    debug!("Resetting card to factory");
    factory_reset_key()?;
    debug!("OpenPGP Key app restored to factory.");
    gen_key(
        gpg_command,
        &config.gpg_debug_level,
        &gpg_home,
        &full_name,
        &job.email,
    )?;
    debug!("OpenPGP key for {} created", &job.email);
    let pgp = export_public(gpg_command, &gpg_home, &job.email)?;
    let ssh = export_ssh(gpg_command, &gpg_home, &job.email)?;
    key_to_card(gpg_command, &config.gpg_debug_level, &gpg_home, &job.email)?;
    debug!("Subkeys saved in yubikey");
    // cleanup after provisioning
    debug!("Clearing gpg process and home");
    if gpg_process.kill().is_err() {
        return Err(WorkerError::GPGSessionEnd);
    }
    debug!("gpg session killed");
    if fs::remove_dir_all(&gpg_home).is_err() {
        return Err(WorkerError::GPGSessionEnd);
    }
    debug!("Temp home cleared");
    info!("Yubikey openpgp provisioning completed.");
    Ok(ProvisioningInfo { pgp, ssh, serial })
}
