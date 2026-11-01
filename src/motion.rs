//! Flying over time: the pose for each frame of a behavior's seamless loop.
//!
//! The dragon is seen from the side and flies in place: its wings beat, its body bobs
//! up on each downstroke, and its tail swishes behind. Cruising beats slowly, darting
//! beats fast and surges forward, resting beats lazily, breathing fire hovers and
//! blasts flames, and deep rest folds the wings and sleeps.

use std::f64::consts::{PI, TAU};

use crate::behavior::Behavior;

/// Frames per second of every behavior's loop; GIF frame delays derive from it.
pub const FRAMES_PER_SECOND: u32 = 15;

/// Fire breathing and rest share this loop.
const REST_FRAME_COUNT: u32 = 90;
const REST_DEPTH: f64 = 0.22;
/// Flames repeat this many times per loop.
const FEEDING_CYCLES_PER_LOOP: f64 = 3.0;
/// Cruising and darting breathe this many times per loop, opening this far.
const BREATHS_PER_LOOP: f64 = 5.0;
const BREATH_OPENING: f64 = 0.7;
/// Resting and breathing fire open the mouth this many times per loop, fully.
const GULPS_PER_LOOP: f64 = 3.0;
const DEEP_REST_FRAME_COUNT: u32 = 120;
const DEEP_REST_DEPTH: f64 = 0.5;
/// How far a wing tip trails its root on the downstroke, per unit of wing amplitude.
const WING_TIP_LAG_RADIANS: f64 = 0.45;
/// How far the far wing trails the near one in the beat.
const FAR_WING_LAG_RADIANS: f64 = 0.45;
const HORN_SWAY_RADIANS: f64 = 0.08;
const HORN_SWAY_LAG_RADIANS: f64 = 0.6;

/// The numbers behind one kind of flying. Each loop holds whole numbers of beats,
/// bobs, surges, and tail swishes, so it closes seamlessly.
struct FlightStyle {
    loop_frame_count: u32,
    /// Frames per wing beat.
    beat_frame_count: f64,
    /// The wings' middle position, from -1 (down) to 1 (up).
    beat_center: f64,
    /// How far the wings swing each way around the middle.
    beat_amplitude: f64,
    /// How far the body rises on each downstroke, in native pixels.
    bob_pixels: f64,
    /// How far the body surges forward and back, and the frames per surge.
    surge_pixels: f64,
    surge_frame_count: f64,
    /// The body's resting pitch, nose up negative.
    pitch_radians: f64,
    pitch_swing_radians: f64,
    tail_swish_frame_count: f64,
    tail_swish_strength: f64,
    leg_swing_radians: f64,
}

/// A slow, steady beat: four beats in eight seconds.
const CRUISE_STYLE: FlightStyle = FlightStyle {
    loop_frame_count: 120,
    beat_frame_count: 30.0,
    beat_center: 0.1,
    beat_amplitude: 0.8,
    bob_pixels: 4.0,
    surge_pixels: 2.0,
    surge_frame_count: 120.0,
    pitch_radians: 0.0,
    pitch_swing_radians: 0.03,
    tail_swish_frame_count: 40.0,
    tail_swish_strength: 0.6,
    leg_swing_radians: 0.1,
};

/// Fast, hard beats with the nose down and a forward surge: eight beats in eight
/// seconds.
const DART_STYLE: FlightStyle = FlightStyle {
    loop_frame_count: 120,
    beat_frame_count: 15.0,
    beat_center: 0.15,
    beat_amplitude: 1.0,
    bob_pixels: 5.0,
    surge_pixels: 8.0,
    surge_frame_count: 60.0,
    pitch_radians: 0.12,
    pitch_swing_radians: 0.05,
    tail_swish_frame_count: 15.0,
    tail_swish_strength: 1.0,
    leg_swing_radians: 0.2,
};

/// Lazy beats while hovering at a lower height.
const REST_STYLE: FlightStyle = FlightStyle {
    loop_frame_count: REST_FRAME_COUNT,
    beat_frame_count: 45.0,
    beat_center: -0.15,
    beat_amplitude: 0.55,
    bob_pixels: 3.0,
    surge_pixels: 0.0,
    surge_frame_count: 90.0,
    pitch_radians: 0.0,
    pitch_swing_radians: 0.02,
    tail_swish_frame_count: 45.0,
    tail_swish_strength: 0.4,
    leg_swing_radians: 0.05,
};

