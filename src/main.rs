mod animations;
mod settings;
mod window;

fn main() {
    let mut application = bevy::app::App::new();

    crate::settings::init(&mut application);
    crate::window::init(&mut application);
    crate::animations::init(&mut application);

    application.run();
}
