use gtk::prelude::*;

mod gnotification;
mod notify_rust;

pub fn init_app(app: &gtk::Application) {
    gnotification::init_app(app);
    notify_rust::init_app(app);
}

pub fn build_ui(app: &gtk::Application) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("WSLPortal Demo")
        .default_width(200)
        .default_height(180)
        .build();

    let box_layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .baseline_position(gtk::BaselinePosition::Top)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    box_layout.append(&gnotification::build_ui(app));
    box_layout.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    box_layout.append(&notify_rust::build_ui(app));

    window.set_child(Some(&box_layout));

    window.present();
}
