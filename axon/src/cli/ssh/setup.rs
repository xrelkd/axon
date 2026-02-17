use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, api::AttachParams};
use snafu::ResultExt;

use crate::{config::Config, error, error::Error, ext::ApiPodExt};

#[derive(Args, Clone)]
pub struct SetupCommand {
    #[arg(short, long, help = "Namespace of the pod")]
    pub namespace: Option<String>,

    #[arg(short = 'p', long = "pod-name", help = "Name of the pod to attach to")]
    pub pod_name: Option<String>,

    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait before timing out"
    )]
    pub timeout_secs: u64,

    #[arg(short = 'i', long = "ssh-private-key-file", help = "File path of a SSH private key")]
    pub ssh_private_key_file: Option<PathBuf>,
}

impl SetupCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, timeout_secs, ssh_private_key_file } = self;
        let ssh_public_key = {
            let ((Some(ssh_private_key_file), _) | (None, Some(ssh_private_key_file))) =
                (ssh_private_key_file, config.ssh_private_key_file_path)
            else {
                return error::NoSshPrivateKeyProvidedSnafu.fail();
            };
            derive_ssh_pubkey(&ssh_private_key_file).await?
        };

        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());
        let pod_name =
            pod_name.filter(|s| !s.is_empty()).unwrap_or_else(|| config.default_pod_name.clone());

        // We use a single shell command to:
        // 1. Create .ssh directory
        // 2. Append the key to authorized_keys
        // 3. Set correct permissions (SSH is picky about 700/600)
        let auth_command = [
            "sh".to_string(),
            "-c".to_string(),
            [
                "mkdir -p ~/.ssh",
                "chmod 700 ~/.ssh",
                &format!("echo '{ssh_public_key}' >> ~/.ssh/authorized_keys"),
                "chmod 600 ~/.ssh/authorized_keys",
            ]
            .join(" && "),
        ];

        let pods = Api::<Pod>::namespaced(kube_client, &namespace);
        let _unused = pods
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;

        let attached = pods
            .exec(&pod_name, auth_command, &AttachParams::default())
            .await
            .with_context(|_| error::UploadSshKeySnafu { namespace, pod_name })?;

        let _unused = attached.join().await;

        Ok(())
    }
}

async fn derive_ssh_pubkey<P>(ssh_private_key_file: P) -> Result<String, Error>
where
    P: AsRef<Path>,
{
    // 1. Read the private key file asynchronously
    let private_key_content = tokio::fs::read_to_string(ssh_private_key_file.as_ref())
        .await
        .with_context(|_| error::ReadSshPrivateKeySnafu {
            file_path: ssh_private_key_file.as_ref().to_path_buf(),
        })?
        .trim()
        .to_string();

    // 2. Parse the private key (supports OpenSSH, PKCS#8, etc.)
    // Note: If the key is encrypted, this will require a passphrase.
    let mut private_key = ssh_key::PrivateKey::from_openssh(&private_key_content)
        .context(error::ParseSshPrivateKeySnafu)?;
    private_key.set_comment("");

    // 3. Extract the public key and format it as OpenSSH (e.g., "ssh-rsa AAA...")
    let public_key = private_key.public_key();
    let openssh_pub = public_key.to_openssh().context(error::SerializeSshPublicKeySnafu)?;

    Ok(openssh_pub)
}
