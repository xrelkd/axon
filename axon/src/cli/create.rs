//! This module defines the `create` CLI command, responsible for provisioning
//! and optionally attaching to temporary Kubernetes pods.
//!
//! It handles the parsing of command-line arguments related to pod creation,
//! resolves pod identity, constructs the Kubernetes Pod manifest based on
//! user-defined specifications (default, preset, or manual), and interacts
//! with the Kubernetes API to create the pod. Optionally, it can automatically
//! attach to the pod's console upon successful creation.

use std::{collections::BTreeMap, time::Duration};

use clap::{ArgAction, Args, Parser};
use k8s_openapi::api::core::v1::{Container, ContainerPort, Pod, PodSpec};
use kube::{
    Api,
    api::{ObjectMeta, PostParams},
};
use snafu::{OptionExt, ResultExt};

use crate::{
    PROJECT_NAME, PROJECT_VERSION,
    cli::{
        Error, error,
        internal::{ApiPodExt, ResolvedResources, ResourceResolver},
    },
    config::{Config, ImagePullPolicy, PortMapping, ServicePorts, Spec},
    consts::{
        DEFAULT_INTERACTIVE_SHELL,
        k8s::{annotations, labels},
    },
    pod_console::PodConsole,
};

const DEFAULT_CONTAINER_NAME: &str = "axon-container";

/// Represents the `create` command in the CLI, used for provisioning new
/// temporary Kubernetes pods.
///
/// This struct defines the command-line arguments available for configuring
/// the new pod, such as its namespace, name, automatic attachment behavior,
/// and timeout settings.
#[derive(Args, Clone)]
pub struct CreateCommand {
    /// Kubernetes namespace to create the pod in. Defaults to the current
    /// Kubernetes context's namespace.
    #[arg(
        short = 'n',
        long = "namespace",
        default_value = "",
        help = "Kubernetes namespace to create the pod in. Defaults to the current Kubernetes \
                context's namespace."
    )]
    pub namespace: Option<String>,

    /// Name for the new temporary pod. If not specified, Axon's default pod
    /// naming convention will be used.
    #[arg(
        short = 'p',
        long = "pod-name",
        help = "Name for the new temporary pod. If not specified, Axon's default pod naming \
                convention will be used."
    )]
    pub pod_name: Option<String>,

    /// Automatically attach to the pod's console after it has been successfully
    /// created and is running.
    #[arg(
        short = 'a',
        long = "auto-attach",
        help = "Automatically attach to the pod's console after it has been successfully created \
                and is running."
    )]
    pub auto_attach: bool,

    /// The maximum time in seconds to wait for the pod to be created and
    /// running before timing out.
    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "90",
        help = "The maximum time in seconds to wait for the pod to be created and running before \
                timing out."
    )]
    pub timeout_secs: u64,

    /// Defines the mode for pod creation, specifying how the pod's image and
    /// configuration are determined.
    #[command(subcommand)]
    pub mode: Option<Mode>,
}

