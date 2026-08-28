use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use zentty_ghostty::GhosttySurface;

const CURSOR_SIZE: usize = 32;
const CURSOR_DIMENSION: i32 = 32;
const HOTSPOT: i32 = 16;
const STRIDE: usize = CURSOR_SIZE * 4;

const OUTLINE: [u8; 4] = [12, 12, 12, 255];
const CORE: [u8; 4] = [255, 255, 255, 255];

pub(crate) fn install(surface: &GhosttySurface) {
    let cursor = outlined_text_cursor();
    surface.on_pointer_cursor_changed(move |widget| {
        if should_substitute(rendered_cursor_name(widget).as_deref()) {
            widget.set_cursor(Some(&cursor));
            eprintln!("zentty-linux: terminal-pointer semantic=text rendered=outlined-ibeam");
        }
    });
}

fn should_substitute(rendered_name: Option<&str>) -> bool {
    rendered_name == Some("text")
}

fn rendered_cursor_name(widget: &gtk::Widget) -> Option<String> {
    widget
        .cursor()
        .and_then(|cursor| cursor.name())
        .map(|name| name.to_string())
}

fn outlined_text_cursor() -> gdk::Cursor {
    let pixels = outlined_ibeam_pixels();
    let bytes = glib::Bytes::from_owned(pixels);
    let texture = gdk::MemoryTexture::new(
        CURSOR_DIMENSION,
        CURSOR_DIMENSION,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        STRIDE,
    );
    let fallback = gdk::Cursor::from_name("text", None);
    gdk::Cursor::from_texture(&texture, HOTSPOT, HOTSPOT, fallback.as_ref())
}

fn outlined_ibeam_pixels() -> Vec<u8> {
    let mut pixels = vec![0_u8; CURSOR_SIZE * STRIDE];

    // A two-tone I-beam keeps a white selection affordance over dark terminal
    // themes and a dark silhouette over light themes. The one-pixel outline is
    // deliberately continuous around the three-pixel core.
    fill_rect(&mut pixels, 7, 4, 25, 10, OUTLINE);
    fill_rect(&mut pixels, 13, 4, 19, 28, OUTLINE);
    fill_rect(&mut pixels, 7, 22, 25, 28, OUTLINE);

    fill_rect(&mut pixels, 9, 6, 23, 8, CORE);
    fill_rect(&mut pixels, 15, 6, 17, 26, CORE);
    fill_rect(&mut pixels, 9, 24, 23, 26, CORE);
    pixels
}

fn fill_rect(
    pixels: &mut [u8],
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    color: [u8; 4],
) {
    debug_assert!(left < right && top < bottom);
    debug_assert!(right <= CURSOR_SIZE && bottom <= CURSOR_SIZE);
    for y in top..bottom {
        for x in left..right {
            let offset = y * STRIDE + x * 4;
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
        let offset = y * STRIDE + x * 4;
        pixels[offset..offset + 4].try_into().expect("RGBA pixel")
    }

    #[test]
    fn outlined_ibeam_has_transparent_exterior_dark_silhouette_and_white_core() {
        let pixels = outlined_ibeam_pixels();
        assert_eq!(pixels.len(), CURSOR_SIZE * CURSOR_SIZE * 4);
        assert_eq!(pixel(&pixels, 0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(&pixels, 13, 16), OUTLINE);
        assert_eq!(pixel(&pixels, 15, 16), CORE);
        assert_eq!(pixel(&pixels, 7, 5), OUTLINE);
        assert_eq!(pixel(&pixels, 9, 6), CORE);
        assert_eq!(pixel(&pixels, 24, 27), OUTLINE);
        assert_eq!(pixel(&pixels, 23, 28), [0, 0, 0, 0]);
    }

    #[test]
    fn substitution_is_scoped_to_the_native_text_cursor() {
        assert!(should_substitute(Some("text")));
        assert!(!should_substitute(Some("pointer")));
        assert!(!should_substitute(Some("none")));
        assert!(!should_substitute(Some("col-resize")));
        assert!(!should_substitute(None));
    }
}
