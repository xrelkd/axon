<h1 align="center">Axon</h1>

Axon is a powerful command-line tool designed to simplify your interactions with Kubernetes, providing a streamlined experience for common operations like attaching to pods, port-forwarding, SSH access, and managing container images.

## Quick Start

To quickly try out Axon, you can run it directly using `cargo`:

```bash
cargo run -- list
```

This command will list all pods in your current Kubernetes context's default namespace.

## Features

- **Attach to Pods**: Connect to an interactive shell within a specified Kubernetes pod.
- **List Pods**: Easily list pods across different namespaces in your Kubernetes cluster.
- **Port Forwarding**: Establish local port-forwarding connections to services running inside Kubernetes pods, based on pod annotations.
- **Image Management**: List all predefined container image specifications configured in Axon.
- **SSH Access**: Securely interact with Kubernetes pods using SSH for shell access or file transfers.
  - **SSH Shell**: Open an interactive SSH shell on a remote pod.
  - **SSH Put**: Upload files from your local machine to a remote pod.
  - **SSH Get**: Download files from a remote pod to your local machine.

## Installation

You can install Axon from source using `cargo`:

```bash
cargo install --path .
```

This will compile Axon and place the `axon` executable in your Cargo bin directory (e.g., `~/.cargo/bin`), making it available in your shell's PATH.

## Usage/Examples

Axon provides a rich set of commands, each with its own help message. You can get a general overview of all commands by running:

```bash
axon help
```

```text
Axon: A command-line tool designed to simplify your interactions with Kubernetes

Usage: axon [OPTIONS] <COMMAND>

Commands:
  attach          Connects to an interactive shell in a specified Kubernetes pod
  list            List pods from Kubernetes
  port-forward    Establishes port-forwarding connections to a Kubernetes pod
  image           Lists all predefined container image specifications in the application's configuration
  ssh             Securely interact with Kubernetes pods using SSH for shell access and file transfers
  help            Print this message or the help of the given subcommand(s)

Options:
      --config-file <CONFIG_FILE>  Path to the configuration file
  -l, --log-level <LOG_LEVEL>      Set the logging level [env: AXON_LOG_LEVEL=] [default: info]
  -h, --help                       Print help
  -V, --version                    Print version
```

Here are some common usage examples:

- **List pods in a specific namespace:**

  ```bash
  axon list --namespace my-app-namespace
  ```

- **Attach to an interactive shell in a pod:**

  ```bash
  axon attach --namespace default my-database-pod
  ```

- **Forward local ports to a pod (configured via annotations):**

  ```bash
  axon port-forward my-web-app-pod
  ```

- **Open an SSH shell on a pod:**

  ```bash
  axon ssh shell my-remote-server-pod --user admin
  ```

- **Upload a file via SSH:**

  ```bash
  axon ssh put ./local/path/to/file.txt my-pod:/remote/path/to/destination.txt
  ```

- **List predefined container images:**
  ```bash
  axon image list
  ```

## License

Axon is dual-licensed under the **MIT License** and the **Apache License, Version 2.0**. You may choose to use this software under the terms of either license.

### MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files...
_See the [LICENSE-MIT](LICENSE-MIT) file for details._

### Apache License, Version 2.0

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License...
_See the [LICENSE-APACHE](LICENSE-APACHE) file for details._
