use bevy::app::{App, AppExit};

mod window;

mod capability {}

fn main() -> AppExit {
    println!("Made with spiders 🕷️🕸️🏳️‍⚧️");

    let mut application = App::new();

    crate::window::init(&mut application);

    application.run()
}
