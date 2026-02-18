mod error;
mod session;

use std::path::Path;

use snafu::ResultExt;

pub use self::{error::Error, session::Session};

/// Load a secret key, deciphering it with the supplied password if necessary.
pub async fn load_secret_key<P: AsRef<Path>>(
    secret_key_file_path: P,
    password: Option<&str>,
) -> Result<russh::keys::PrivateKey, Error> {
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

pub async fn load_public_key_from_secret_key_file<P>(
    secret_key_file_path: P,
    password: Option<&str>,
) -> Result<String, Error>
where
    P: AsRef<Path>,
{
    load_secret_key(secret_key_file_path, password)
        .await?
        .public_key()
        .to_openssh()
        .map_err(|_| error::SerializeSshPublicKeySnafu.build())
}
