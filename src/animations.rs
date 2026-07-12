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
fn startup(
    settings: Res<crate::settings::Settings>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    windows: Query<&mut Window>,
) {
    tracing::info!("Made with spiders 🕷️🕸️🏳️‍⚧️");

    commands.spawn(Camera2d);

    let animations = chomik_extract::AnimationSet::extract_from_msi(std::io::BufReader::new(
        std::fs::File::open(settings.msi_path.as_path()).unwrap(),
    ))
    .unwrap();

    let animation_set = AnimationSet::from_set(&animations, asset_server.as_ref());

    commands.spawn((
        Sprite::from_image(animation_set.idle.first_image()),
        State::default_with_timer(&animation_set.idle),
        AnimationTimer::from_fps(20.0),
    ));
    commands.insert_resource(animation_set);

    for mut window in windows {
        window.visible = true;
    }
}

#[expect(clippy::needless_pass_by_value, reason = "`Res` is used for querying")]
fn animate_sprite(
    time: Res<Time>,
    animation_set: Res<AnimationSet>,
    input: Res<ButtonInput<MouseButton>>,
    mut file_dnd_reader: MessageReader<FileDragAndDrop>,
    mut query: Query<(&mut State, &mut AnimationTimer, &mut Sprite)>,
) {
    let dnd_msg = file_dnd_reader.read().last();

    for (mut state, mut animation_timer, mut sprite) in &mut query {
        animation_timer.tick(time.delta());
        state.tick(&animation_set, time.delta(), &input, dnd_msg, &animation_timer, &mut sprite);
    }
}

#[derive(Component)]
enum State {
    Idle(IdleState),
    FileDnd(FileDndState),
    WindowDrag(WindowDragState),
}

impl State {
    fn default_with_timer(idle_animations: &IdleAnimations) -> Self {
        Self::Idle(IdleState::default_with_timer(idle_animations))
    }

