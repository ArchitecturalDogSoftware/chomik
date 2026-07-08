use std::time::Duration;

use bevy::prelude::*;
use rand::RngExt;
use rand::distr::weighted::WeightedIndex;

pub fn init(application: &mut App) {
    application //
        .add_systems(Startup, self::startup)
        .add_systems(Update, self::animate_sprite);
}

#[expect(clippy::needless_pass_by_value, reason = "`Res` is used for querying")]
fn startup(mut commands: Commands, asset_server: Res<AssetServer>, mut windows: Query<&mut Window>) {
    tracing::info!("Made with spiders 🕷️🕸️🏳️‍⚧️");

    commands.spawn(Camera2d);

    let animations = chomik_extract::AnimationSet::extract_from_msi(std::io::BufReader::new(
        std::fs::File::open("./ChomikBox.msi").unwrap(),
    ))
    .unwrap();

    let idle_animations = IdleAnimations::from_set(&animations, asset_server.as_ref()).unwrap();

    commands.spawn((
        Sprite::from_image(idle_animations.starting_image()),
        idle_animations,
        State::default(),
        AnimationTimer::from_fps(20.0),
    ));

    for mut window in windows {
        window.visible = true;
    }
}

#[expect(clippy::needless_pass_by_value, reason = "`Res` is used for querying")]
fn animate_sprite(
    time: Res<Time>,
    idle_animations: Res<IdleAnimations>,
    mut query: Query<(&mut State, &mut AnimationTimer, &mut Sprite)>,
) {
    for (mut state, mut animation_timer, mut sprite) in &mut query {
        animation_timer.tick(time.delta());
        state.tick(&idle_animations, time.delta(), &animation_timer, &mut sprite);
    }
}

#[derive(Component)]
enum State {
    Idle(IdleState),
}

impl State {
    fn tick(
        &mut self,
        idle_animations: &IdleAnimations,
        delta: Duration,
        // Expected to have already been ticked.
        animation_timer: &AnimationTimer,
        sprite: &mut Sprite,
    ) {
        let should_return_to_idle = match self {
            Self::Idle(idle_state) => idle_state.tick(idle_animations, delta, animation_timer, sprite),
        };

        if should_return_to_idle {
            *self = Self::Idle(IdleState::default());
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::Idle(IdleState::default())
    }
}

enum IdleState {
    Main { timer: Timer, frame_idx: usize },
    Random { animation_idx: usize, frame_idx: usize },
}

impl IdleState {
    fn tick(
        &mut self,
        idle_animations: &IdleAnimations,
        delta: Duration,
        // Expected to have already been ticked.
        animation_timer: &AnimationTimer,
        sprite: &mut Sprite,
    ) -> bool {
        let switch = match self {
            Self::Main { timer, frame_idx } => {
                timer.tick(delta);

                if timer.just_finished() {
                    true
                } else {
                    if animation_timer.just_finished() {
                        let animation = &idle_animations.main;

                        *frame_idx += 1;
                        if *frame_idx == animation.frames.len() {
                            *frame_idx = 0;
                        }

                        sprite.image = animation.frames[*frame_idx].clone();
                    }

                    false
                }
            }
            Self::Random { animation_idx, frame_idx } => {
                if animation_timer.just_finished() {
                    let animation = &idle_animations.animations[*animation_idx];

                    *frame_idx += 1;
                    if *frame_idx == animation.frames.len() {
                        true
                    } else {
                        sprite.image = animation.frames[*frame_idx].clone();
                        false
                    }
                } else {
                    false
                }
            }
        };

        if switch {
            *self = match self {
                Self::Main { .. } => {
                    let animation_idx = idle_animations.get_rand_idx();

                    tracing::trace!("switching to idle animation {animation_idx}");

                    Self::Random { animation_idx, frame_idx: 0 }
                }
                Self::Random { .. } => {
                    tracing::trace!(
                        "switching to main idle animation for {} seconds",
                        idle_animations.main_duration.as_secs_f64(),
                    );

                    Self::Main { timer: Timer::new(idle_animations.main_duration, TimerMode::Once), frame_idx: 0 }
                }
            };
        }

        false
    }
}

impl Default for IdleState {
    fn default() -> Self {
        Self::Main { timer: Timer::new(Duration::ZERO, TimerMode::Once), frame_idx: 0 }
    }
}

#[derive(Resource)]
#[component(immutable)]
struct IdleAnimations {
    main: Animation,
    main_duration: Duration,
    animations: Box<[Animation]>,
    weighted_distribution: WeightedIndex<u64>,
}

impl IdleAnimations {
    fn new(
        main: Animation,
        main_duration: Duration,
        animations: Box<[Animation]>,
        // Must be of the same length as `animations`.
        animation_weights: impl IntoIterator<Item = u64>,
    ) -> Result<Self, rand::distr::weighted::Error> {
        let weighted_distribution = WeightedIndex::new(animation_weights)?;

        Ok(Self { main, main_duration, animations, weighted_distribution })
    }

