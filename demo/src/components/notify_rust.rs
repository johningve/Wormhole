use gtk::prelude::*;

pub fn init_app(_app: &gtk::Application) {}

pub fn build_ui(_app: &gtk::Application) -> impl IsA<gtk::Widget> {
    let box_layout = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .baseline_position(gtk::BaselinePosition::Top)
        .spacing(12)
        .build();

    box_layout.append(&gtk::Label::new(Some("D-Bus notification")));

    let button = gtk::Button::builder().label("Send notification").build();

    button.connect_clicked(|_| {
        let handle = notify_rust::Notification::new()
            .summary("Hello from Linux")
            .icon("security-high")
            .body("This notification was sent from a Linux application using notify-rust.")
            .action("ok", "Ok")
            .action("default", "")
            .show();

        if let Err(err) = handle {
            println!("failed to send notification: {}", err);
            return;
        }

        let handle = handle.unwrap();

        // spawning a thread for this may be overkill, but we'll block the UI otherwise.
        std::thread::spawn(move || {
            handle.wait_for_action(|action| {
                println!("{}", action);
            })
        });
    });

    box_layout.append(&button);

    box_layout
}
