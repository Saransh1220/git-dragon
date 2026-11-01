use git_dragon::behavior::Behavior;
use git_dragon::motion::{FRAMES_PER_SECOND, FlightPose, calculate_flight_pose, frame_count};

const ALL_BEHAVIORS: [Behavior; 5] = [
    Behavior::Cruise,
    Behavior::Dart,
    Behavior::Rest,
    Behavior::SurfaceFeeding,
    Behavior::DeepRest,
];

fn collect_loop(behavior: Behavior) -> Vec<FlightPose> {
    (0..frame_count(behavior))
        .map(|frame_index| calculate_flight_pose(behavior, frame_index))
        .collect()
}

/// How many times the near wing turns from rising to falling across one loop.
fn count_beats(poses: &[FlightPose]) -> usize {
    let wing_beats: Vec<f64> = poses.iter().map(|pose| pose.wing_beat).collect();
    (0..wing_beats.len())
        .filter(|&frame_index| {
            let previous = wing_beats[(frame_index + wing_beats.len() - 1) % wing_beats.len()];
            let next = wing_beats[(frame_index + 1) % wing_beats.len()];
            wing_beats[frame_index] > previous && wing_beats[frame_index] >= next
        })
        .count()
}

fn range_of(poses: &[FlightPose], read: impl Fn(&FlightPose) -> f64) -> (f64, f64) {
    poses
        .iter()
        .map(read)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(value), high.max(value))
        })
}

#[test]
fn every_behavior_loops_at_its_own_length() {
    let lengths: Vec<_> = ALL_BEHAVIORS.map(frame_count).into();
    assert_eq!(lengths, [120, 120, 90, 90, 120]);
    assert_eq!(FRAMES_PER_SECOND, 15);
}

#[test]
fn every_loop_is_seamless_and_never_jumps() {
    for behavior in ALL_BEHAVIORS {
        let loop_frame_count = frame_count(behavior);
        assert_eq!(
            calculate_flight_pose(behavior, loop_frame_count),
            calculate_flight_pose(behavior, 0),
            "{behavior:?} does not close its loop"
        );
        let mut poses = collect_loop(behavior);
        poses.push(calculate_flight_pose(behavior, loop_frame_count));
        for pair in poses.windows(2) {
            let [before, after] = [&pair[0], &pair[1]];
            assert!((after.wing_beat - before.wing_beat).abs() < 0.5, "{behavior:?}");
            assert!(
                (after.far_wing_beat - before.far_wing_beat).abs() < 0.5,
                "{behavior:?}"
            );
            for axis_index in 0..2 {
                assert!(
                    (after.drift_pixels[axis_index] - before.drift_pixels[axis_index]).abs() < 3.0,
                    "{behavior:?}"
                );
            }
            assert!((after.pitch_radians - before.pitch_radians).abs() < 0.05, "{behavior:?}");
            assert!((after.mouth_opening - before.mouth_opening).abs() < 0.6, "{behavior:?}");
        }
    }
}

#[test]
fn frames_wrap_around_the_loop() {
    for behavior in ALL_BEHAVIORS {
        let loop_frame_count = frame_count(behavior);
        assert_eq!(
            calculate_flight_pose(behavior, 7),
            calculate_flight_pose(behavior, 7 + 3 * loop_frame_count)
        );
    }
}

#[test]
fn cruising_beats_its_wings_slowly_through_a_full_stroke() {
    let poses = collect_loop(Behavior::Cruise);
    assert_eq!(count_beats(&poses), 4);
    let (lowest, highest) = range_of(&poses, |pose| pose.wing_beat);
    assert!(lowest < -0.4 && highest > 0.6, "{lowest} {highest}");
}

#[test]
fn darting_beats_twice_as_fast_and_harder_than_cruising() {
    let cruise = collect_loop(Behavior::Cruise);
    let dart = collect_loop(Behavior::Dart);
    assert_eq!(count_beats(&dart), 2 * count_beats(&cruise));
    let (cruise_low, cruise_high) = range_of(&cruise, |pose| pose.wing_beat);
    let (dart_low, dart_high) = range_of(&dart, |pose| pose.wing_beat);
    assert!(dart_high - dart_low > cruise_high - cruise_low);
    let (surge_low, surge_high) = range_of(&dart, |pose| pose.drift_pixels[0]);
    let (cruise_surge_low, cruise_surge_high) = range_of(&cruise, |pose| pose.drift_pixels[0]);
    assert!(surge_high - surge_low > 2.0 * (cruise_surge_high - cruise_surge_low));
    assert!(dart.iter().all(|pose| pose.pitch_radians > 0.0), "nose down");
}

