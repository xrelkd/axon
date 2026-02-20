//! This module provides utilities for handling SSH keys and sessions.
//!
//! It includes functionality to load private keys from files, optionally
//! deciphering them with a password, and to derive public keys. It also
//! re-exports error types and session management.

mod error;
mod session;

use std::path::Path;

use russh::keys::PrivateKey;
use snafu::{OptionExt, ResultExt};

pub use self::{error::Error, session::Session};

/// Loads a secret key from a file, optionally deciphering it with a password.
///
/// This asynchronous function reads the content of the specified file, trims
/// whitespace, and then attempts to decode it as an SSH private key. If a
/// password is provided, it will be used to decipher the key. The comment of
/// the loaded private key is set to an empty string.
///
/// # Arguments
///
/// * `secret_key_file_path` - The path to the file containing the secret key.
/// * `password` - An optional password to use for deciphering the private key.
///
/// # Errors
///
/// This function returns an `Err` if:
///
/// * The `secret_key_file_path` cannot be read (e.g., file not found,
///   permission denied). The error will be of type
///   `error::ReadSshPrivateKeySnafu`.
/// * The content of the file cannot be decoded as a valid SSH private key, or
///   the provided password is incorrect for an encrypted key. The error will be
///   of type `error::ParseSshPrivateKeySnafu`.
///
/// # Examples
///
/// ```rust
/// use std::path::PathBuf;
/// use axon::ssh; // Assuming axon is the crate name
/// use tokio::fs;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create a dummy key file for the example
///     let key_content = "-----BEGIN RSA PRIVATE KEY-----
/// MIIBOQIBAAJAbjOQ...
/// -----END RSA PRIVATE KEY-----"; // Replace with actual key content or generate one
///     let temp_dir = tempfile::tempdir()?;
///     let key_path = temp_dir.path().join("id_rsa");
///     fs::write(&key_path, key_content).await?;
///
///     // Load the key without a password
///     let private_key = ssh::load_secret_key(&key_path, None).await?;
///     println!("Successfully loaded private key!");
///
///     // Example with a password (if the key was encrypted)
///     // let encrypted_key_path = temp_dir.path().join("id_rsa_encrypted");
///     // fs::write(&encrypted_key_path, "encrypted_key_content").await?;
///     // let password = Some("mysecretpassword");
///     // let encrypted_private_key = ssh::load_secret_key(&encrypted_key_path, password).await?;
///     // println!("Successfully loaded encrypted private key!");
///
///     Ok(())
/// }
/// ```
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
    russh::keys::decode_secret_key(&secret, password)
        .map(|mut secret_key| {
            // Remove the comment
            secret_key.set_comment(String::new());
            secret_key
        })
        .map_err(|_| error::ParseSshPrivateKeySnafu.build())
}

pub async fn resolve_ssh_key_pair<I, P>(paths: I) -> Result<(PrivateKey, String), Error>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut last_error = None;
    let mut attempted_paths = Vec::new();

    for path in paths {
        attempted_paths.push(path.as_ref().to_path_buf());

        match load_secret_key(path, None).await {
            Ok(private_key) => {
                return private_key
                    .public_key()
                    .to_openssh()
                    .map(|public_key| (private_key, public_key))
                    .ok()
                    .context(error::SerializeSshPublicKeySnafu);
            }
            Err(e) => last_error = Some(e),
        }
    }

    Err(Error::ResolveIdentities {
        paths: attempted_paths,
        source: last_error.map(Box::new).expect("`last_error` must be Some"),
    })
}
