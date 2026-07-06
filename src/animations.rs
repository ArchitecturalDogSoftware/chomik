use bevy::prelude::*;

pub fn init(application: &mut App) {
    application //
        .add_systems(Startup, self::startup)
        .add_systems(Update, self::animate_sprite);
}

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    tracing::info!("Made with spiders 🕷️🕸️🏳️‍⚧️");

    commands.spawn(Camera2d);

    let animations = chomik_extract::AnimationSet::extract_from_msi(std::io::BufReader::new(
        std::fs::File::open("./ChomikBox.msi").unwrap(),
    ))
    .unwrap();

    let animation = match &animations.file_drop {
        chomik_extract::Animation::OneShot(one_shot) => one_shot,
        chomik_extract::Animation::Looping(_) => panic!(),
    };
    let animation = Animation::from_sequence(&animation.sequence, asset_server).unwrap();

    commands.spawn((
        Sprite::from_image(animation.frames[0].clone()),
        animation,
        AnimationTimer(Timer::from_seconds(1.0 / 20.0, TimerMode::Repeating)),
    ));
}

#[expect(clippy::needless_pass_by_value, reason = "`Res` _is_ a reference type")]
fn animate_sprite(time: Res<Time>, mut query: Query<(&mut Animation, &mut AnimationTimer, &mut Sprite)>) {
    for (mut animation, mut timer, mut sprite) in &mut query {
        timer.tick(time.delta());

        if timer.just_finished() {
            animation.step_idx();
            sprite.image = animation.get();
        }
    }
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

#[derive(Component)]
struct Animation {
    frames: Box<[Handle<Image>]>,
    current_frame: usize,
}

impl Animation {
    fn step_idx(&mut self) {
        if self.current_frame == self.frames.len() - 1 {
            self.current_frame = 0;
        } else {
            self.current_frame += 1;
        }
    }

    fn get(&self) -> Handle<Image> {
        self.frames[self.current_frame].clone()
    }

    #[expect(clippy::needless_pass_by_value, reason = "`Res` _is_ a reference type")]
    fn from_sequence(
        sequence: &chomik_extract::Sequence,
        asset_server: Res<AssetServer>,
    ) -> Result<Self, image::ImageError> {
        let frames = sequence
            .images
            .iter()
            .map(|image: &chomik_extract::Image| {
                let image: image::RgbaImage = image.to_rgba()?;
                // This may not be what Bevy is asking for when it asks for "sRGB."
                let is_srgb = image.color_space() == image::metadata::Cicp::SRGB;
                let image: bevy::image::Image =
                    Image::from_dynamic(image.into(), is_srgb, bevy::asset::RenderAssetUsages::default());
                Ok(asset_server.add(image))
            })
            .collect::<image::ImageResult<_>>()?;

        Ok(Self { frames, current_frame: 0 })
    }
}
