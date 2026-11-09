#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod app;
mod game;
mod save;
mod text_input;

use gpui::{
    px, size, App, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions,
};
use gpui::prelude::*;

use crate::app::AppState;
use crate::text_input::{
    Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Quit, Right, SelectAll, SelectLeft,
    SelectRight, ShowCharacterPalette,
};

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", Left, None),
            KeyBinding::new("right", Right, None),
            KeyBinding::new("shift-left", SelectLeft, None),
            KeyBinding::new("shift-right", SelectRight, None),
            KeyBinding::new("ctrl-a", SelectAll, None),
            KeyBinding::new("ctrl-v", Paste, None),
            KeyBinding::new("ctrl-c", Copy, None),
            KeyBinding::new("ctrl-x", Cut, None),
            KeyBinding::new("home", Home, None),
            KeyBinding::new("end", End, None),
            KeyBinding::new("ctrl-shift-space", ShowCharacterPalette, None),
            KeyBinding::new("ctrl-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        let bounds = Bounds::centered(None, size(px(1180.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(960.), px(640.))),
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("P4G Save Editor v{}", env!("CARGO_PKG_VERSION")).into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| AppState::new(cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