    fn tick(
        &mut self,
        animation_set: &AnimationSet,
        delta: Duration,
        input: &ButtonInput<MouseButton>,
        file_dnd_event: Option<&FileDragAndDrop>,
        // Expected to have already been ticked.
        animation_timer: &AnimationTimer,
        sprite: &mut Sprite,
    ) {
        if input.just_pressed(MouseButton::Left) {
            *self = Self::WindowDrag(WindowDragState::grab());
            return;
        }

        if input.just_released(MouseButton::Left) {
            *self = Self::WindowDrag(WindowDragState::drop());
            return;
        }

        if let Some(file_dnd_event) = file_dnd_event
            && !matches!(self, Self::FileDnd(_))
        {
            *self = Self::FileDnd(FileDndState::from_event(&animation_set.file_dnd, file_dnd_event, sprite));
            return;
        }

        let should_return_to_idle = match self {
            Self::Idle(idle_state) => idle_state.tick(&animation_set.idle, delta, animation_timer, sprite),
            Self::FileDnd(file_dnd_state) => {
                file_dnd_state.tick(&animation_set.file_dnd, file_dnd_event, animation_timer, sprite)
            }
            Self::WindowDrag(window_drag_state) => {
                window_drag_state.tick(&animation_set.window_drag, animation_timer, sprite)
            }
        };

        if should_return_to_idle {
            *self = Self::Idle(IdleState::default_with_timer(&animation_set.idle));
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
    fn default_with_timer(idle_animations: &IdleAnimations) -> Self {
        Self::Main { timer: Timer::new(idle_animations.main_duration, TimerMode::Once), frame_idx: 0 }
    }

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

                        sprite.image = animation.get(*frame_idx);
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
                        sprite.image = animation.get(*frame_idx);
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

enum FileDndState {
    HoverStart { frame_idx: usize },
    HoverLoop { frame_idx: usize },
    HoverCancel { frame_idx: usize },
    Drop { frame_idx: usize },
}

impl FileDndState {
    fn from_event(
        file_dnd_animations: &FileDndAnimations,
        file_dnd_event: &FileDragAndDrop,
        sprite: &mut Sprite,
    ) -> Self {
        // TO-DO: this doesn't actually filter for the window being dropped upon. This would be problematic if we
        // spawned multiple windows. So not in scope now, but it would be hilarious to release a swarm of hamsters, so
        // it's worth considering in the future.
        match file_dnd_event {
            FileDragAndDrop::DroppedFile { window: _, path_buf } => {
                tracing::trace!("file '{}' dropped onto a sprite", path_buf.to_string_lossy());

                sprite.image = file_dnd_animations.drop.first_image();
                Self::Drop { frame_idx: 0 }
            }
            FileDragAndDrop::HoveredFile { window: _, path_buf } => {
                tracing::trace!("file '{}' hovered over a sprite", path_buf.to_string_lossy());

                sprite.image = file_dnd_animations.hover_start.first_image();
                Self::HoverStart { frame_idx: 0 }
            }
            FileDragAndDrop::HoveredFileCanceled { window: _ } => {
                tracing::trace!("file hover over sprite canceled");

                sprite.image = file_dnd_animations.hover_cancel.first_image();
                Self::HoverCancel { frame_idx: 0 }
            }
        }
    }

    fn tick(
        &mut self,
        file_dnd_animations: &FileDndAnimations,
        file_dnd_event: Option<&FileDragAndDrop>,
        // Expected to have already been ticked.
        animation_timer: &AnimationTimer,
        sprite: &mut Sprite,
    ) -> bool {
        // TO-DO: wait for existing animation to finish.
        if let Some(event) = file_dnd_event {
            *self = Self::from_event(file_dnd_animations, event, sprite);
            return false;
        }

        if !animation_timer.just_finished() {
            return false;
        }

        match self {
            Self::HoverStart { frame_idx } => {
                let animation = &file_dnd_animations.hover_start;

                *frame_idx += 1;
                if *frame_idx == animation.frames.len() {
                    sprite.image = file_dnd_animations.hover_loop.first_image();
                    *self = Self::HoverLoop { frame_idx: 0 };
                } else {
                    sprite.image = animation.get(*frame_idx);
                }

                false
            }
            Self::HoverLoop { frame_idx } => {
                let animation = &file_dnd_animations.hover_loop;

                *frame_idx += 1;
                if *frame_idx == animation.frames.len() {
                    *frame_idx = 0;
                }

                sprite.image = animation.get(*frame_idx);

                false
            }
            Self::HoverCancel { frame_idx } => {
                let animation = &file_dnd_animations.hover_cancel;

                *frame_idx += 1;
                if *frame_idx == animation.frames.len() {
                    return true;
                }

                sprite.image = animation.get(*frame_idx);

                false
            }
            Self::Drop { frame_idx } => {
                let animation = &file_dnd_animations.drop;

                *frame_idx += 1;
                if *frame_idx == animation.frames.len() {
                    return true;
                }

                sprite.image = animation.get(*frame_idx);

                false
            }
        }
    }
}

enum WindowDragState {
    Grab { frame_idx: usize },
    Looping { frame_idx: usize },
    Drop { frame_idx: usize },
}

impl WindowDragState {
    fn grab() -> Self {
        Self::Grab { frame_idx: 0 }
    }

    fn drop() -> Self {
        Self::Drop { frame_idx: 0 }
    }

    fn tick(
        &mut self,
        window_drag_animations: &WindowDragAnimations,
        // Expected to have already been ticked.
        animation_timer: &AnimationTimer,
        sprite: &mut Sprite,
    ) -> bool {
        if !animation_timer.just_finished() {
            return false;
        }

        match self {
            Self::Grab { frame_idx } => {
                let animation = &window_drag_animations.grab;

                *frame_idx += 1;
                if *frame_idx == animation.frames.len() {
                    sprite.image = window_drag_animations.looping.first_image();
                    *self = Self::Looping { frame_idx: 0 };
                } else {
                    sprite.image = animation.get(*frame_idx);
                }

                false
            }
            Self::Looping { frame_idx } => {
                let animation = &window_drag_animations.looping;

                *frame_idx += 1;
                if *frame_idx == animation.frames.len() {
                    *frame_idx = 0;
                }

                sprite.image = animation.get(*frame_idx);

                false
            }
            Self::Drop { frame_idx } => {
                let animation = &window_drag_animations.drop;

                *frame_idx += 1;
                if *frame_idx == animation.frames.len() {
                    return true;
                }

                sprite.image = animation.get(*frame_idx);

                false
            }
        }
    }
}

#[derive(Resource)]
#[component(immutable)]
struct AnimationSet {
    idle: IdleAnimations,
    file_dnd: FileDndAnimations,
    window_drag: WindowDragAnimations,
}

impl AnimationSet {
    fn from_set(set: &chomik_extract::AnimationSet, asset_server: &AssetServer) -> Self {
        Self {
            idle: IdleAnimations::from_set(set, asset_server).unwrap(),
            file_dnd: FileDndAnimations::from_set(set, asset_server).unwrap(),
            window_drag: WindowDragAnimations::from_set(set, asset_server).unwrap(),
        }
    }
}

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

    fn first_image(&self) -> Handle<Image> {
        self.main.first_image()
    }
}

struct FileDndAnimations {
    hover_start: Animation,
    hover_loop: Animation,
    hover_cancel: Animation,
    drop: Animation,
}

impl FileDndAnimations {
    fn from_set(
        chomik_extract::AnimationSet { file_over, file_drop, .. }: &chomik_extract::AnimationSet,
        asset_server: &AssetServer,
    ) -> Result<Self, image::ImageError> {
        let chomik_extract::LoopingAnimation { name: _, entrance, looping, exit } = match file_over {
            chomik_extract::Animation::Looping(file_over) => file_over,
            _ => panic!("Received one-shot file over animation, expected looping animation"),
        };
        let chomik_extract::OneShotAnimation { name: _, sequence: file_drop } = match file_drop {
            chomik_extract::Animation::OneShot(file_drop) => file_drop,
            _ => panic!("Received looping file drop animation, expected one-shot animation"),
        };

        let hover_start = Animation::from_sequence(entrance, asset_server)?;
        let hover_loop = Animation::from_sequence(looping, asset_server)?;
        let hover_cancel = Animation::from_sequence(exit, asset_server)?;
        let drop = Animation::from_sequence(file_drop, asset_server)?;

        Ok(Self { hover_start, hover_loop, hover_cancel, drop })
    }
}

struct WindowDragAnimations {
    grab: Animation,
    looping: Animation,
    drop: Animation,
}

impl WindowDragAnimations {
    fn from_set(
        chomik_extract::AnimationSet { mouse_press, .. }: &chomik_extract::AnimationSet,
        asset_server: &AssetServer,
    ) -> Result<Self, image::ImageError> {
        let chomik_extract::LoopingAnimation { name: _, entrance, looping, exit } = match mouse_press {
            chomik_extract::Animation::Looping(file_over) => file_over,
            _ => panic!("Received one-shot mouse press animation, expected looping animation"),
        };

        let grab = Animation::from_sequence(entrance, asset_server)?;
        let looping = Animation::from_sequence(looping, asset_server)?;
        let drop = Animation::from_sequence(exit, asset_server)?;

        Ok(Self { grab, looping, drop })
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

    fn get(&self, idx: usize) -> Handle<Image> {
        self.frames[idx].clone()
    }

    fn first_image(&self) -> Handle<Image> {
        self.get(0)
    }
}
