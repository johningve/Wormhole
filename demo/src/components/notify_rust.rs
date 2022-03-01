use gtk::{
    glib::{self, clone},
    prelude::*,
};

pub fn init_app(_app: &gtk::Application) {}

pub fn build_ui(_app: &gtk::Application) -> impl IsA<gtk::Widget> {
    let box_layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .baseline_position(gtk::BaselinePosition::Top)
        .spacing(12)
        .build();

    simple_notification(&box_layout);
    icon_notification(&box_layout);
    action_notification(&box_layout);

    box_layout
}

fn simple_notification(parent: &gtk::Box) {
    let layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();

    let button = gtk::Button::builder().label("Simple Notification").build();
    let label = gtk::Label::builder()
        .wrap(true)
        .width_chars(50)
        .max_width_chars(50)
        .build();

    button.connect_clicked(clone!(@weak label => move |_| {
        match notify_rust::Notification::new()
            .summary("Simple Notification")
            .body("This is a simple notification with no icon or actions :^)")
            .show()
        {
            Ok(_) => label.set_label("Notification sent :^)"),
            Err(e) => label.set_label(&format!("Failed to send notification: {}\n:^(", e)),
        }
    }));

    layout.append(&button);
    layout.append(&label);
    parent.append(&layout);
}

fn icon_notification(parent: &gtk::Box) {
    let layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();

    let button = gtk::Button::builder().label("Icon Notification").build();
    let label = gtk::Label::builder()
        .wrap(true)
        .width_chars(50)
        .max_width_chars(50)
        .build();

    button.connect_clicked(clone!(@weak label => move |_| {
        match notify_rust::Notification::new()
            .summary("Icon Notification")
            .body("This is a notification with an icon :^)")
            .icon("face-glasses")
            .show()
        {
            Ok(_) => label.set_label("Notification sent :^)"),
            Err(e) => label.set_label(&format!("Failed to send notification: {}\n:^(", e)),
        }
    }));

    layout.append(&button);
    layout.append(&label);
    parent.append(&layout);
}

fn action_notification(parent: &gtk::Box) {
    let layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();

    let button = gtk::Button::builder().label("Action Notification").build();
    let label = gtk::Label::builder()
        .wrap(true)
        .width_chars(50)
        .max_width_chars(50)
        .build();

    button.connect_clicked(clone!(@weak label => move |_| {
        match notify_rust::Notification::new()
            .summary("Action Notification")
            .body("This is a notification with actions :^)")
            .icon("face-glasses")
            .action("ok", "Ok")
            .action("cancel", "Cancel")
            .action("default", "")
            .show()
        {
            Ok(handle) => {
                label.set_label("Notification sent :^)");

                let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

                std::thread::spawn(move || {
                    handle.wait_for_action(|action| tx.send(action.to_string()).unwrap());
                });

                rx.attach(None, move |action| {
                    match action.as_str() {
                        "ok" => label.set_label("You clicked 'Ok' :^)"),
                        "cancel" => label.set_label("You clicked 'Cancel' :^)"),
                        "default" => label.set_label("You clicked on the notification :^)"),
                        "__closed" => label.set_label("The notification was dismissed :^)"),
                        _ => label.set_label(&format!("Unknown action {} :^|", action)),
                    }

                    glib::Continue(false)
                });
            },
            Err(e) => label.set_label(&format!("Failed to send notification: {}\n:^(", e)),
        }
    }));

    layout.append(&button);
    layout.append(&label);
    parent.append(&layout);
}
