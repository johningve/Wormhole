use gtk::gio::{Notification, SimpleAction};
use gtk::glib::{self, clone};
use gtk::prelude::*;
use gtk::Application;
use gtk::ApplicationWindow;
use gtk::Button;

fn main() {
    let app = Application::builder()
        .application_id("com.github.Raytar.WSLPortalDemo")
        .build();

    let action = SimpleAction::new("quit", None);
    action.connect_activate(clone!(@weak app => move |_, _| {
        println!("Activated!");
        app.quit();
    }));

    app.add_action(&action);

    app.connect_activate(build_ui);

    app.run();
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("WSLPortal Demo")
        .build();

    let button = Button::builder()
        .label("Press me!")
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    button.connect_clicked(clone!(@weak app => move |button| {
        let notification = Notification::new("You clicked the button!");
        notification.add_button("Ok", "app.quit");
        app.send_notification(None, &notification);

        button.set_label("Hello World!");
    }));

    window.set_child(Some(&button));

    window.present();
}
