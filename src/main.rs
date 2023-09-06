use std::fs::remove_dir_all;

use gpg::{
    check_card, export_public, export_ssh, factory_reset_key, gen_key, get_fingerprint, init_gpg,
    key_to_card,
};

mod config;
mod gpg;
mod server;

fn main() {
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
    println!("{:?}", pgp);
    println!("{:?}", ssh);
    println!("{:?}", fingerprint);
}
