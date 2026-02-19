use std::{collections::BTreeMap, time::Duration};

use axon_base::consts::{
    DEFAULT_INTERACTIVE_SHELL,
    k8s::{annotations, labels},
};
use clap::{ArgAction, Args, Parser};
use k8s_openapi::api::core::v1::{Container, ContainerPort, Pod, PodSpec};
use kube::{
    Api,
    api::{ObjectMeta, PostParams},
};
use snafu::{OptionExt, ResultExt};

use crate::{
    cli::{Error, error, internal::ApiPodExt},
    config::{Config, ImagePullPolicy, PortMapping, ServicePorts, Spec},
    pod_console::PodConsole,
};

#[derive(Args, Clone)]
pub struct CreateCommand {
    #[arg(
        short = 'n',
        long = "namespace",
        default_value = "",
        help = "Namespace used to create a pod, use current namespace if not provided"
    )]
    pub namespace: Option<String>,

    #[arg(short = 'p', long = "pod-name", help = "Pod name")]
    pub pod_name: Option<String>,

    #[arg(
        short = 'a',
        long = "auto-attach",
        help = "Attach to the pod automatically after creating"
    )]
    pub auto_attach: bool,

    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "90",
        help = "The maximum time in seconds to wait before timing out"
    )]
    pub timeout_secs: u64,

    #[command(subcommand)]
    pub mode: Option<Mode>,
}

impl CreateCommand {
    #[allow(clippy::too_many_lines)]
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, auto_attach, timeout_secs, mode } = self;
        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());
        let pod_name =
            pod_name.filter(|s| !s.is_empty()).unwrap_or_else(|| config.default_pod_name.clone());

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

        let interactive_shell =
            (!target.interactive_shell.is_empty()).then_some(target.interactive_shell);

        // Apply to Cluster
        let api = Api::<Pod>::namespaced(kube_client, &namespace);

        let pod_exists = api.get(&pod_name).await.is_ok();
        if pod_exists {
            tracing::info!("pod/{pod_name} has been created in namespace {namespace}");
        } else {
            // Construct the Pod Manifest
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

            let labels = BTreeMap::from_iter([(
                labels::MANAGED_BY.to_string(),
                axon_base::PROJECT_NAME.to_string(),
            )]);

            let annotations = {
                let shell_json = serde_json::Value::from(interactive_shell.clone()).to_string();
                [
                    (annotations::SHELL_INTERACTIVE.to_string(), shell_json),
                    (annotations::VERSION.to_string(), "1.0.0".to_string()),
                ]
                .into_iter()
                .chain(port_mappings.iter().flatten().map(PortMapping::to_kubernetes_annotation))
                .chain(target.service_ports.to_kubernetes_annotation())
                .collect::<BTreeMap<_, _>>()
            };

            let pod = Pod {
                metadata: ObjectMeta {
                    name: Some(pod_name.clone()),
                    namespace: Some(namespace.clone()),
                    labels: Some(labels),
                    annotations: Some(annotations),
                    ..ObjectMeta::default()
                },
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "axon-container".to_string(),
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
            };

            let _resource =
                api.create(&PostParams::default(), &pod).await.context(error::CreatePodSnafu {
                    pod_name: pod_name.clone(),
                    namespace: namespace.clone(),
                })?;

            tracing::info!("pod/{pod_name} created in namespace {namespace}");
        }

        if auto_attach {
            let _pod = api
                .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
                .await?;
            let interactive_shell = interactive_shell.unwrap_or_else(|| {
                DEFAULT_INTERACTIVE_SHELL.iter().map(ToString::to_string).collect()
            });
            PodConsole::new(api, pod_name, namespace, interactive_shell)
                .run()
                .await
                .map_err(Error::from)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Parser)]
pub enum Mode {
    Default,
    Preset {
        #[arg(help = "Container spec name")]
        spec_name: String,
    },
    Manual {
        #[arg(long = "image", default_value = "docker.io/alpine:3.23", help = "Container image")]
        image: String,

        #[arg(
            long = "image-pull-policy",
            default_value = "IfNotPresent",
            help = "Image pull policy"
        )]
        image_pull_policy: ImagePullPolicy,

        #[arg(long = "command", action = ArgAction::Append, default_value = "sh", help = "Command")]
        command: Vec<String>,

        #[arg(
            long = "args",
            action = ArgAction::Append,
            default_values_t = ["-c".to_string(), "while true; do sleep 1; done".to_string()],
            help = "Arguments"
        )]
        args: Vec<String>,

        #[arg(long = "shell", action = ArgAction::Append, default_value = "/bin/sh", help = "Interactive shell")]
        interactive_shell: Vec<String>,

        #[arg(long = "ports", action = ArgAction::Append, help = "Port mappings")]
        port_mappings: Vec<PortMapping>,
    },
}
