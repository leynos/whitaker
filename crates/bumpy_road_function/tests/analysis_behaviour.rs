//! Unit and behaviour-driven coverage for bumpy road interval detection.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

// When the lint crate is built with `dylint-driver` enabled (for example, under
// `cargo test --all-features`), this test crate must opt into `rustc_private`
// so the transitive `rustc_*` dependencies can link successfully.
#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use bumpy_road_function::analysis::{
    Settings, Weights, detect_bumps, normalise_settings, top_two_bumps,
};
use rstest::fixture;
use rstest::rstest;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[rstest]
fn normalise_settings_rejects_even_window() {
    let settings = Settings {
        window: 2,
        ..Settings::default()
    };

    let normalised = normalise_settings(settings);
    assert_eq!(normalised.window, Settings::default().window);
}

#[rstest]
fn normalise_settings_rejects_zero_window() {
    let settings = Settings {
        window: 0,
        ..Settings::default()
    };

    let normalised = normalise_settings(settings);
    assert_eq!(normalised.window, Settings::default().window);
}

#[rstest]
fn normalise_settings_rejects_negative_threshold() {
    let settings = Settings {
        threshold: -1.0,
        ..Settings::default()
    };

    let normalised = normalise_settings(settings);
    assert_eq!(normalised.threshold, Settings::default().threshold);
}

#[rstest]
fn normalise_settings_clamps_min_bump_lines() {
    let settings = Settings {
        min_bump_lines: 0,
        ..Settings::default()
    };

    let normalised = normalise_settings(settings);
    assert_eq!(normalised.min_bump_lines, 1);
}

#[rstest]
fn normalise_settings_rejects_negative_weights() {
    let settings = Settings {
        weights: Weights {
            depth: -1.0,
            predicate: 0.5,
            flow: -0.25,
        },
        ..Settings::default()
    };

    let normalised = normalise_settings(settings);
    assert_eq!(normalised.weights, Settings::default().weights);
}

#[rstest]
fn detect_bumps_reports_two_intervals() {
    let smoothed = vec![0.0, 3.0, 3.0, 0.0, 3.1, 3.0];
    let bumps = detect_bumps(&smoothed, 3.0, 2);

    assert_eq!(bumps.len(), 2);
    assert_eq!((bumps[0].start_index(), bumps[0].end_index()), (1, 2));
    assert_eq!((bumps[1].start_index(), bumps[1].end_index()), (4, 5));
}

#[rstest]
fn detect_bumps_ignores_short_spikes() {
    let smoothed = vec![0.0, 4.0, 0.0];
    let bumps = detect_bumps(&smoothed, 3.0, 2);

    assert!(bumps.is_empty());
}

#[rstest]
fn top_two_bumps_prefers_area_then_length() {
    let smoothed = vec![0.0, 4.0, 4.0, 0.0, 10.0, 0.0, 4.0, 4.0, 4.0];
    let bumps = detect_bumps(&smoothed, 3.0, 2);
    let top = top_two_bumps(bumps);

    assert_eq!(top.len(), 2);
    assert_eq!((top[0].start_index(), top[0].end_index()), (6, 8));
    assert_eq!((top[1].start_index(), top[1].end_index()), (1, 2));
}

#[derive(Default)]
struct World {
    signal: RefCell<Vec<f64>>,
    threshold: RefCell<f64>,
    min_bump_lines: RefCell<usize>,
    bumps: RefCell<Vec<bumpy_road_function::analysis::BumpInterval>>,
    settings: RefCell<Settings>,
    normalised: RefCell<Option<Settings>>,
}

#[fixture]
fn world() -> World {
    World::default()
}

#[given("a smoothed signal with two bumps")]
fn given_signal_two_bumps(world: &World) {
    world
        .signal
        .replace(vec![0.0, 3.0, 3.2, 0.0, 3.1, 3.0, 0.0]);
}

#[given("a smoothed signal with one bump")]
fn given_signal_one_bump(world: &World) {
    world.signal.replace(vec![0.0, 3.0, 3.0, 0.0]);
}

#[given("a smoothed signal with a short spike")]
fn given_signal_short_spike(world: &World) {
    world.signal.replace(vec![0.0, 4.0, 0.0]);
}

#[given("the threshold is {threshold:f64}")]
fn given_threshold(world: &World, threshold: f64) {
    world.threshold.replace(threshold);
}

#[given("the minimum bump length is {min_lines}")]
fn given_min_bump_lines(world: &World, min_lines: usize) {
    world.min_bump_lines.replace(min_lines);
}

#[when("I detect bumps")]
fn when_detect(world: &World) {
    let signal = world.signal.borrow();
    let threshold = *world.threshold.borrow();
    let min_lines = *world.min_bump_lines.borrow();
    let bumps = detect_bumps(&signal, threshold, min_lines);
    world.bumps.replace(bumps);
}

#[then("{count} bumps are reported")]
fn then_bump_count(world: &World, count: usize) {
    assert_eq!(world.bumps.borrow().len(), count);
}

#[given("default settings")]
fn given_default_settings(world: &World) {
    world.settings.replace(Settings::default());
}

#[when("the smoothing window is set to {window}")]
fn when_set_window(world: &World, window: usize) {
    world.settings.borrow_mut().window = window;
}

#[when("the threshold is set to {threshold:f64}")]
fn when_set_threshold(world: &World, threshold: f64) {
    world.settings.borrow_mut().threshold = threshold;
}

#[when("I normalise the settings")]
fn when_normalise(world: &World) {
    let settings = *world.settings.borrow();
    let normalised = normalise_settings(settings);
    world.normalised.replace(Some(normalised));
}

#[then("the window becomes {window}")]
fn then_window(world: &World, window: usize) {
    let settings = world
        .normalised
        .borrow()
        .expect("settings should be normalised");
    assert_eq!(settings.window, window);
}

#[then("the threshold becomes {threshold:f64}")]
fn then_threshold(world: &World, threshold: f64) {
    let settings = world
        .normalised
        .borrow()
        .expect("settings should be normalised");
    assert_eq!(settings.threshold, threshold);
}

#[scenario(path = "tests/features/bumpy_road.feature", index = 0)]
fn scenario_two_bumps(world: World) {
    let _ = world;
}

#[scenario(path = "tests/features/bumpy_road.feature", index = 1)]
fn scenario_one_bump(world: World) {
    let _ = world;
}

#[scenario(path = "tests/features/bumpy_road.feature", index = 2)]
fn scenario_short_spike(world: World) {
    let _ = world;
}

#[scenario(path = "tests/features/bumpy_road.feature", index = 3)]
fn scenario_even_window(world: World) {
    let _ = world;
}

#[scenario(path = "tests/features/bumpy_road.feature", index = 4)]
fn scenario_negative_threshold(world: World) {
    let _ = world;
}
