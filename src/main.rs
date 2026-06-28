use bevy::app::{App, AppExit, Startup};

mod window;

mod capability {}

fn main() -> AppExit {
    let mut application = App::new();

    crate::window::init(&mut application);

    application.add_systems(Startup, crate::init).run()
}

fn init() {
    tracing::info!("Made with spiders 🕷️🕸️🏳️‍⚧️");
}
