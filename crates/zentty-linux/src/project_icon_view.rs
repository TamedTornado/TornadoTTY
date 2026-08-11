use std::path::Path;

use gtk::prelude::*;

pub(crate) fn picture(name: &str, size: i32) -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_widget_name(name);
    picture.set_size_request(size, size);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_can_shrink(true);
    picture.set_visible(false);
    picture
}

pub(crate) fn configure(picture: &gtk::Picture, path: Option<&Path>, owner: &str) {
    let Some(path) = path else {
        picture.set_paintable(None::<&gtk::gdk::Paintable>);
        picture.set_visible(false);
        return;
    };
    let file = gtk::gio::File::for_path(path);
    match gtk::gdk::Texture::from_file(&file) {
        Ok(texture) => {
            picture.set_paintable(Some(&texture));
            picture.set_visible(true);
            picture.set_tooltip_text(path.file_name().and_then(|name| name.to_str()));
            eprintln!(
                "zentty-linux: project-icon-projected owner={owner} path={} decoded=true",
                path.display()
            );
        }
        Err(error) => {
            picture.set_paintable(None::<&gtk::gdk::Paintable>);
            picture.set_visible(false);
            eprintln!(
                "zentty-linux: project-icon-projected owner={owner} path={} decoded=false error={error}",
                path.display()
            );
        }
    }
}
