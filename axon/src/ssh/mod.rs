mod error;
mod session;

use std::path::{Path, PathBuf};

use russh::keys::PrivateKey;
use snafu::{OptionExt, ResultExt};

pub use self::{error::Error, session::Session};

/// Load a secret key, deciphering it with the supplied password if necessary.
pub async fn load_secret_key<P: AsRef<Path>>(
    secret_key_file_path: P,
    password: Option<&str>,
) -> Result<PrivateKey, Error> {
    let secret = tokio::fs::read_to_string(secret_key_file_path.as_ref())
        .await
        .with_context(|_| error::ReadSshPrivateKeySnafu {
            file_path: secret_key_file_path.as_ref().to_path_buf(),
        })?
        .trim()
        .to_string();
    let mut secret_key = russh::keys::decode_secret_key(&secret, password)
        .map_err(|_| error::ParseSshPrivateKeySnafu.build())?;
    secret_key.set_comment(String::new());
    Ok(secret_key)
}

/// Helper function to load the SSH private key and generate the public key.
/// It takes `ssh_private_key_file` (from CLI arg) and
/// `config_ssh_private_key_file_path` (from config) as input.
/// Returns `Result<(PrivateKey, String), crate::cli::Error>`.
pub async fn load_ssh_key_pair(
    ssh_private_key_file: Option<PathBuf>,
    config_ssh_private_key_file_path: Option<PathBuf>,
) -> Result<(PrivateKey, String), Error> {
    let private_key = {
        let ((Some(private_key_file), _) | (None, Some(private_key_file))) =
            (ssh_private_key_file, config_ssh_private_key_file_path)
        else {
            return error::NoSshPrivateKeyProvidedSnafu.fail();
        };
        load_secret_key(private_key_file, None).await?
    };

    let public_key =
        private_key.public_key().to_openssh().ok().context(error::SerializeSshPublicKeySnafu)?;
    Ok((private_key, public_key))
}
