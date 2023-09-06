use std::fs::remove_dir_all;

use error::WorkerError;
use gpg::{
    check_card, export_public, export_ssh, factory_reset_key, gen_key, get_fingerprint, init_gpg,
    key_to_card,
};

mod client;
mod config;
mod error;
mod gpg;

#[macro_use]
extern crate log;

#[allow(non_snake_case)]
mod proto {
    tonic::include_proto!("worker");
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<(), WorkerError> {
    check_card();
    let (gpg_home, mut gpg_process) = init_gpg();
    factory_reset_key();
    gen_key(&gpg_home);
    let fingerprint = get_fingerprint();
    let pgp = export_public(&gpg_home);
    let ssh = export_ssh(&gpg_home);
    key_to_card(&gpg_home);
    // cleanup on exit
    gpg_process.kill().expect("Failed to kill gpg agent");
    remove_dir_all(&gpg_home).expect("Failed to cleanup temp");
    info!("{:?}", pgp);
    println!("{:?}", ssh);
    println!("{:?}", fingerprint);
    Ok(())
}
