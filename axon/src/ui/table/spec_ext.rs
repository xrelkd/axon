//! This module contains extensions for `Spec` related to UI rendering.

use comfy_table::{Cell, ContentArrangement};

use crate::config::Spec;

/// Extension trait for `Spec` to facilitate rendering operations.
pub trait SpecExt {
    /// Renders a vector of `Spec` instances into a formatted table string.
    ///
    /// # Returns
    ///
    /// A `String` containing the table representation of the `Spec` vector.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::net::IpAddr;
    /// use axon::config::{ImagePullPolicy, PortMapping, Spec};
    /// use axon::ui::table::spec_ext::SpecExt;
    ///
    /// let spec1 = Spec {
    ///     name: "my-app".to_string(),
    ///     image: "ubuntu:latest".to_string(),
    ///     image_pull_policy: ImagePullPolicy::Always,
    ///     port_mappings: vec![PortMapping {
    ///         container_port: 3000,
    ///         local_port: 3000,
    ///         address: "127.0.0.1".parse::<IpAddr>().unwrap(),
    ///     }],
    ///     command: vec!["sh".to_string(), "-c".to_string()],
    ///     args: vec!["sleep infinity".to_string()],
    ///     interactive_shell: vec!["bash".to_string()],
    /// };
    ///
    /// let spec2 = Spec {
    ///     name: "another-app".to_string(),
    ///     image: "alpine:latest".to_string(),
    ///     image_pull_policy: ImagePullPolicy::IfNotPresent,
    ///     port_mappings: vec![],
    ///     command: vec![],
    ///     args: vec!["nginx".to_string(), "-g".to_string(), "daemon off;".to_string()],
    ///     interactive_shell: vec![],
    /// };
    ///
    /// let specs = vec![spec1, spec2];
    /// let table_string = specs.render_table();
    /// println!("{}", table_string);
    /// ```
    fn render_table(&self) -> String;
}

impl SpecExt for Vec<Spec> {
    fn render_table(&self) -> String {
        let rows = self
            .iter()
            .map(|image| {
                [
                    Cell::new(&image.name),
                    Cell::new(&image.image),
                    Cell::new(&image.image_pull_policy),
                    Cell::new(image.interactive_shell.join(" ")),
                    Cell::new(image.command.join(" ")),
                    Cell::new(image.args.join(" ")),
                ]
            })
            .collect::<Vec<_>>();

        comfy_table::Table::new()
            .load_preset(comfy_table::presets::NOTHING)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                "NAME",
                "IMAGE",
                "PULL POLICY",
                "INTERACTIVE SHELL",
                "COMMAND",
                "ARGS",
            ])
            .add_rows(rows)
            .to_string()
    }
}
