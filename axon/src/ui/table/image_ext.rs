use comfy_table::{Cell, ContentArrangement};

use crate::config::Spec;

pub trait ImageExt {
    fn render_table(&self) -> String;
}

impl ImageExt for Vec<Spec> {
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
