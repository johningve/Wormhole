// Copyright (c) 2022 John Ingve Olsen
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use gtk::prelude::*;

mod components;

fn main() {
    let app = gtk::Application::builder()
        .application_id("com.github.Raytar.WSLPortalDemo")
        .build();
    components::init_app(&app);
    app.connect_activate(components::build_ui);
    app.run();
}