impl CreateCommand {
    /// Executes the `create` command, provisioning a new Kubernetes pod and
    /// optionally attaching to its console.
    ///
    /// This function resolves the target namespace and pod name, determines
    /// the pod specification based on the chosen `Mode` (default, preset, or
    /// manual), constructs the Kubernetes Pod manifest, creates the pod in
    /// the cluster, and if `auto_attach` is true, waits for the pod to be
    /// running and then initiates an interactive console session.
    ///
    /// # Arguments
    ///
    /// * `self` - The `CreateCommand` instance containing the parsed arguments.
    /// * `kube_client` - A Kubernetes client used to interact with the cluster
    ///   API.
    /// * `config` - The application's configuration, used to resolve pod
    ///   specifications.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if:
    /// - A specified preset `spec_name` is not found in the configuration.
    /// - Serialization of the interactive shell command to JSON fails.
    /// - Creation of the pod in Kubernetes fails.
    /// - Waiting for the pod to reach a running state times out or fails.
    /// - Attaching to the pod's console fails.
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, auto_attach, timeout_secs, mode } = self;

        // Resolve Identity
        let ResolvedResources { namespace, pod_name } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, pod_name);

        let target = match mode {
            None | Some(Mode::Default) => config.find_default_spec(),
            Some(Mode::Preset { spec_name }) => config
                .find_spec_by_name(&spec_name)
                .with_context(|| error::SpecNotFoundSnafu { spec_name: spec_name.clone() })?,
            Some(Mode::Manual {
                image,
                image_pull_policy,
                command,
                args,
                interactive_shell,
                port_mappings,
            }) => Spec {
                name: pod_name.clone(),
                image,
                image_pull_policy,
                port_mappings,
                service_ports: ServicePorts::default(),
                command,
                args,
                interactive_shell,
            },
        };

        let interactive_shell = if target.interactive_shell.is_empty() {
            DEFAULT_INTERACTIVE_SHELL.clone()
        } else {
            target.interactive_shell.clone()
        };

        // Apply to Cluster
        let api = Api::<Pod>::namespaced(kube_client, &namespace);

        let pod_exists = api.get(&pod_name).await.is_ok();
        if pod_exists {
            println!("pod/{pod_name} has been created in namespace {namespace}");
        } else {
            // Construct the Pod Manifest
            let pod = build_pod_manifest(&pod_name, &namespace, target, &interactive_shell)?;
            let _resource =
                api.create(&PostParams::default(), &pod).await.context(error::CreatePodSnafu {
                    pod_name: pod_name.clone(),
                    namespace: namespace.clone(),
                })?;

            println!("pod/{pod_name} created in namespace {namespace}");
        }

        if auto_attach {
            let _pod = api
                .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
                .await?;
            PodConsole::new(api, pod_name, namespace, interactive_shell)
                .run()
                .await
                .map_err(Error::from)
        } else {
            Ok(())
        }
    }
}

/// Builds a Kubernetes `Pod` manifest based on the provided specifications.
///
/// This function constructs a `Pod` object, populating its metadata (name,
/// namespace, labels, annotations) and spec (containers, image, command,
/// arguments, ports) according to the `pod_name`, `namespace`, `target`
/// specification, and the interactive shell command.
///
/// # Arguments
///
/// * `pod_name` - The name of the pod to be created.
/// * `namespace` - The Kubernetes namespace where the pod will reside.
/// * `target` - A `Spec` object containing the desired configuration for the
///   pod.
/// * `interactive_shell` - A slice of strings representing the command and
///   arguments for the interactive shell to be used when attaching to the
///   container.
///
/// # Returns
///
/// A `Result` which is `Ok` containing the constructed `Pod` object, or an
/// `Error` if serialization of the interactive shell command fails.
///
/// # Errors
///
/// Returns an `Error` if the `interactive_shell` cannot be serialized into a
/// JSON string for the Kubernetes annotation.
fn build_pod_manifest(
    pod_name: impl Into<String>,
    namespace: impl Into<String>,
    target: Spec,
    interactive_shell: &[String],
) -> Result<Pod, Error> {
    let image = Some(target.image);
    let command = (!target.command.is_empty()).then_some(target.command);
    let args = (!target.args.is_empty()).then_some(target.args);
    let image_pull_policy = Some(target.image_pull_policy.to_string());
    let port_mappings = (!target.port_mappings.is_empty()).then_some(target.port_mappings);
    let container_ports = port_mappings.as_ref().map(|port_mappings| {
        port_mappings
            .iter()
            .map(|port_mapping| ContainerPort {
                container_port: i32::from(port_mapping.container_port),
                ..ContainerPort::default()
            })
            .collect::<Vec<_>>()
    });

    let labels = BTreeMap::from_iter([
        (labels::MANAGED_BY.to_string(), PROJECT_NAME.to_string()),
        (labels::DEFAULT_CONTAINER.to_string(), DEFAULT_CONTAINER_NAME.to_string()),
    ]);

    let annotations = {
        let shell_json = serde_json::to_string(&interactive_shell)
            .context(error::SerializeInteractiveShellSnafu)?;
        [
            (annotations::SHELL_INTERACTIVE.to_string(), shell_json),
            (annotations::VERSION.to_string(), PROJECT_VERSION.to_string()),
        ]
        .into_iter()
        .chain(port_mappings.iter().flatten().map(PortMapping::to_kubernetes_annotation))
        .chain(target.service_ports.to_kubernetes_annotation())
        .collect::<BTreeMap<_, _>>()
    };

    Ok(Pod {
        metadata: ObjectMeta {
            name: Some(pod_name.into()),
            namespace: Some(namespace.into()),
            labels: Some(labels),
            annotations: Some(annotations),
            ..ObjectMeta::default()
        },
        spec: Some(PodSpec {
            containers: vec![Container {
                name: DEFAULT_CONTAINER_NAME.to_string(),
                image,
                image_pull_policy,
                command,
                args,
                ports: container_ports,
                ..Container::default()
            }],
            ..PodSpec::default()
        }),
        ..Pod::default()
    })
}