/// How the dragon looks in one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightPose {
    /// How far the dragon is moved from the hover center, in native pixels.
    pub drift_pixels: [f64; 2],
    /// How far the body is pitched; positive tips the nose down.
    pub pitch_radians: f64,
    /// How deeply the dragon sleeps, from zero awake to one.
    pub depth: f64,
    /// Where the tail's swish is in its cycle.
    pub swish_phase_radians: f64,
    /// Multiplies the tail's swish amplitude.
    pub swish_strength: f64,
    /// Where the near wing is in its beat, from -1 (fully down) to 1 (fully up).
    pub wing_beat: f64,
    /// Where the far wing is in its beat; it trails the near wing.
    pub far_wing_beat: f64,
    /// Bend along each wing, so its tip trails its root.
    pub wing_bend_radians: f64,
    /// How far the legs swing back from their tucked pose.
    pub leg_swing_radians: f64,
    /// The horns swing together, trailing the head.
    pub horn_sway_radians: f64,
    /// Where fire breathing's flames are in their cycle, from zero up to one; none in
    /// every other behavior.
    pub feeding_phase: Option<f64>,
    /// How far the mouth is open, from zero closed to one wide open.
    pub mouth_opening: f64,
}

/// The still image: hovering with the wings raised and a gentle tail curl.
pub(crate) const STILL_POSE: FlightPose = FlightPose {
    drift_pixels: [0.0; 2],
    pitch_radians: 0.0,
    depth: 0.0,
    swish_phase_radians: 0.8,
    swish_strength: 1.0,
    wing_beat: 0.85,
    far_wing_beat: 0.6,
    wing_bend_radians: 0.0,
    leg_swing_radians: 0.0,
    horn_sway_radians: 0.0,
    feeding_phase: None,
    mouth_opening: 0.0,
};

/// Frames in a behavior's loop at FRAMES_PER_SECOND.
pub fn frame_count(behavior: Behavior) -> u32 {
    match behavior {
        Behavior::Cruise => CRUISE_STYLE.loop_frame_count,
        Behavior::Dart => DART_STYLE.loop_frame_count,
        Behavior::Rest | Behavior::SurfaceFeeding => REST_FRAME_COUNT,
        Behavior::DeepRest => DEEP_REST_FRAME_COUNT,
    }
}

/// The pose for a frame of a behavior's loop; frame numbers wrap around the loop, so
/// frame `frame_count(behavior)` is frame zero again.
///
/// Cruising beats slowly and steadily; darting beats fast with the nose down and
/// surges forward on each pair of beats. Resting beats lazily at a lower height; fire
/// breathing hovers the same way and blasts flames; deep rest folds the wings and
/// sleeps, breathing slowly, with one yawn per loop. In every behavior the body bobs
/// up on each downstroke, the wing tips trail their roots, the far wing trails the
/// near one, the tail swishes behind, and the horns sway. The mouth pants while the
/// dragon cruises or darts, gulps while it rests or breathes fire, and takes one slow
/// yawn in deep rest.
pub fn calculate_flight_pose(behavior: Behavior, frame_index: u32) -> FlightPose {
    let loop_frame = frame_index % frame_count(behavior);
    let drive = calculate_drive(behavior, loop_frame);
    FlightPose {
        horn_sway_radians: HORN_SWAY_RADIANS
            * (drive.pose.swish_phase_radians - HORN_SWAY_LAG_RADIANS).sin(),
        ..drive.pose
    }
}

struct Drive {
    pose: FlightPose,
}

fn calculate_drive(behavior: Behavior, loop_frame: u32) -> Drive {
    let pose = match behavior {
        Behavior::Cruise => CRUISE_STYLE.calculate_pose(loop_frame, 0.0, None, breath(CRUISE_STYLE.loop_frame_count, loop_frame)),
        Behavior::Dart => DART_STYLE.calculate_pose(loop_frame, 0.0, None, breath(DART_STYLE.loop_frame_count, loop_frame)),
        Behavior::Rest => REST_STYLE.calculate_pose(loop_frame, REST_DEPTH, None, gulp(loop_frame)),
        Behavior::SurfaceFeeding => {
            let feeding_phase = (FEEDING_CYCLES_PER_LOOP * f64::from(loop_frame)
                / f64::from(REST_FRAME_COUNT))
            .fract();
            REST_STYLE.calculate_pose(loop_frame, 0.0, Some(feeding_phase), gulp(loop_frame))
        }
        Behavior::DeepRest => calculate_deep_rest_pose(loop_frame),
    };
    Drive { pose }
}

