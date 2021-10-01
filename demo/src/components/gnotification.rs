use gtk::gio;
use gtk::glib::{self, clone};
use gtk::prelude::*;

pub fn init_app(app: &gtk::Application) {
    let action = gio::SimpleAction::new("activated", Some(glib::VariantTy::new("s").unwrap()));
    action.connect_activate(|_, val| {
        if let Some(val) = val {
            println!("{}", val.str().unwrap_or(""));
        }
    });
    app.add_action(&action);
}

pub fn build_ui(app: &gtk::Application) -> impl IsA<gtk::Widget> {
    let box_layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .baseline_position(gtk::BaselinePosition::Center)
        .spacing(12)
        .build();

    box_layout.append(&gtk::Label::new(Some("GNotification")));

    let button = gtk::Button::builder().label("Send notification").build();

    button.connect_clicked(clone!(@weak app => move |_| {
        let notification = gio::Notification::new("Hello from Linux");
        notification.set_body(Some("This notification was sent from a Linux application using gio::GNotification."));
        notification.add_button_with_target_value("Yes", "app.activated", Some(&"yes".to_variant()));
        notification.add_button_with_target_value("No", "app.activated", Some(&"no".to_variant()));
        notification.set_default_action_and_target_value("app.activated", Some(&"default".to_variant()));
        app.send_notification(None, &notification);
    }));

    box_layout.append(&button);

    box_layout
}