/// Defines the different modes for creating a Kubernetes pod.
///
/// Users can choose between a default configuration, a predefined preset
/// from the application's configuration, or a fully manual specification
/// of the container image, command, arguments, and port mappings.
#[derive(Clone, Parser)]
pub enum Mode {
    /// Creates a pod using the default image and configuration specified
    /// in the application's configuration.
    Default,
    /// Creates a pod based on a named, predefined specification from the
    /// application's configuration file.
    Preset {
        /// Name of the predefined image specification to use from the
        /// configuration file.
        #[arg(
            help = "Name of the predefined image specification to use from the configuration file."
        )]
        spec_name: String,
    },
    /// Manually specifies all aspects of the pod's container.
    Manual {
        /// Container image to use for the pod (e.g., `ubuntu:latest`,
        /// `myregistry/myimage:v1`).
        #[arg(
            long = "image",
            default_value = "docker.io/alpine:3.23",
            help = "Container image to use for the pod (e.g., `ubuntu:latest`, \
                    `myregistry/myimage:v1`)."
        )]
        image: String,

        /// Policy for pulling the container image (e.g., `Always`,
        /// `IfNotPresent`, `Never`).
        #[arg(
            long = "image-pull-policy",
            default_value = "IfNotPresent",
            help = "Policy for pulling the container image (e.g., `Always`, `IfNotPresent`, \
                    `Never`)."
        )]
        image_pull_policy: ImagePullPolicy,

        /// Command to execute as the container's entrypoint. Can be specified
        /// multiple times for multiple arguments.
        #[arg(
            long = "command",
            action = ArgAction::Append,
            default_value = "sh",
            help = "Command to execute as the container's entrypoint. Can be specified multiple times for multiple arguments."
        )]
        command: Vec<String>,

        /// Arguments to pass to the container's command. Can be specified
        /// multiple times.
        #[arg(
            long = "args",
            action = ArgAction::Append,
            default_values_t = ["-c".to_string(), "while true; do sleep 1; done".to_string()],
            help = "Arguments to pass to the container's command. Can be specified multiple times."
        )]
        args: Vec<String>,

        /// Interactive shell command and arguments to use when attaching to the
        /// container (e.g., `/bin/bash`, `bash -c 'sh'`).
        #[arg(
            long = "shell",
            action = ArgAction::Append,
            default_value = "/bin/sh",
            help = "Interactive shell command and arguments to use when attaching to the container (e.g., `/bin/bash`, `bash -c 'sh'`)."
        )]
        interactive_shell: Vec<String>,

        /// Port mappings to forward from the local machine to the container
        /// (e.g., `8080:80/tcp`). Can be specified multiple times.
        #[arg(
            long = "ports",
            action = ArgAction::Append,
            help = "Port mappings to forward from the local machine to the container (e.g., `8080:80/tcp`). Can be specified multiple times."
        )]
        port_mappings: Vec<PortMapping>,
    },
}