/// A pant: the mouth opens a little, five times per loop.
fn breath(loop_frame_count: u32, loop_frame: u32) -> f64 {
    let breath = (TAU * BREATHS_PER_LOOP * f64::from(loop_frame) / f64::from(loop_frame_count))
        .sin()
        .max(0.0);
    BREATH_OPENING * breath * breath
}

/// A gulp: the mouth opens fully, three times per loop.
fn gulp(loop_frame: u32) -> f64 {
    (GULPS_PER_LOOP * TAU * f64::from(loop_frame) / f64::from(REST_FRAME_COUNT))
        .sin()
        .max(0.0)
}

impl FlightStyle {
    fn calculate_pose(
        &self,
        loop_frame: u32,
        depth: f64,
        feeding_phase: Option<f64>,
        mouth_opening: f64,
    ) -> FlightPose {
        let frame = f64::from(loop_frame);
        let beat_turn = TAU * frame / self.beat_frame_count;
        let surge_turn = TAU * frame / self.surge_frame_count;
        let swish_turn = TAU * frame / self.tail_swish_frame_count;
        let wing_beat = |lag_radians: f64| {
            (self.beat_center + self.beat_amplitude * (beat_turn - lag_radians).cos())
                .clamp(-1.0, 1.0)
        };
        FlightPose {
            drift_pixels: [
                self.surge_pixels * surge_turn.sin(),
                -self.bob_pixels * beat_turn.sin(),
            ],
            pitch_radians: self.pitch_radians + self.pitch_swing_radians * beat_turn.sin(),
            depth,
            swish_phase_radians: STILL_POSE.swish_phase_radians + swish_turn,
            swish_strength: self.tail_swish_strength,
            wing_beat: wing_beat(0.0),
            far_wing_beat: wing_beat(FAR_WING_LAG_RADIANS),
            wing_bend_radians: WING_TIP_LAG_RADIANS * self.beat_amplitude * beat_turn.sin(),
            leg_swing_radians: self.leg_swing_radians * (beat_turn - 1.0).sin(),
            horn_sway_radians: 0.0,
            feeding_phase,
            mouth_opening,
        }
    }
}

/// Sleeps with the wings folded: a slow breath lifts the body and tail, and one slow
/// yawn per loop opens the mouth.
fn calculate_deep_rest_pose(loop_frame: u32) -> FlightPose {
    const SINK_PIXELS: f64 = 5.0;
    const BREATH_PIXELS: f64 = 1.5;
    const SWISH_FRAME_COUNT: f64 = 60.0;
    const SWISH_STRENGTH: f64 = 0.12;
    const FOLDED_BEAT: f64 = -0.95;
    const BREATH_BEAT: f64 = 0.05;
    // One slow yawn.
    const YAWN_START_FRAME: f64 = 66.0;
    const YAWN_FRAME_COUNT: f64 = 16.0;
    let frame = f64::from(loop_frame);
    let loop_turn = TAU * frame / f64::from(DEEP_REST_FRAME_COUNT);
    let yawn_fraction = (frame - YAWN_START_FRAME) / YAWN_FRAME_COUNT;
    let mouth_opening = if (0.0..=1.0).contains(&yawn_fraction) {
        (PI * yawn_fraction).sin()
    } else {
        0.0
    };
    FlightPose {
        drift_pixels: [0.0, SINK_PIXELS - BREATH_PIXELS * (2.0 * loop_turn).sin()],
        pitch_radians: 0.1,
        depth: DEEP_REST_DEPTH,
        swish_phase_radians: STILL_POSE.swish_phase_radians + TAU * frame / SWISH_FRAME_COUNT,
        swish_strength: SWISH_STRENGTH,
        wing_beat: FOLDED_BEAT + BREATH_BEAT * (2.0 * loop_turn).sin(),
        far_wing_beat: FOLDED_BEAT + BREATH_BEAT * (2.0 * loop_turn - 0.4).sin(),
        wing_bend_radians: 0.0,
        leg_swing_radians: 0.0,
        horn_sway_radians: 0.0,
        feeding_phase: None,
        mouth_opening,
    }
}