#[test]
fn the_body_rises_on_each_downstroke() {
    let poses = collect_loop(Behavior::Cruise);
    // Frame zero is the top of a beat; a quarter beat later the wings are mid downstroke.
    assert!(poses[0].drift_pixels[1].abs() < 1e-9);
    assert!(poses[7].drift_pixels[1] < -2.0, "up is negative");
    assert!(poses[22].drift_pixels[1] > 2.0, "sinks on the upstroke");
}

#[test]
fn the_far_wing_trails_the_near_wing() {
    for behavior in [Behavior::Cruise, Behavior::Dart, Behavior::Rest] {
        let poses = collect_loop(behavior);
        assert!(poses.iter().any(|pose| pose.far_wing_beat != pose.wing_beat));
    }
    // Every loop starts with the near wing at the top of its beat.
    for behavior in [Behavior::Cruise, Behavior::Rest] {
        let first_pose = calculate_flight_pose(behavior, 0);
        assert!(first_pose.far_wing_beat < first_pose.wing_beat, "{behavior:?}");
    }
}

#[test]
fn wing_tips_trail_their_roots_through_the_downstroke() {
    let poses = collect_loop(Behavior::Cruise);
    assert!(poses[7].wing_bend_radians > 0.1);
    assert!(poses[22].wing_bend_radians < -0.1);
}

#[test]
fn resting_hovers_lower_with_lazier_beats_than_cruising() {
    let rest = collect_loop(Behavior::Rest);
    let cruise = collect_loop(Behavior::Cruise);
    assert_eq!(count_beats(&rest), 2);
    let (rest_low, rest_high) = range_of(&rest, |pose| pose.wing_beat);
    let (cruise_low, cruise_high) = range_of(&cruise, |pose| pose.wing_beat);
    assert!(rest_high - rest_low < cruise_high - cruise_low);
    assert!(rest.iter().all(|pose| pose.depth == 0.22));
    assert!(rest.iter().all(|pose| pose.feeding_phase.is_none()));
}

#[test]
fn breathing_fire_cycles_the_flames_three_times_per_loop_while_the_mouth_gulps() {
    let poses = collect_loop(Behavior::SurfaceFeeding);
    let phases: Vec<f64> = poses
        .iter()
        .map(|pose| pose.feeding_phase.expect("flames in every frame"))
        .collect();
    assert!(phases.iter().all(|phase| (0.0..1.0).contains(phase)));
    let wraps = phases
        .windows(2)
        .filter(|pair| pair[1] < pair[0])
        .count();
    assert_eq!(wraps, 2, "three cycles wrap twice inside the loop");
    assert!(poses.iter().all(|pose| pose.depth == 0.0));
    let (closed, open) = range_of(&poses, |pose| pose.mouth_opening);
    assert_eq!(closed, 0.0);
    assert!((open - 1.0).abs() < 0.05);
}

#[test]
fn deep_rest_folds_the_wings_and_sleeps_with_one_yawn() {
    let poses = collect_loop(Behavior::DeepRest);
    assert!(poses.iter().all(|pose| pose.depth == 0.5));
    assert!(poses.iter().all(|pose| pose.wing_beat < -0.8));
    assert!(poses.iter().all(|pose| pose.far_wing_beat < -0.8));
    let yawning: Vec<_> = poses
        .iter()
        .enumerate()
        .filter(|(_, pose)| pose.mouth_opening > 0.0)
        .map(|(frame_index, _)| frame_index)
        .collect();
    assert!(!yawning.is_empty());
    assert!(
        yawning.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "one continuous yawn"
    );
    let (low, high) = range_of(&poses, |pose| pose.drift_pixels[1]);
    assert!(low > 2.0 && high - low < 4.0, "sinks and barely moves");
}

#[test]
fn only_fire_breathing_has_flames_and_only_deep_rest_sleeps() {
    for behavior in ALL_BEHAVIORS {
        for pose in collect_loop(behavior) {
            assert_eq!(
                pose.feeding_phase.is_some(),
                behavior == Behavior::SurfaceFeeding
            );
            assert_eq!(pose.depth > 0.4, behavior == Behavior::DeepRest);
        }
    }
}

#[test]
fn the_horns_sway_and_the_tail_swishes_in_every_behavior() {
    for behavior in ALL_BEHAVIORS {
        let poses = collect_loop(behavior);
        let (low, high) = range_of(&poses, |pose| pose.horn_sway_radians);
        assert!(high - low > 0.01, "{behavior:?}");
        let (low, high) = range_of(&poses, |pose| pose.swish_phase_radians.sin());
        assert!(high - low > 1.0, "{behavior:?}");
        assert!(poses.iter().all(|pose| pose.swish_strength > 0.0));
    }
}