    fn from_pairs(
        main: (Duration, Animation),
        animations: impl IntoIterator<Item = (u64, Animation)>,
    ) -> Result<Self, rand::distr::weighted::Error> {
        let (main_duration, main) = main;
        let (animation_weights, animations): (Vec<_>, Vec<_>) = animations.into_iter().unzip();

        Self::new(main, main_duration, animations.into_boxed_slice(), animation_weights)
    }

    fn from_set(
        chomik_extract::AnimationSet {
            main_idle: (main_duration, main_animation),
            idle,
            ..
        }: &chomik_extract::AnimationSet,
        asset_server: &AssetServer,
    ) -> Result<Self, rand::distr::weighted::Error> {
        let main_animation = Animation::from_animation_lossy(main_animation, asset_server).unwrap();

        let mut error: Option<image::ImageError> = None;
        let (animation_weights, animations): (Vec<u64>, Vec<Animation>) = idle
            .iter()
            .map(|(duration, animation)| -> Result<(u64, Animation), image::ImageError> {
                let animation = Animation::from_animation_lossy(animation, asset_server)?;
                Ok((*duration, animation))
            })
            .map_while(|res| match res {
                Ok(v) => Some(v),
                Err(e) => {
                    error = Some(e);
                    None
                }
            })
            .unzip();

        // Will continue anyways.
        if let Some(error) = error {
            tracing::error!("idle animation extraction failed: {error}");
        }

        Self::new(main_animation, *main_duration, animations.into_boxed_slice(), animation_weights)
    }

    fn get_rand_idx(&self) -> usize {
        rand::rng().sample(&self.weighted_distribution)
    }

    fn starting_image(&self) -> Handle<Image> {
        self.main.frames[0].clone()
    }
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

impl AnimationTimer {
    fn from_fps(fps: f32) -> Self {
        Self(Timer::from_seconds(1.0 / fps, TimerMode::Repeating))
    }
}

#[derive(Component)]
struct Animation {
    frames: Box<[Handle<Image>]>,
}

impl Animation {
    // TO-DO: maybe rename to `from_extracted_sequence`?
    fn from_sequence(
        sequence: &chomik_extract::Sequence,
        asset_server: &AssetServer,
    ) -> Result<Self, image::ImageError> {
        Self::from_images(&sequence.images, asset_server)
    }

    // Will lose metadata for looping animations
    fn from_animation_lossy(
        animation: &chomik_extract::Animation,
        asset_server: &AssetServer,
    ) -> Result<Self, image::ImageError> {
        match animation {
            chomik_extract::Animation::OneShot(one_shot) => Self::from_sequence(&one_shot.sequence, asset_server),
            chomik_extract::Animation::Looping(looping) => Self::from_looping_lossy(looping, asset_server),
        }
    }

    // Will lose metadata.
    fn from_looping_lossy(
        sequence: &chomik_extract::LoopingAnimation,
        asset_server: &AssetServer,
    ) -> Result<Self, image::ImageError> {
        Self::from_images(
            sequence.entrance.images.iter().chain(sequence.looping.images.iter()).chain(sequence.exit.images.iter()),
            asset_server,
        )
    }

    // Internal
    fn from_images<'seq>(
        sequence: impl IntoIterator<Item = &'seq chomik_extract::Image>,
        asset_server: &AssetServer,
    ) -> Result<Self, image::ImageError> {
        let frames = sequence
            .into_iter()
            .map(|image: &'seq chomik_extract::Image| {
                let image: image::RgbaImage = image.to_rgba()?;
                // This may not be what Bevy is asking for when it asks for "sRGB."
                let is_srgb = image.color_space() == image::metadata::Cicp::SRGB;
                let image: bevy::image::Image =
                    Image::from_dynamic(image.into(), is_srgb, bevy::asset::RenderAssetUsages::default());

                Ok(asset_server.add(image))
            })
            .collect::<image::ImageResult<_>>()?;

        Ok(Self { frames })
    }
}
