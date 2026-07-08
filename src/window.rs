use bevy::DefaultPlugins;
use bevy::app::{App, PluginGroup, Update};
use bevy::camera::ClearColor;
use bevy::color::Color;
use bevy::ecs::system::{Query, Res};
use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::math::Vec2;
use bevy::window::{
    CompositeAlphaMode, EnabledButtons, ExitCondition, InternalWindowState, PresentMode, ScreenEdge, Window,
    WindowLevel, WindowMode, WindowPlugin, WindowPosition, WindowResizeConstraints, WindowResolution,
};
use bevy::winit::{UpdateMode, WinitSettings};

pub fn init(application: &mut App) {
    application.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(self::create_default_window()),
        exit_condition: ExitCondition::OnPrimaryClosed,
        ..WindowPlugin::default()
    }));
    application.insert_resource(WinitSettings {
        focused_mode: UpdateMode::Continuous,
        unfocused_mode: UpdateMode::Continuous,
    });
    application.insert_resource(ClearColor(Color::NONE));
    application.add_systems(Update, self::drag_window);
}

#[expect(clippy::cast_precision_loss, reason = "the window size should never be large enough")]
pub fn create_window_resolution_and_constraints(
    physical_width: u32,
    physical_height: u32,
    known_scale_factor: Option<f32>,
) -> (WindowResolution, WindowResizeConstraints) {
    let scale_factor = known_scale_factor.unwrap_or(1.0);

    (WindowResolution::new(physical_width, physical_height), WindowResizeConstraints {
        min_width: physical_width as f32 / scale_factor,
        max_width: physical_width as f32 / scale_factor,
        min_height: physical_height as f32 / scale_factor,
        max_height: physical_height as f32 / scale_factor,
    })
}

fn create_default_window() -> Window {
    let (resolution, resize_constraints) = self::create_window_resolution_and_constraints(256, 256, None);

    Window {
        present_mode: PresentMode::AutoNoVsync,
        mode: WindowMode::Windowed,
        position: WindowPosition::Automatic,
        resolution,
        title: concat!(env!("CARGO_BIN_NAME"), " v", env!("CARGO_PKG_VERSION")).to_string(),
        name: None,
        composite_alpha_mode: if cfg!(target_os = "macos") {
            CompositeAlphaMode::PostMultiplied
        } else {
            CompositeAlphaMode::PreMultiplied
        },
        resize_constraints,
        resizable: false,
        enabled_buttons: EnabledButtons { minimize: false, maximize: false, close: false },
        decorations: false,
        transparent: true,
        focused: true,
        window_level: WindowLevel::Normal,
        canvas: None,
        fit_canvas_to_parent: false,
        prevent_default_event_handling: false,
        internal: InternalWindowState::default(),
        ime_enabled: false,
        ime_position: Vec2::default(),
        window_theme: None,
        // Set to `true` after assets are loaded.
        visible: false,
        skip_taskbar: false,
        clip_children: true,
        desired_maximum_frame_latency: None,
        recognize_pinch_gesture: false,
        recognize_rotation_gesture: false,
        recognize_doubletap_gesture: false,
        recognize_pan_gesture: None,
        movable_by_window_background: false,
        fullsize_content_view: true,
        has_shadow: false,
        titlebar_shown: false,
        titlebar_transparent: true,
        titlebar_show_title: false,
        titlebar_show_buttons: false,
        prefers_home_indicator_hidden: true,
        prefers_status_bar_hidden: true,
        preferred_screen_edges_deferring_system_gestures: ScreenEdge::None,
        borderless_game: false,
    }
}

// Doesn't seem to work reliably, but it's better than nothing.
#[expect(clippy::needless_pass_by_value, reason = "`Res` is used for querying")]
fn drag_window(input: Res<ButtonInput<MouseButton>>, mut windows: Query<&mut Window>) {
    if input.just_pressed(MouseButton::Left) {
        for mut window in windows.iter_mut() {
            window.start_drag_move();
        }
    }
}
