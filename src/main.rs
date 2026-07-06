mod animations;
mod window;

fn main() {
    let mut application = bevy::app::App::new();

    crate::window::init(&mut application);
    crate::animations::init(&mut application);

    application.run();
}
