use gtk::prelude::*;

use crate::theme_catalog::{ThemeColor, ThemePreview};

pub(crate) fn compact_area() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(96);
    area.set_content_height(46);
    area
}

pub(crate) fn detail_area() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(280);
    area.set_content_height(250);
    area.set_hexpand(true);
    area.set_vexpand(true);
    area
}

pub(crate) fn configure_compact(area: &gtk::DrawingArea, theme: Option<&ThemePreview>) {
    let theme = theme.cloned();
    area.set_draw_func(move |_, context, width, height| {
        let Some(theme) = theme.as_ref() else {
            draw_empty(context, width, height);
            return;
        };
        fill(
            context,
            &theme.background,
            0.0,
            0.0,
            f64::from(width),
            f64::from(height),
        );
        let label_width = 22.0;
        draw_text(
            context,
            &theme.foreground,
            "Aa",
            7.0,
            f64::from(height) - 9.0,
            10.0,
            true,
        );
        let swatch_width = (f64::from(width) - label_width) / 8.0;
        let row_height = (f64::from(height) - 2.0) / 2.0;
        for index in 0..16 {
            let row = index / 8;
            let column = index % 8;
            let column_offset = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0][column];
            let row_offset = [0.0, 1.0][row];
            let color = theme.palette.get(index).unwrap_or(&theme.foreground);
            fill(
                context,
                color,
                label_width + column_offset * swatch_width,
                row_offset * (row_height + 2.0),
                swatch_width.ceil(),
                row_height,
            );
        }
    });
    area.queue_draw();
}

pub(crate) fn configure_detail(area: &gtk::DrawingArea, theme: Option<&ThemePreview>) {
    let theme = theme.cloned();
    area.set_draw_func(move |_, context, width, height| {
        let Some(theme) = theme.as_ref() else {
            draw_empty(context, width, height);
            return;
        };
        let width = f64::from(width);
        let height = f64::from(height);
        fill(context, &theme.background, 0.0, 0.0, width, height);

        draw_text(
            context,
            &theme.foreground,
            &theme.name,
            18.0,
            30.0,
            15.0,
            true,
        );
        let prompt = theme.palette.get(2).unwrap_or(&theme.foreground);
        draw_text(context, prompt, "jason@zentty", 18.0, 62.0, 12.0, true);
        draw_text(
            context,
            &theme.foreground,
            ":~/project$ cargo test",
            105.0,
            62.0,
            12.0,
            false,
        );
        draw_text(
            context,
            &theme.foreground,
            "running 42 tests",
            18.0,
            86.0,
            12.0,
            false,
        );

        let selection_background = theme
            .selection_background
            .as_ref()
            .unwrap_or_else(|| theme.palette.get(4).unwrap_or(&theme.foreground));
        let selection_foreground = theme
            .selection_foreground
            .as_ref()
            .unwrap_or(&theme.background);
        fill(
            context,
            selection_background,
            16.0,
            101.0,
            width.min(248.0),
            25.0,
        );
        draw_text(
            context,
            selection_foreground,
            "selected terminal output",
            22.0,
            119.0,
            12.0,
            false,
        );

        let swatch_left = 18.0;
        let swatch_top = height - 72.0;
        let gap = 3.0;
        let swatch_width = ((width - swatch_left * 2.0) - gap * 7.0) / 8.0;
        for index in 0..16 {
            let row = index / 8;
            let column = index % 8;
            let column_offset = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0][column];
            let row_offset = [0.0, 1.0][row];
            let color = theme.palette.get(index).unwrap_or(&theme.foreground);
            fill(
                context,
                color,
                swatch_left + column_offset * (swatch_width + gap),
                swatch_top + row_offset * 25.0,
                swatch_width,
                20.0,
            );
        }

        let cursor = theme.cursor.as_ref().unwrap_or(&theme.foreground);
        fill(context, cursor, 18.0, 139.0, 8.0, 17.0);
        if let Some(cursor_text) = theme.cursor_text.as_ref() {
            draw_text(context, cursor_text, "_", 18.0, 152.0, 10.0, true);
        }
        draw_text(
            context,
            &theme.foreground,
            "cursor",
            32.0,
            153.0,
            12.0,
            false,
        );
    });
    area.queue_draw();
}

fn draw_empty(context: &gtk::cairo::Context, width: i32, height: i32) {
    context.set_source_rgb(0.12, 0.13, 0.15);
    context.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
    let _ = context.fill();
}

fn fill(
    context: &gtk::cairo::Context,
    color: &ThemeColor,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    let (red, green, blue) = color.rgb();
    context.set_source_rgb(red, green, blue);
    context.rectangle(x, y, width, height);
    let _ = context.fill();
}

fn draw_text(
    context: &gtk::cairo::Context,
    color: &ThemeColor,
    text: &str,
    x: f64,
    y: f64,
    size: f64,
    bold: bool,
) {
    let (red, green, blue) = color.rgb();
    context.set_source_rgb(red, green, blue);
    context.select_font_face(
        "monospace",
        gtk::cairo::FontSlant::Normal,
        if bold {
            gtk::cairo::FontWeight::Bold
        } else {
            gtk::cairo::FontWeight::Normal
        },
    );
    context.set_font_size(size);
    context.move_to(x, y);
    let _ = context.show_text(text);
}
