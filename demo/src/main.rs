use gtk::gio::{Notification, SimpleAction};
use gtk::glib::{self, clone, VariantTy};
use gtk::prelude::*;
use gtk::Application;
use gtk::ApplicationWindow;
use gtk::Button;

fn main() {
    let app = Application::builder()
        .application_id("com.github.Raytar.WSLPortalDemo")
        .build();

    let quit_action = SimpleAction::new("quit", None);
    quit_action.connect_activate(clone!(@weak app => move |_, _| {
        println!("Goodbye!");
        app.quit();
    }));
    app.add_action(&quit_action);

    let action = SimpleAction::new("activated", Some(VariantTy::new("s").unwrap()));
    action.connect_activate(|_, val| {
        if let Some(val) = val {
            println!("{}", val.str().unwrap_or(""));
        }
    });
    app.add_action(&action);

    app.connect_activate(build_ui);

    app.run();
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("WSLPortal Demo")
        .default_width(200)
        .default_height(180)
        .build();

    let button = Button::builder()
        .label("Press me!")
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    button.connect_clicked(clone!(@weak app => move |_| {
        let notification = Notification::new("Hello from Linux");
        notification.set_body(Some("This notification was sent from a Linux application."));
        notification.add_button_with_target_value("Yes", "app.activated", Some(&"yes".to_variant()));
        notification.add_button_with_target_value("No", "app.activated", Some(&"no".to_variant()));
        notification.add_button("Quit", "app.quit");
        app.send_notification(None, &notification);
    }));

    window.set_child(Some(&button));

    window.present();
}
