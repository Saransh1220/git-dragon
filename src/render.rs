//! A baby dragon seen from the side, colored from its languages on a chosen body, with
//! Classic shading and transparent surroundings.
//!
//! The body follows a sampled S-curve: a wedge-shaped head, an arched neck, a deep
//! chest, and a long tapering tail that ends in a spade. A pale belly with plates runs
//! underneath, a row of spines runs along the back, and two horns sweep back from the
//! head. Bat wings spring from the shoulders; a near wing beats over the body and a
//! darker far wing beats behind it. Short legs tuck under the chest and hips. Contribution
//! totals set the size through [`crate::dragon::calculate_growth_fraction`], and growth
//! slows as the dragon matures. Young dragons have a rounder head, larger eyes, a
//! chubbier body, and small wings, and grow into the adult shape by
//! [`crate::dragon::YOUNG_SHAPE_CONTRIBUTIONS`]. After [`crate::dragon::ADULT_CONTRIBUTIONS`]
//! the body keeps lengthening, and its wings and tail lengthen; the maximum form keeps
//! its anatomy at higher totals. Quiet days alone do not change the base anatomy. The
//! pose and framing account for the wings and tail.
//!
//! Every growth level shares one square native canvas of [`FRAME_WIDTH_PIXELS`] by
//! [`FRAME_HEIGHT_PIXELS`], which the encoders in [`crate::output`] scale up exactly.
//! Anatomical proportions live here; pigments, bodies, and palettes in [`crate::coat`];
//! streak mark strengths in [`crate::markings`]; poses in [`crate::motion`].

use std::f64::consts::{PI, TAU};

use crate::behavior::Behavior;
use crate::coat::{DragonBody, DragonCoat};
use crate::dragon::{
    ADULT_CONTRIBUTIONS, DragonDevelopment, YOUNG_SHAPE_CONTRIBUTIONS, calculate_growth_fraction,
};
use crate::motion::{FlightPose, STILL_POSE, calculate_flight_pose, frame_count};

/// Width of the native canvas every dragon is drawn on, at every growth level.
pub const FRAME_WIDTH_PIXELS: u32 = 256;
/// Height of the native canvas; the canvas is square.
pub const FRAME_HEIGHT_PIXELS: u32 = FRAME_WIDTH_PIXELS;

/// A renderer-owned, fixed-size frame with row-major RGBA pixels.
#[derive(Debug, Eq, PartialEq)]
pub struct DragonFrame {
    rgba_pixels: Vec<[u8; 4]>,
}

impl DragonFrame {
    /// The pixels as bytes, row by row from the top left, four channels per pixel: red,
    /// green, blue, alpha. The length is the canvas area times four.
    pub fn as_rgba_bytes(&self) -> &[u8] {
        self.rgba_pixels.as_flattened()
    }

    /// Build a frame from chosen pixels, for encoder tests that need colors no dragon has.
    #[cfg(test)]
    pub(crate) fn from_rgba_pixels(rgba_pixels: Vec<[u8; 4]>) -> Self {
        Self { rgba_pixels }
    }
}

/// Render the dragon in a centered, fixed hovering pose with its wings raised, colored
/// from its languages on the caller's chosen body. Contribution totals set its size and
/// how far its colors have come in. The account seed places patches in body coordinates,
/// so growth does not reroll the pattern. Streak marks follow the development's streaks
/// as of its observation date. Identical inputs produce identical pixels without I/O,
/// clock reads, or input mutation.
pub fn render_dragon(dragon_development: &DragonDevelopment, dragon_body: DragonBody) -> DragonFrame {
    let dragon_anatomy = calculate_anatomy(dragon_development.total_contributions);
    render_pose(
        dragon_development,
        dragon_body,
        &dragon_anatomy,
        &STILL_POSE,
        Framing::CenterOnDragon,
    )
}

/// Render one frame of a behavior's seamless loop, colored like `render_dragon`, in the
/// pose `motion::calculate_flight_pose` gives; frame numbers wrap around the loop of
/// `motion::frame_count(behavior)` frames. The dragon flies in place: a loop is framed
/// once, centered on everything the dragon covers across its frames, and its drift
/// shrinks only as far as the largest dragons need to stay inside. The view never
/// follows the wings, so it never shakes. Asleep the dragon turns toward moonlit blue
/// and partly drops out, more with depth. Identical inputs produce identical pixels
/// without I/O, clock reads, or input mutation.
pub fn render_flight_frame(
    dragon_development: &DragonDevelopment,
    dragon_body: DragonBody,
    behavior: Behavior,
    frame_index: u32,
) -> DragonFrame {
    let dragon_anatomy = calculate_anatomy(dragon_development.total_contributions);
    let flight_pose = calculate_flight_pose(behavior, frame_index);
    let hover_fit = fit_hover_loop(behavior, &dragon_anatomy);
    let pose_center = [0, 1].map(|axis_index| {
        hover_fit.sweep_center[axis_index]
            - hover_fit.drift_scales[axis_index] * flight_pose.drift_pixels[axis_index]
    });
    render_pose(
        dragon_development,
        dragon_body,
        &dragon_anatomy,
        &flight_pose,
        Framing::At(pose_center),
    )
}

/// Where the view is centered: on the dragon itself for the still image, or at a point
/// fixed for the whole loop.
enum Framing {
    CenterOnDragon,
    At([f64; 2]),
}

/// One framing for a whole hovering loop.
struct HoverFit {
    sweep_center: [f64; 2],
    /// Shares of the pose's drift that keep every frame inside, from zero through one.
    drift_scales: [f64; 2],
}

/// Center a hovering loop on everything the dragon covers across its frames, then shrink
/// the drift on each axis only as far as needed to keep every frame inside the frame.
fn fit_hover_loop(behavior: Behavior, dragon_anatomy: &DragonAnatomy) -> HoverFit {
    // Room for the outline, a clear pixel at the frame edge, and rounding.
    const FIT_MARGIN_PIXELS: f64 = 2.5;
    // Share of the body length reserved ahead of the mouth for breathing fire.
    const PLUME_PER_BODY_LENGTH: f64 = 0.5;
    let frame_bounds: Vec<([[f64; 2]; 2], [f64; 2])> = (0..frame_count(behavior))
        .map(|frame_index| {
            let flight_pose = calculate_flight_pose(behavior, frame_index);
            let body_spine = build_spine(dragon_anatomy, &flight_pose);
            let fin_curves = build_fins(&body_spine, dragon_anatomy, &flight_pose);
            let [mut min_corner, mut max_corner] = measure_curve_bounds(&body_spine, &fin_curves);
            // A horn point covers the pixel to its right and below.
            for horn_point in
                calculate_horn_points(&body_spine, dragon_anatomy, flight_pose.horn_sway_radians)
            {
                for axis_index in 0..2 {
                    min_corner[axis_index] = min_corner[axis_index].min(horn_point[axis_index]);
                    max_corner[axis_index] =
                        max_corner[axis_index].max(horn_point[axis_index] + 1.0);
                }
            }
            let drift_pixels = flight_pose.drift_pixels;
            ([min_corner, max_corner], drift_pixels)
        })
        .collect();
    let mut frame_bounds = frame_bounds;
    if calculate_flight_pose(behavior, 0).feeding_phase.is_some() {
        // Leave room ahead of the mouth for the plume, as far as the frame allows.
        let sweep_width = frame_bounds
            .iter()
            .map(|(bounds, _)| bounds[1][0])
            .fold(f64::NEG_INFINITY, f64::max)
            - frame_bounds
                .iter()
                .map(|(bounds, _)| bounds[0][0])
                .fold(f64::INFINITY, f64::min);
        let plume_room = (f64::from(FRAME_WIDTH_PIXELS) - 2.0 * FIT_MARGIN_PIXELS - sweep_width)
            .clamp(0.0, dragon_anatomy.body_length_pixels * PLUME_PER_BODY_LENGTH);
        for (bounds, _) in &mut frame_bounds {
            bounds[1][0] += plume_room;
        }
    }
    let frame_sizes = [
        f64::from(FRAME_WIDTH_PIXELS),
        f64::from(FRAME_HEIGHT_PIXELS),
    ];
    let mut hover_fit = HoverFit {
        sweep_center: [0.0; 2],
        drift_scales: [1.0; 2],
    };
    for axis_index in 0..2 {
        let sweep_min = frame_bounds
            .iter()
            .map(|(bounds, _)| bounds[0][axis_index])
            .fold(f64::INFINITY, f64::min);
        let sweep_max = frame_bounds
            .iter()
            .map(|(bounds, _)| bounds[1][axis_index])
            .fold(f64::NEG_INFINITY, f64::max);
        let sweep_center = (sweep_min + sweep_max) / 2.0;
        let frame_half = frame_sizes[axis_index] / 2.0;
        let mut drift_scale: f64 = 1.0;
        for (bounds, drift_pixels) in &frame_bounds {
            let drift = drift_pixels[axis_index];
            let room = if drift > 0.0 {
                frame_half - FIT_MARGIN_PIXELS - (bounds[1][axis_index] - sweep_center)
            } else {
                frame_half - FIT_MARGIN_PIXELS + (bounds[0][axis_index] - sweep_center)
            };
            if drift != 0.0 {
                drift_scale = drift_scale.min(room / drift.abs());
            }
        }
        hover_fit.sweep_center[axis_index] = sweep_center;
        hover_fit.drift_scales[axis_index] = drift_scale.clamp(0.0, 1.0);
    }
    hover_fit
}

fn render_pose(
    dragon_development: &DragonDevelopment,
    dragon_body: DragonBody,
    dragon_anatomy: &DragonAnatomy,
    flight_pose: &FlightPose,
    framing: Framing,
) -> DragonFrame {
    let dragon_coat = DragonCoat::new(dragon_development, dragon_body, |along_fraction| {
        calculate_body_half_width(along_fraction, dragon_anatomy)
    });
    let body_spine = build_spine(dragon_anatomy, flight_pose);
    let fin_curves = build_fins(&body_spine, dragon_anatomy, flight_pose);
    let [min_corner, max_corner] = measure_curve_bounds(&body_spine, &fin_curves);
    let pose_center = match framing {
        Framing::CenterOnDragon => [
            (min_corner[0] + max_corner[0]) / 2.0,
            (min_corner[1] + max_corner[1]) / 2.0,
        ],
        Framing::At(pose_center) => pose_center,
    };
    let dragon_appearance = DragonAppearance {
        dragon_anatomy,
        dragon_coat: &dragon_coat,
    };
    let mut rgba_pixels = Vec::with_capacity((FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize);
    for pixel_y in 0..FRAME_HEIGHT_PIXELS {
        for pixel_x in 0..FRAME_WIDTH_PIXELS {
            let sample_position = [
                f64::from(pixel_x) + 0.5 - f64::from(FRAME_WIDTH_PIXELS) / 2.0 + pose_center[0],
                f64::from(pixel_y) + 0.5 - f64::from(FRAME_HEIGHT_PIXELS) / 2.0 + pose_center[1],
            ];
            // Nothing outside the curves' bounds can be covered, so skip sampling it.
            let is_inside_bounds = (0..2).all(|axis_index| {
                (min_corner[axis_index]..=max_corner[axis_index])
                    .contains(&sample_position[axis_index])
            });
            rgba_pixels.push(if is_inside_bounds {
                sample_dragon(
                    &body_spine,
                    &fin_curves,
                    sample_position,
                    [pixel_x, pixel_y],
                    &dragon_appearance,
                )
            } else {
                [0; 4]
            });
        }
    }
    paint_mouth(
        &mut rgba_pixels,
        &body_spine,
        dragon_body,
        pose_center,
        flight_pose.mouth_opening,
    );
    paint_spines(
        &mut rgba_pixels,
        &body_spine,
        dragon_anatomy,
        pose_center,
        dragon_coat.select_spine_color(),
    );
    let dragon_pixel_mask: Vec<bool> = rgba_pixels
        .iter()
        .map(|pixel_channels| pixel_channels[3] != 0)
        .collect();
    let horn_points =
        calculate_horn_points(&body_spine, dragon_anatomy, flight_pose.horn_sway_radians);
    paint_horns(&mut rgba_pixels, &horn_points, pose_center);
    paint_outline(&mut rgba_pixels);
    if let Some(feeding_phase) = flight_pose.feeding_phase {
        paint_feeding(
            &mut rgba_pixels,
            &body_spine,
            dragon_anatomy,
            pose_center,
            feeding_phase,
        );
    }
    if flight_pose.depth > 0.0 {
        paint_depth(&mut rgba_pixels, &dragon_pixel_mask, flight_pose.depth);
    }
    DragonFrame { rgba_pixels }
}

/// Where the mouth opens: on the snout's center line, just inside its front.
fn calculate_mouth_position(body_spine: &[CurvePoint]) -> [f64; 2] {
    const MOUTH_FORWARD_FRACTION: f64 = 0.75;
    let snout = body_spine[0];
    let forward_direction = calculate_forward_direction(body_spine, 0);
    [0, 1].map(|axis_index| {
        snout.position[axis_index]
            + forward_direction[axis_index] * snout.half_width_pixels * MOUTH_FORWARD_FRACTION
    })
}

/// The mouth: a circle at the front of the snout with a rim in the body's color around
/// a dark opening, growing from under a pixel to half the snout's width as it opens.
/// It is painted before the outline, so it is outlined like the dragon, and a mostly
/// closed mouth paints nothing.
fn paint_mouth(
    rgba_pixels: &mut [[u8; 4]],
    body_spine: &[CurvePoint],
    dragon_body: DragonBody,
    pose_center: [f64; 2],
    mouth_opening: f64,
) {
    const VISIBLE_OPENING: f64 = 0.1;
    const CLOSED_RADIUS_PIXELS: f64 = 0.8;
    const OPEN_RADIUS_PER_SNOUT_RADIUS: f64 = 0.5;
    const MIN_OPEN_RADIUS_PIXELS: f64 = 1.8;
    const RIM_WIDTH_PIXELS: f64 = 0.9;
    if mouth_opening <= VISIBLE_OPENING {
        return;
    }
    let open_radius_pixels = (body_spine[0].half_width_pixels * OPEN_RADIUS_PER_SNOUT_RADIUS)
        .max(MIN_OPEN_RADIUS_PIXELS);
    let radius_pixels = interpolate(
        CLOSED_RADIUS_PIXELS,
        open_radius_pixels,
        mouth_opening.min(1.0),
    );
    let mouth_world = calculate_mouth_position(body_spine);
    let frame_sizes = [
        f64::from(FRAME_WIDTH_PIXELS),
        f64::from(FRAME_HEIGHT_PIXELS),
    ];
    let mouth = [0, 1].map(|axis_index| {
        mouth_world[axis_index] - pose_center[axis_index] + frame_sizes[axis_index] / 2.0
    });
    let rim_color = dragon_body.select_mouth_rim_color();
    let frame_width = FRAME_WIDTH_PIXELS as usize;
    let first_y = (mouth[1] - radius_pixels).floor().max(0.0) as usize;
    let last_y = ((mouth[1] + radius_pixels).ceil() as usize).min(FRAME_HEIGHT_PIXELS as usize - 1);
    let first_x = (mouth[0] - radius_pixels).floor().max(0.0) as usize;
    let last_x = ((mouth[0] + radius_pixels).ceil() as usize).min(frame_width - 1);
    for pixel_y in first_y..=last_y {
        for pixel_x in first_x..=last_x {
            let distance_pixels =
                (pixel_x as f64 + 0.5 - mouth[0]).hypot(pixel_y as f64 + 0.5 - mouth[1]);
            if distance_pixels <= radius_pixels {
                rgba_pixels[pixel_y * frame_width + pixel_x] =
                    if distance_pixels > radius_pixels - RIM_WIDTH_PIXELS {
                        rim_color
                    } else {
                        OUTLINE_COLOR
                    };
            }
        }
    }
}

/// Surface feeding: a plume of fire from the mouth, hot white and yellow near the jaws,
/// cooling to orange and red embers, drawn in the air around the dragon. Where the frame
/// edge leaves too little room, the plume shortens instead of running off the edge.
fn paint_feeding(
    rgba_pixels: &mut [[u8; 4]],
    body_spine: &[CurvePoint],
    dragon_anatomy: &DragonAnatomy,
    pose_center: [f64; 2],
    feeding_phase: f64,
) {
    const PARTICLE_COUNT: usize = 72;
    const REACH_PER_BODY_LENGTH: f64 = 0.6;
    const SPREAD_PER_REACH: f64 = 0.24;
    const FLAME_COLORS: [[u8; 4]; 5] = [
        [255, 250, 214, 255],
        [254, 231, 97, 255],
        [254, 174, 52, 255],
        [247, 118, 34, 255],
        [190, 74, 47, 255],
    ];
    // A clear pixel between the flames and the frame edge, plus rounding.
    const EDGE_MARGIN_PIXELS: f64 = 3.0;
    const MIN_REACH_PIXELS: f64 = 8.0;
    let frame_sizes = [
        f64::from(FRAME_WIDTH_PIXELS),
        f64::from(FRAME_HEIGHT_PIXELS),
    ];
    let mouth_world = calculate_mouth_position(body_spine);
    let mouth = [0, 1].map(|axis_index| {
        mouth_world[axis_index] - pose_center[axis_index] + frame_sizes[axis_index] / 2.0
    });
    let forward_direction = calculate_forward_direction(body_spine, 0);
    let side_direction = [-forward_direction[1], forward_direction[0]];
    let room_ahead = (0..2)
        .filter(|&axis_index| forward_direction[axis_index].abs() > 1e-6)
        .map(|axis_index| {
            let room = if forward_direction[axis_index] > 0.0 {
                frame_sizes[axis_index] - 1.0 - EDGE_MARGIN_PIXELS - mouth[axis_index]
            } else {
                mouth[axis_index] - EDGE_MARGIN_PIXELS
            };
            room.max(0.0) / forward_direction[axis_index].abs()
        })
        .fold(f64::INFINITY, f64::min);
    let reach_pixels = (dragon_anatomy.body_length_pixels * REACH_PER_BODY_LENGTH)
        .min(room_ahead - 4.0)
        .max(MIN_REACH_PIXELS.min(room_ahead));
    let frame_width = FRAME_WIDTH_PIXELS as usize;
    let mut paint_air = |point: [f64; 2], color: [u8; 4]| {
        let [pixel_x, pixel_y] = point.map(f64::floor);
        if !(0.0..frame_sizes[0]).contains(&pixel_x) || !(0.0..frame_sizes[1]).contains(&pixel_y) {
            return;
        }
        let pixel_index = pixel_y as usize * frame_width + pixel_x as usize;
        if rgba_pixels[pixel_index][3] == 0 {
            rgba_pixels[pixel_index] = color;
        }
    };
    for particle_index in 0..PARTICLE_COUNT {
        let life = (feeding_phase + particle_index as f64 / PARTICLE_COUNT as f64).fract();
        let distance_pixels = 3.0 + life * reach_pixels;
        let spread_pixels = life.powf(0.85)
            * reach_pixels
            * SPREAD_PER_REACH
            * (particle_index as f64 * 2.399).sin();
        let particle = [0, 1].map(|axis_index| {
            mouth[axis_index]
                + forward_direction[axis_index] * distance_pixels
                + side_direction[axis_index] * spread_pixels
        });
        let [pixel_x, pixel_y] = particle.map(f64::floor);
        if calculate_dither_threshold([pixel_x.max(0.0) as u32, pixel_y.max(0.0) as u32])
            > 1.15 - life
        {
            continue;
        }
        let color = FLAME_COLORS[((life.powf(0.85) * FLAME_COLORS.len() as f64) as usize)
            .min(FLAME_COLORS.len() - 1)];
        paint_air(particle, color);
        // Flames are fat near the jaws and thin to single sparks.
        if life < 0.6 {
            for offset in [[1.0, 0.0], [0.0, 1.0], [1.0, 1.0]] {
                paint_air([particle[0] + offset[0], particle[1] + offset[1]], color);
            }
        }
    }
}

/// Asleep, the dragon turns moonlit blue: each body and fin pixel maps its own
/// brightness onto a water-blue scale, the horns and outline soften to deep blue,
/// and a dithered share of each drops out, more for the outline.
fn paint_depth(rgba_pixels: &mut [[u8; 4]], dragon_pixel_mask: &[bool], depth: f64) {
    const DEEP_WATER: [f64; 3] = [18.0, 78.0, 137.0];
    const MID_WATER: [f64; 3] = [0.0, 153.0, 219.0];
    const SHALLOW_WATER: [f64; 3] = [190.0, 225.0, 245.0];
    const OUTLINE_WATER: [f64; 3] = [24.0, 48.0, 92.0];
    const FISH_DROPOUT_PER_DEPTH: f64 = 0.15;
    const OUTLINE_DROPOUT_PER_DEPTH: f64 = 0.45;
    const TINT_PER_DEPTH: f64 = 1.15;
    let tint = (depth * TINT_PER_DEPTH).min(1.0);
    let frame_width = FRAME_WIDTH_PIXELS as usize;
    for (pixel_index, pixel_channels) in rgba_pixels.iter_mut().enumerate() {
        if pixel_channels[3] == 0 {
            continue;
        }
        let is_dragon_pixel = dragon_pixel_mask[pixel_index];
        let dropout_per_depth = if is_dragon_pixel {
            FISH_DROPOUT_PER_DEPTH
        } else {
            OUTLINE_DROPOUT_PER_DEPTH
        };
        let pixel_position = [
            (pixel_index % frame_width) as u32,
            (pixel_index / frame_width) as u32,
        ];
        if calculate_dither_threshold(pixel_position) < depth * dropout_per_depth {
            *pixel_channels = [0; 4];
            continue;
        }
        let original_color =
            [0, 1, 2].map(|channel_index| f64::from(pixel_channels[channel_index]));
        let water_color = if is_dragon_pixel {
            let brightness =
                (0.3 * original_color[0] + 0.59 * original_color[1] + 0.11 * original_color[2])
                    / 255.0;
            let (from_color, to_color, blend) = if brightness < 0.5 {
                (DEEP_WATER, MID_WATER, brightness * 2.0)
            } else {
                (MID_WATER, SHALLOW_WATER, brightness * 2.0 - 1.0)
            };
            [0, 1, 2].map(|channel_index| {
                interpolate(from_color[channel_index], to_color[channel_index], blend)
            })
        } else {
            OUTLINE_WATER
        };
        for channel_index in 0..3 {
            pixel_channels[channel_index] = interpolate(
                original_color[channel_index],
                water_color[channel_index],
                tint,
            )
            .round() as u8;
        }
    }
}

const OUTLINE_COLOR: [u8; 4] = [24, 20, 37, 255];
const HORN_COLOR: [u8; 4] = [234, 212, 170, 255];
const DITHER_RANKS: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

struct DragonAnatomy {
    body_length_pixels: f64,
    body_width_pixels: f64,
    /// Share of the body length that the head covers, from the nose to the jaw.
    head_slant_fraction: f64,
    /// Nose width as a share of the full body width.
    nose_width_fraction: f64,
    /// Width of the head at the jaw as a share of the full body width.
    head_width_fraction: f64,
    /// Width of the neck as a share of the full body width.
    neck_width_fraction: f64,
    /// Exponent of the taper from the shoulders to the tail stalk.
    tail_taper_exponent: f64,
    /// Fin shapes blend from young (0) to adult (1).
    adult_fin_fraction: f64,
    fin_length_multiplier: f64,
    eye_along_fraction: f64,
    /// Young dragons have proportionally larger eyes; adults use 1.
    eye_scale: f64,
}

struct DragonAppearance<'a> {
    dragon_anatomy: &'a DragonAnatomy,
    dragon_coat: &'a DragonCoat,
}

// Positions and half widths are pixels; consecutive points form tapered strokes.
#[derive(Clone, Copy)]
struct CurvePoint {
    position: [f64; 2],
    half_width_pixels: f64,
}

struct CurveSample {
    along_fraction: f64,
    lateral_pixels: f64,
    distance_pixels: f64,
    half_width_pixels: f64,
    normal_direction: [f64; 2],
}

#[derive(Clone, Copy)]
struct FinShape {
    length_body_widths: f64,
    half_width_body_widths: f64,
    width_start_fraction: f64,
    taper_exponent: f64,
}

impl FinShape {
    fn blend(self, other: Self, other_fraction: f64) -> Self {
        Self {
            length_body_widths: interpolate(
                self.length_body_widths,
                other.length_body_widths,
                other_fraction,
            ),
            half_width_body_widths: interpolate(
                self.half_width_body_widths,
                other.half_width_body_widths,
                other_fraction,
            ),
            width_start_fraction: interpolate(
                self.width_start_fraction,
                other.width_start_fraction,
                other_fraction,
            ),
            taper_exponent: interpolate(self.taper_exponent, other.taper_exponent, other_fraction),
        }
    }
}

// Wings, legs, and the tail share one shape: small and short when young, longer when
// grown. Near limbs are drawn over the body, far limbs behind it. Direction offsets in
// radians are relative to the spine's backward tangent; a negative turn points down.
struct FinPair {
    fin_kind: FinKind,
    in_front: bool,
    root_along_fraction: f64,
    /// Where on the body's depth the limb is rooted: -1 is the back, 1 the belly.
    root_side_fraction: f64,
    young_shape: FinShape,
    adult_shape: FinShape,
    direction_offsets: &'static [f64],
}

#[derive(Clone, Copy, PartialEq)]
enum FinKind {
    /// The spade-tipped tail.
    Tail,
    /// Wings, with a membrane stretched between finger bones.
    Wing,
    /// Fore and hind legs, in scale color.
    Leg,
}

const fn leg_shape(length_body_widths: f64) -> FinShape {
    FinShape {
        length_body_widths,
        half_width_body_widths: 0.11,
        width_start_fraction: 0.2,
        taper_exponent: 0.5,
    }
}

const WING_SHAPE_YOUNG: FinShape = FinShape {
    length_body_widths: 1.4,
    half_width_body_widths: 0.0,
    width_start_fraction: 0.0,
    taper_exponent: 0.0,
};
const WING_SHAPE_ADULT: FinShape = FinShape {
    length_body_widths: 2.3,
    half_width_body_widths: 0.0,
    width_start_fraction: 0.0,
    taper_exponent: 0.0,
};

const FIN_PAIRS: [FinPair; 7] = [
    FinPair {
        fin_kind: FinKind::Tail,
        in_front: false,
        root_along_fraction: 1.0,
        root_side_fraction: 0.0,
        young_shape: FinShape {
            length_body_widths: 0.8,
            half_width_body_widths: 0.24,
            width_start_fraction: 0.0,
            taper_exponent: 0.6,
        },
        adult_shape: FinShape {
            length_body_widths: 1.5,
            half_width_body_widths: 0.18,
            width_start_fraction: 0.0,
            taper_exponent: 0.6,
        },
        direction_offsets: &[0.0, 0.05, 0.18, 0.34, 0.48, 0.5],
    },
    FinPair {
        fin_kind: FinKind::Wing,
        in_front: false,
        root_along_fraction: 0.33,
        root_side_fraction: -0.75,
        young_shape: WING_SHAPE_YOUNG,
        adult_shape: WING_SHAPE_ADULT,
        direction_offsets: &[],
    },
    FinPair {
        fin_kind: FinKind::Wing,
        in_front: true,
        root_along_fraction: 0.36,
        root_side_fraction: -0.8,
        young_shape: WING_SHAPE_YOUNG,
        adult_shape: WING_SHAPE_ADULT,
        direction_offsets: &[],
    },
    FinPair {
        fin_kind: FinKind::Leg,
        in_front: false,
        root_along_fraction: 0.38,
        root_side_fraction: 0.8,
        young_shape: leg_shape(0.4),
        adult_shape: leg_shape(0.55),
        direction_offsets: &[-0.8, -1.1, -1.55],
    },
    FinPair {
        fin_kind: FinKind::Leg,
        in_front: true,
        root_along_fraction: 0.41,
        root_side_fraction: 0.8,
        young_shape: leg_shape(0.4),
        adult_shape: leg_shape(0.55),
        direction_offsets: &[-1.0, -1.35, -1.8],
    },
    FinPair {
        fin_kind: FinKind::Leg,
        in_front: false,
        root_along_fraction: 0.64,
        root_side_fraction: 0.75,
        young_shape: leg_shape(0.45),
        adult_shape: leg_shape(0.65),
        direction_offsets: &[-0.6, -1.0, -1.5],
    },
    FinPair {
        fin_kind: FinKind::Leg,
        in_front: true,
        root_along_fraction: 0.67,
        root_side_fraction: 0.75,
        young_shape: leg_shape(0.45),
        adult_shape: leg_shape(0.65),
        direction_offsets: &[-0.8, -1.2, -1.75],
    },
];

fn calculate_anatomy(total_contributions: u64) -> DragonAnatomy {
    const MIN_BODY_LENGTH_PIXELS: f64 = 37.5;
    const ADULT_BODY_LENGTH_PIXELS: f64 = 137.5;
    const MAX_BODY_LENGTH_PIXELS: f64 = 167.5;
    const YOUNG_BODY_WIDTH_FRACTION: f64 = 0.27;
    const ADULT_BODY_WIDTH_FRACTION: f64 = 0.22;
    const MATURE_BODY_WIDTH_GAIN: f64 = 0.045;
    const YOUNG_HEAD_SLANT_FRACTION: f64 = 0.19;
    const ADULT_HEAD_SLANT_FRACTION: f64 = 0.15;
    const YOUNG_NOSE_WIDTH_FRACTION: f64 = 0.62;
    const YOUNG_HEAD_WIDTH_FRACTION: f64 = 0.98;
    const ADULT_HEAD_WIDTH_FRACTION: f64 = 0.78;
    const YOUNG_NECK_WIDTH_FRACTION: f64 = 0.78;
    const ADULT_NECK_WIDTH_FRACTION: f64 = 0.55;
    const ADULT_NOSE_WIDTH_FRACTION: f64 = 0.4;
    const YOUNG_TAIL_TAPER_EXPONENT: f64 = 1.25;
    const ADULT_TAIL_TAPER_EXPONENT: f64 = 1.8;
    const YOUNG_EYE_ALONG_FRACTION: f64 = 0.1;
    const ADULT_EYE_ALONG_FRACTION: f64 = 0.07;
    const YOUNG_EYE_SCALE: f64 = 2.1;
    const YOUNG_FIN_LENGTH_MULTIPLIER: f64 = 0.85;
    const MAX_FIN_LENGTH_MULTIPLIER: f64 = 1.1;
    // Fins keep their young shape for the first quarter of the young stage.
    const YOUNG_FIN_HOLD_FRACTION: f64 = 0.25;
    let growth_fraction = calculate_growth_fraction(total_contributions);
    let adult_growth_fraction = calculate_growth_fraction(ADULT_CONTRIBUTIONS);
    let young_growth_fraction = (growth_fraction / adult_growth_fraction).min(1.0);
    let mature_growth_fraction =
        ((growth_fraction - adult_growth_fraction) / (1.0 - adult_growth_fraction)).max(0.0);
    let adult_shape_fraction =
        (growth_fraction / calculate_growth_fraction(YOUNG_SHAPE_CONTRIBUTIONS)).min(1.0);
    let body_length_pixels = MIN_BODY_LENGTH_PIXELS
        + (ADULT_BODY_LENGTH_PIXELS - MIN_BODY_LENGTH_PIXELS) * young_growth_fraction
        + (MAX_BODY_LENGTH_PIXELS - ADULT_BODY_LENGTH_PIXELS) * mature_growth_fraction;
    let body_width_fraction = interpolate(
        YOUNG_BODY_WIDTH_FRACTION,
        ADULT_BODY_WIDTH_FRACTION,
        adult_shape_fraction,
    ) + MATURE_BODY_WIDTH_GAIN * mature_growth_fraction;
    DragonAnatomy {
        body_length_pixels,
        body_width_pixels: body_length_pixels * body_width_fraction,
        head_slant_fraction: interpolate(
            YOUNG_HEAD_SLANT_FRACTION,
            ADULT_HEAD_SLANT_FRACTION,
            adult_shape_fraction,
        ),
        nose_width_fraction: interpolate(
            YOUNG_NOSE_WIDTH_FRACTION,
            ADULT_NOSE_WIDTH_FRACTION,
            adult_shape_fraction,
        ),
        tail_taper_exponent: interpolate(
            YOUNG_TAIL_TAPER_EXPONENT,
            ADULT_TAIL_TAPER_EXPONENT,
            adult_shape_fraction,
        ),
        adult_fin_fraction: smooth_step(
            (adult_shape_fraction - YOUNG_FIN_HOLD_FRACTION) / (1.0 - YOUNG_FIN_HOLD_FRACTION),
        ),
        fin_length_multiplier: interpolate(
            YOUNG_FIN_LENGTH_MULTIPLIER,
            1.0,
            smooth_step(adult_shape_fraction),
        ) * interpolate(
            1.0,
            MAX_FIN_LENGTH_MULTIPLIER,
            mature_growth_fraction,
        ),
        head_width_fraction: interpolate(
            YOUNG_HEAD_WIDTH_FRACTION,
            ADULT_HEAD_WIDTH_FRACTION,
            adult_shape_fraction,
        ),
        neck_width_fraction: interpolate(
            YOUNG_NECK_WIDTH_FRACTION,
            ADULT_NECK_WIDTH_FRACTION,
            adult_shape_fraction,
        ),
        eye_along_fraction: interpolate(
            YOUNG_EYE_ALONG_FRACTION,
            ADULT_EYE_ALONG_FRACTION,
            adult_shape_fraction,
        ),
        eye_scale: interpolate(YOUNG_EYE_SCALE, 1.0, adult_shape_fraction),
    }
}

fn interpolate(from: f64, to: f64, fraction: f64) -> f64 {
    from + (to - from) * fraction
}

/// Ease in and out across zero through one; values outside are clamped.
fn smooth_step(fraction: f64) -> f64 {
    let fraction = fraction.clamp(0.0, 1.0);
    fraction * fraction * (3.0 - 2.0 * fraction)
}

/// The spine runs from the nose back through the neck, chest, and tail as an S-curve:
/// the head is held up, the neck arches down to the shoulders, the body is nearly level,
/// and the tail hangs and curls up at its tip. Each pair is a share of the body length
/// and the turn that the spine takes there; positive turns point the way back up.
const POSTURE: [(f64, f64); 9] = [
    (0.0, 0.05),
    (0.1, 0.0),
    (0.16, -0.55),
    (0.24, -1.05),
    (0.33, -0.4),
    (0.5, -0.15),
    (0.68, -0.3),
    (0.85, -0.05),
    (1.0, 0.6),
];

fn calculate_posture(along_fraction: f64) -> f64 {
    for posture_pair in POSTURE.windows(2) {
        let [(start_fraction, start_turn), (end_fraction, end_turn)] =
            [posture_pair[0], posture_pair[1]];
        if along_fraction <= end_fraction {
            return interpolate(
                start_turn,
                end_turn,
                (along_fraction - start_fraction) / (end_fraction - start_fraction),
            );
        }
    }
    POSTURE[POSTURE.len() - 1].1
}

fn build_spine(dragon_anatomy: &DragonAnatomy, flight_pose: &FlightPose) -> Vec<CurvePoint> {
    const SWISH_RADIANS: f64 = 0.3;
    const SWISH_CYCLES: f64 = 0.8;
    const MIN_HALF_WIDTH_PIXELS: f64 = 1.3;
    let point_count = dragon_anatomy.body_length_pixels.round() as usize;
    let calculate_point_half_width = |point_index: usize| {
        calculate_body_half_width(point_index as f64 / point_count as f64, dragon_anatomy)
            .max(MIN_HALF_WIDTH_PIXELS)
    };
    let mut body_spine = vec![CurvePoint {
        position: [0.0, 0.0],
        half_width_pixels: calculate_point_half_width(0),
    }];
    for point_index in 1..point_count {
        let along_fraction = point_index as f64 / point_count as f64;
        // The tail swishes, more toward its tip.
        let swish_radians = SWISH_RADIANS
            * flight_pose.swish_strength
            * along_fraction.powi(2)
            * (along_fraction * TAU * SWISH_CYCLES - flight_pose.swish_phase_radians).sin();
        let backward_angle_radians =
            PI + flight_pose.pitch_radians + calculate_posture(along_fraction) + swish_radians;
        let [previous_x, previous_y] = body_spine[point_index - 1].position;
        body_spine.push(CurvePoint {
            position: [
                previous_x + backward_angle_radians.cos(),
                previous_y + backward_angle_radians.sin(),
            ],
            half_width_pixels: calculate_point_half_width(point_index),
        });
    }
    body_spine
}

/// One limb with where it is drawn: over the body, or behind it.
struct Limb {
    part: FinPart,
    fin_kind: FinKind,
    in_front: bool,
}

/// The wing's sweep from straight up, at the top and the bottom of the beat.
const WING_UP_SWEEP_RADIANS: f64 = 0.15;
const WING_DOWN_SWEEP_RADIANS: f64 = 2.3;

fn build_fins(
    body_spine: &[CurvePoint],
    dragon_anatomy: &DragonAnatomy,
    flight_pose: &FlightPose,
) -> Vec<Limb> {
    let mut limbs = Vec::new();
    for fin_pair in &FIN_PAIRS {
        let root_index =
            (fin_pair.root_along_fraction * (body_spine.len() - 1) as f64).round() as usize;
        let root_point = body_spine[root_index];
        let forward_direction = calculate_forward_direction(body_spine, root_index);
        let backward_angle_radians = (-forward_direction[1]).atan2(-forward_direction[0]);
        let root_offset_pixels = root_point.half_width_pixels * fin_pair.root_side_fraction;
        let root_position = [
            root_point.position[0] - forward_direction[1] * root_offset_pixels,
            root_point.position[1] + forward_direction[0] * root_offset_pixels,
        ];
        let part = match fin_pair.fin_kind {
            FinKind::Wing => {
                // The wing's frame is the level body, not the arched neck it springs from.
                let wing_back_angle_radians = PI + flight_pose.pitch_radians;
                let wing_up_direction = [wing_back_angle_radians.sin(), wing_back_angle_radians.cos()];
                let wing_root = [
                    root_point.position[0] + wing_up_direction[0] * root_offset_pixels,
                    root_point.position[1] + wing_up_direction[1] * root_offset_pixels,
                ];
                let wing_beat = if fin_pair.in_front {
                    flight_pose.wing_beat
                } else {
                    flight_pose.far_wing_beat
                };
                FinPart::Wing(build_wing(
                    fin_pair,
                    wing_root,
                    wing_back_angle_radians,
                    interpolate(
                        WING_DOWN_SWEEP_RADIANS,
                        WING_UP_SWEEP_RADIANS,
                        (wing_beat.clamp(-1.0, 1.0) + 1.0) / 2.0,
                    ),
                    flight_pose.wing_bend_radians,
                    dragon_anatomy,
                ))
            }
            FinKind::Tail => FinPart::Stroke(build_fin(
                fin_pair,
                root_position,
                backward_angle_radians,
                dragon_anatomy,
                fin_pair.direction_offsets,
                0.4 * flight_pose.swish_strength
                    * (flight_pose.swish_phase_radians - 1.7).sin(),
            )),
            FinKind::Leg => FinPart::Stroke(build_fin(
                fin_pair,
                root_position,
                backward_angle_radians + flight_pose.leg_swing_radians,
                dragon_anatomy,
                fin_pair.direction_offsets,
                0.0,
            )),
        };
        limbs.push(Limb {
            part,
            fin_kind: fin_pair.fin_kind,
            in_front: fin_pair.in_front,
        });
    }
    limbs
}

/// One drawn fin part: a tapered stroke, or a wing with its bones and membrane.
enum FinPart {
    Stroke(Vec<CurvePoint>),
    Wing(Wing),
}

/// A bat wing: a membrane outline with scalloped edges, an arm bone from the shoulder
/// to the wrist, and finger bones from the wrist out to the membrane's points.
struct Wing {
    outline: Vec<[f64; 2]>,
    shoulder: [f64; 2],
    bones: Vec<[[f64; 2]; 2]>,
    span_pixels: f64,
}

/// Wing points as shares of the span: back along the body, then out from it.
const WING_WRIST: [f64; 2] = [0.05, 0.55];
const WING_FINGER_TIPS: [[f64; 2]; 4] = [[0.2, 1.0], [0.4, 0.95], [0.57, 0.78], [0.7, 0.55]];
const WING_HIND_ATTACHMENT: [f64; 2] = [0.62, 0.06];
/// How far each scallop dips toward the wrist, as a share of the way there.
const WING_SCALLOP_DEPTH: f64 = 0.3;

fn build_wing(
    fin_pair: &FinPair,
    shoulder: [f64; 2],
    backward_angle_radians: f64,
    sweep_radians: f64,
    bend_radians: f64,
    dragon_anatomy: &DragonAnatomy,
) -> Wing {
    let fin_shape = fin_pair
        .young_shape
        .blend(fin_pair.adult_shape, dragon_anatomy.adult_fin_fraction);
    let span_pixels = dragon_anatomy.body_width_pixels
        * fin_shape.length_body_widths
        * dragon_anatomy.fin_length_multiplier;
    let back_direction = [backward_angle_radians.cos(), backward_angle_radians.sin()];
    // Up from the body's back.
    let up_direction = [-back_direction[1], back_direction[0]];
    let place = |local: [f64; 2]| {
        // Points farther out flex more, so the tip trails the arm.
        let turn_radians = sweep_radians - bend_radians * local[1];
        let [turn_cos, turn_sin] = [turn_radians.cos(), turn_radians.sin()];
        let back_share = local[0] * turn_cos + local[1] * turn_sin;
        let up_share = -local[0] * turn_sin + local[1] * turn_cos;
        [0, 1].map(|axis_index| {
            shoulder[axis_index]
                + span_pixels
                    * (back_direction[axis_index] * back_share
                        + up_direction[axis_index] * up_share)
        })
    };
    let wrist = place(WING_WRIST);
    let mut membrane_points: Vec<[f64; 2]> =
        WING_FINGER_TIPS.iter().map(|&tip| place(tip)).collect();
    membrane_points.push(place(WING_HIND_ATTACHMENT));
    let mut outline = vec![shoulder, wrist];
    outline.push(membrane_points[0]);
    for tip_pair in membrane_points.windows(2) {
        let scallop_middle = [0, 1].map(|axis_index| {
            let midpoint = (tip_pair[0][axis_index] + tip_pair[1][axis_index]) / 2.0;
            midpoint + (wrist[axis_index] - midpoint) * WING_SCALLOP_DEPTH
        });
        outline.push(scallop_middle);
        outline.push(tip_pair[1]);
    }
    let mut bones = vec![[shoulder, wrist]];
    bones.extend(
        membrane_points[..WING_FINGER_TIPS.len()]
            .iter()
            .map(|&tip| [wrist, tip]),
    );
    Wing {
        outline,
        shoulder,
        bones,
        span_pixels,
    }
}

fn build_fin(
    fin_pair: &FinPair,
    root_position: [f64; 2],
    backward_angle_radians: f64,
    dragon_anatomy: &DragonAnatomy,
    direction_offsets: &[f64],
    bend_radians: f64,
) -> Vec<CurvePoint> {
    let fin_shape = fin_pair
        .young_shape
        .blend(fin_pair.adult_shape, dragon_anatomy.adult_fin_fraction);
    let fin_length_pixels = dragon_anatomy.body_width_pixels
        * fin_shape.length_body_widths
        * dragon_anatomy.fin_length_multiplier;
    let segment_count = fin_length_pixels.ceil() as usize;
    let segment_length_pixels = fin_length_pixels / segment_count as f64;
    let mut center_position = root_position;
    let mut fin_points = Vec::new();
    for point_index in 0..=segment_count {
        let along_fraction = point_index as f64 / segment_count as f64;
        let width_progress = fin_shape.width_start_fraction
            + (1.0 - fin_shape.width_start_fraction) * along_fraction;
        fin_points.push(CurvePoint {
            position: center_position,
            half_width_pixels: dragon_anatomy.body_width_pixels
                * fin_shape.half_width_body_widths
                * if fin_pair.fin_kind == FinKind::Tail {
                    // A thin stalk that swells into a spade at the tip.
                    (PI * ((along_fraction - 0.55) / 0.45).clamp(0.0, 1.0))
                        .sin()
                        .max(0.0)
                        .powf(0.8)
                        .max(0.5)
                } else {
                    (PI * width_progress)
                        .sin()
                        .max(0.0)
                        .powf(fin_shape.taper_exponent)
                },
        });
        let direction_position = (along_fraction * direction_offsets.len() as f64 - 0.5)
            .clamp(0.0, (direction_offsets.len() - 1) as f64);
        let direction_index = direction_position.floor() as usize;
        let start_angle = direction_offsets[direction_index];
        let end_angle = direction_offsets[(direction_index + 1).min(direction_offsets.len() - 1)];
        let segment_angle = backward_angle_radians
            + start_angle
            + (end_angle - start_angle) * direction_position.fract()
            + bend_radians * along_fraction;
        center_position[0] += segment_length_pixels * segment_angle.cos();
        center_position[1] += segment_length_pixels * segment_angle.sin();
    }
    fin_points
}

/// Paint the horn points. They fill only empty pixels and are painted before the
/// outline, so they are outlined like the dragon.
fn paint_horns(rgba_pixels: &mut [[u8; 4]], horn_points: &[[f64; 2]], pose_center: [f64; 2]) {
    let frame_width = FRAME_WIDTH_PIXELS as usize;
    for horn_point in horn_points {
        let pixel_x =
            (horn_point[0] - pose_center[0] + f64::from(FRAME_WIDTH_PIXELS) / 2.0).floor();
        let pixel_y =
            (horn_point[1] - pose_center[1] + f64::from(FRAME_HEIGHT_PIXELS) / 2.0).floor();
        if !(0.0..f64::from(FRAME_WIDTH_PIXELS)).contains(&pixel_x)
            || !(0.0..f64::from(FRAME_HEIGHT_PIXELS)).contains(&pixel_y)
        {
            continue;
        }
        let pixel_index = pixel_y as usize * frame_width + pixel_x as usize;
        if rgba_pixels[pixel_index][3] == 0 {
            rgba_pixels[pixel_index] = HORN_COLOR;
        }
    }
}

/// One point per horn pixel: two short bones that sweep up and back from the top of
/// the head, swung together by the sway.
fn calculate_horn_points(
    body_spine: &[CurvePoint],
    dragon_anatomy: &DragonAnatomy,
    sway_radians: f64,
) -> Vec<[f64; 2]> {
    // Root along the head, the angle from the heading, and the share of full length.
    const HORNS: [(f64, f64, f64); 2] = [(0.9, -2.05, 1.0), (0.78, -1.7, 0.75)];
    const CURL_RADIANS_PER_PIXEL: f64 = 0.1;
    const ROOT_DEPTH_FRACTION: f64 = 0.8;
    let horn_length_pixels = f64::from(calculate_horn_length(dragon_anatomy));
    let mut horn_points = Vec::new();
    for (root_along_head, outward_radians, length_share) in HORNS {
        let root_index = ((dragon_anatomy.head_slant_fraction * root_along_head)
            * (body_spine.len() - 1) as f64)
            .round() as usize;
        let root_point = body_spine[root_index];
        let forward_direction = calculate_forward_direction(body_spine, root_index);
        let heading_radians = forward_direction[1].atan2(forward_direction[0]);
        let depth_offset_pixels = -root_point.half_width_pixels * ROOT_DEPTH_FRACTION;
        let mut horn_position = [
            root_point.position[0] - forward_direction[1] * depth_offset_pixels,
            root_point.position[1] + forward_direction[0] * depth_offset_pixels,
        ];
        let mut horn_angle = heading_radians + sway_radians + outward_radians;
        for _ in 0..(horn_length_pixels * length_share).round() as usize {
            horn_position[0] += horn_angle.cos();
            horn_position[1] += horn_angle.sin();
            horn_points.push(horn_position);
            // A second pixel beside each one makes the horn two pixels thick.
            horn_points.push([horn_position[0] + 1.0, horn_position[1]]);
            // Each horn curls back toward the tail as it grows.
            horn_angle -= CURL_RADIANS_PER_PIXEL;
        }
    }
    horn_points
}

/// A row of spines along the back, small near the neck, tallest over the shoulders and
/// hips, and shrinking toward the tail. Each leans back and fills only empty pixels, so
/// it rises from the body's edge and is outlined with the dragon.
fn paint_spines(
    rgba_pixels: &mut [[u8; 4]],
    body_spine: &[CurvePoint],
    dragon_anatomy: &DragonAnatomy,
    pose_center: [f64; 2],
    spine_color: [u8; 4],
) {
    const FIRST_ALONG_FRACTION: f64 = 0.19;
    const LAST_ALONG_FRACTION: f64 = 0.92;
    const BODY_PIXELS_PER_SPINE: f64 = 24.0;
    let frame_width = FRAME_WIDTH_PIXELS as usize;
    let spine_count = ((LAST_ALONG_FRACTION - FIRST_ALONG_FRACTION)
        * dragon_anatomy.body_length_pixels
        / (dragon_anatomy.body_length_pixels / BODY_PIXELS_PER_SPINE).clamp(3.0, 6.0))
    .round() as usize;
    for spine_index in 0..spine_count {
        let along_fraction = FIRST_ALONG_FRACTION
            + (LAST_ALONG_FRACTION - FIRST_ALONG_FRACTION) * (spine_index as f64 + 0.5)
                / spine_count as f64;
        let point_index = (along_fraction * (body_spine.len() - 1) as f64) as usize;
        let body_point = body_spine[point_index];
        let forward_direction = calculate_forward_direction(body_spine, point_index);
        let up_direction = [forward_direction[1], -forward_direction[0]];
        let height_pixels = (dragon_anatomy.body_width_pixels * 0.2).clamp(1.5, 6.0)
            * (1.0 - 0.6 * ((along_fraction - 0.45) / 0.47).clamp(0.0, 1.0));
        let step_count = height_pixels.ceil() as i32;
        for step in 0..=step_count {
            let step_fraction = f64::from(step) / f64::from(step_count + 1);
            let half_width_pixels = 1.5 * (1.0 - step_fraction);
            let mut offset_pixels = -half_width_pixels;
            while offset_pixels <= half_width_pixels {
                let rise_pixels = body_point.half_width_pixels * 0.85 + f64::from(step);
                let lean_pixels = -0.6 * f64::from(step) + offset_pixels;
                let spine_position = [
                    body_point.position[0]
                        + up_direction[0] * rise_pixels
                        + forward_direction[0] * lean_pixels,
                    body_point.position[1]
                        + up_direction[1] * rise_pixels
                        + forward_direction[1] * lean_pixels,
                ];
                let pixel_x =
                    (spine_position[0] - pose_center[0] + f64::from(FRAME_WIDTH_PIXELS) / 2.0)
                        .floor();
                let pixel_y =
                    (spine_position[1] - pose_center[1] + f64::from(FRAME_HEIGHT_PIXELS) / 2.0)
                        .floor();
                if (0.0..f64::from(FRAME_WIDTH_PIXELS)).contains(&pixel_x)
                    && (0.0..f64::from(FRAME_HEIGHT_PIXELS)).contains(&pixel_y)
                {
                    let pixel_index = pixel_y as usize * frame_width + pixel_x as usize;
                    if rgba_pixels[pixel_index][3] == 0 {
                        rgba_pixels[pixel_index] = spine_color;
                    }
                }
                offset_pixels += 1.0;
            }
        }
    }
}

/// Horns grow with the dragon, from three to eight pixels.
fn calculate_horn_length(dragon_anatomy: &DragonAnatomy) -> u32 {
    const BODY_PIXELS_PER_HORN_PIXEL: f64 = 22.0;
    (dragon_anatomy.body_length_pixels / BODY_PIXELS_PER_HORN_PIXEL)
        .round()
        .clamp(3.0, 8.0) as u32
}

/// The corners of the box that holds every curve, including its half widths.
fn measure_curve_bounds(body_spine: &[CurvePoint], limbs: &[Limb]) -> [[f64; 2]; 2] {
    let mut min_corner = [f64::INFINITY; 2];
    let mut max_corner = [f64::NEG_INFINITY; 2];
    let mut include = |position: [f64; 2], half_width_pixels: f64| {
        for axis_index in 0..2 {
            min_corner[axis_index] =
                min_corner[axis_index].min(position[axis_index] - half_width_pixels);
            max_corner[axis_index] =
                max_corner[axis_index].max(position[axis_index] + half_width_pixels);
        }
    };
    for curve_point in body_spine {
        include(curve_point.position, curve_point.half_width_pixels);
    }
    for limb in limbs {
        match &limb.part {
            FinPart::Stroke(fin_points) => {
                for curve_point in fin_points {
                    include(curve_point.position, curve_point.half_width_pixels);
                }
            }
            FinPart::Wing(wing) => {
                for &outline_point in &wing.outline {
                    include(outline_point, 0.0);
                }
            }
        }
    }
    [min_corner, max_corner]
}

/// Near limbs first, then the body, then the far limbs, which are darker for depth.
fn sample_dragon(
    body_spine: &[CurvePoint],
    limbs: &[Limb],
    sample_position: [f64; 2],
    pixel_position: [u32; 2],
    dragon_appearance: &DragonAppearance,
) -> [u8; 4] {
    const FAR_LIMB_BRIGHTNESS: f64 = 0.68;
    let sample_limb = |limb: &Limb| match &limb.part {
        FinPart::Stroke(fin_curve) => sample_curve(fin_curve, sample_position).map(|fin_sample| {
            shade_fin(
                &fin_sample,
                limb.fin_kind,
                pixel_position,
                dragon_appearance.dragon_coat,
            )
        }),
        FinPart::Wing(wing) => shade_wing(
            wing,
            sample_position,
            pixel_position,
            dragon_appearance.dragon_coat,
        ),
    };
    for limb in limbs.iter().filter(|limb| limb.in_front) {
        if let Some(limb_color) = sample_limb(limb) {
            return limb_color;
        }
    }
    if let Some(body_sample) = sample_curve(body_spine, sample_position) {
        return shade_body(&body_sample, pixel_position, dragon_appearance);
    }
    for limb in limbs.iter().filter(|limb| !limb.in_front) {
        if let Some(limb_color) = sample_limb(limb) {
            let [red, green, blue, alpha] = limb_color;
            let shade = |channel: u8| (f64::from(channel) * FAR_LIMB_BRIGHTNESS).round() as u8;
            return [shade(red), shade(green), shade(blue), alpha];
        }
    }
    [0; 4]
}

// Constructed curves contain at least two points and no zero-length segments.
fn sample_curve(curve_points: &[CurvePoint], sample_position: [f64; 2]) -> Option<CurveSample> {
    let mut closest_sample = None;
    let mut closest_distance_pixels = f64::INFINITY;
    for (segment_index, end_points) in curve_points.windows(2).enumerate() {
        let [start_point, end_point] = [end_points[0], end_points[1]];
        let segment_x = end_point.position[0] - start_point.position[0];
        let segment_y = end_point.position[1] - start_point.position[1];
        let offset_x = sample_position[0] - start_point.position[0];
        let offset_y = sample_position[1] - start_point.position[1];
        let segment_length_pixels = segment_x.hypot(segment_y);
        let segment_progress = ((offset_x * segment_x + offset_y * segment_y)
            / segment_length_pixels.powi(2))
        .clamp(0.0, 1.0);
        let distance_pixels = (offset_x - segment_progress * segment_x)
            .hypot(offset_y - segment_progress * segment_y);
        if distance_pixels >= closest_distance_pixels {
            continue;
        }
        closest_distance_pixels = distance_pixels;
        closest_sample = Some(CurveSample {
            along_fraction: (segment_index as f64 + segment_progress)
                / (curve_points.len() - 1) as f64,
            lateral_pixels: (offset_x * segment_y - offset_y * segment_x) / segment_length_pixels,
            distance_pixels,
            half_width_pixels: start_point.half_width_pixels
                + segment_progress * (end_point.half_width_pixels - start_point.half_width_pixels),
            normal_direction: [
                segment_y / segment_length_pixels,
                -segment_x / segment_length_pixels,
            ],
        });
    }
    closest_sample
        .filter(|curve_sample| curve_sample.distance_pixels <= curve_sample.half_width_pixels)
}

fn shade_body(
    body_sample: &CurveSample,
    pixel_position: [u32; 2],
    dragon_appearance: &DragonAppearance,
) -> [u8; 4] {
    let dragon_anatomy = dragon_appearance.dragon_anatomy;
    let body_length_pixels = dragon_anatomy.body_length_pixels;
    let lateral_fraction = body_sample.lateral_pixels / body_sample.half_width_pixels;
    let body_paint = dragon_appearance.dragon_coat.select_body_paint(
        body_sample.along_fraction,
        lateral_fraction,
        body_length_pixels,
        body_sample.half_width_pixels,
        calculate_dither_threshold(pixel_position),
    );
    const INNER_RIM_PIXELS: f64 = 0.8;
    if body_sample.distance_pixels > body_sample.half_width_pixels - INNER_RIM_PIXELS {
        return body_paint.shade_ramp[0];
    }
    // One round amber eye with a dark pupil, high on the side of the head.
    let eye_radius_pixels = 1.7 * dragon_anatomy.eye_scale.sqrt();
    let eye_offset_pixels = [
        (body_sample.along_fraction - dragon_anatomy.eye_along_fraction) * body_length_pixels,
        (lateral_fraction + 0.3) * body_sample.half_width_pixels,
    ];
    let eye_distance_pixels = eye_offset_pixels[0].hypot(eye_offset_pixels[1]);
    if eye_distance_pixels < eye_radius_pixels {
        return if eye_distance_pixels < eye_radius_pixels * 0.5 {
            OUTLINE_COLOR
        } else {
            dragon_appearance.dragon_coat.select_eye_color()
        };
    }
    if let Some(glint_color) = body_paint.glint_color {
        return glint_color;
    }
    let surface_brightness = calculate_body_brightness(body_sample, lateral_fraction, dragon_anatomy)
        + body_paint.brightness_shift;
    adjust_saturation(
        choose_shade(body_paint.shade_ramp, surface_brightness, pixel_position),
        body_paint.saturation,
    )
}

fn calculate_body_brightness(
    body_sample: &CurveSample,
    lateral_fraction: f64,
    dragon_anatomy: &DragonAnatomy,
) -> f64 {
    const AMBIENT_BRIGHTNESS: f64 = 0.24;
    const DIFFUSE_STRENGTH: f64 = 0.7;
    const LIGHT_DIRECTION: [f64; 3] = [-0.32, -0.5, 0.8];
    let surface_normal_z = (1.0 - lateral_fraction.powi(2)).max(0.0).sqrt();
    let incident_light = lateral_fraction
        * (body_sample.normal_direction[0] * LIGHT_DIRECTION[0]
            + body_sample.normal_direction[1] * LIGHT_DIRECTION[1])
        + surface_normal_z * LIGHT_DIRECTION[2];
    let mut surface_brightness = AMBIENT_BRIGHTNESS + DIFFUSE_STRENGTH * incident_light.max(0.0);
    // A ridge of spines runs down the back, dark and light in turn.
    let dorsal_span = (dragon_anatomy.head_slant_fraction + 0.05)..0.9;
    let dorsal_half_width_fraction = 0.7;
    if dorsal_span.contains(&body_sample.along_fraction)
        && lateral_fraction < -dorsal_half_width_fraction
    {
        let spine_phase = (body_sample.along_fraction * dragon_anatomy.body_length_pixels / 3.0)
            .rem_euclid(1.0);
        surface_brightness += if spine_phase < 0.5 { -0.22 } else { 0.18 };
    }
    // The gill line sits near the back of the head's slant.
    let gill_along_fraction = dragon_anatomy.head_slant_fraction * 0.72;
    let gill_inner_fraction = 0.3;
    let gill_shadow = 0.22;
    if ((body_sample.along_fraction - gill_along_fraction) * dragon_anatomy.body_length_pixels).abs()
        < 0.5
        && lateral_fraction.abs() > gill_inner_fraction
    {
        surface_brightness -= gill_shadow;
    }
    surface_brightness
}

/// Whether the point lies inside the polygon, by counting edge crossings.
fn is_inside_polygon(outline: &[[f64; 2]], point: [f64; 2]) -> bool {
    let mut is_inside = false;
    let mut previous = outline[outline.len() - 1];
    for &vertex in outline {
        if (vertex[1] > point[1]) != (previous[1] > point[1])
            && point[0]
                < (previous[0] - vertex[0]) * (point[1] - vertex[1]) / (previous[1] - vertex[1])
                    + vertex[0]
        {
            is_inside = !is_inside;
        }
        previous = vertex;
    }
    is_inside
}

fn calculate_segment_distance(segment: [[f64; 2]; 2], point: [f64; 2]) -> f64 {
    let [start, end] = segment;
    let segment_x = end[0] - start[0];
    let segment_y = end[1] - start[1];
    let squared_length = (segment_x * segment_x + segment_y * segment_y).max(1e-9);
    let progress = (((point[0] - start[0]) * segment_x + (point[1] - start[1]) * segment_y)
        / squared_length)
        .clamp(0.0, 1.0);
    (point[0] - start[0] - progress * segment_x).hypot(point[1] - start[1] - progress * segment_y)
}

/// A wing pixel: dark bones, a rim along the membrane's edge, and a membrane that
/// brightens away from the bones.
fn shade_wing(
    wing: &Wing,
    sample_position: [f64; 2],
    pixel_position: [u32; 2],
    dragon_coat: &DragonCoat,
) -> Option<[u8; 4]> {
    const BONE_HALF_WIDTH_PIXELS: f64 = 0.75;
    const RIM_PIXELS: f64 = 0.8;
    const BONE_FADE_PIXELS: f64 = 7.0;
    if !is_inside_polygon(&wing.outline, sample_position) {
        return None;
    }
    let mut edge_distance_pixels = f64::INFINITY;
    let mut previous = wing.outline[wing.outline.len() - 1];
    for &vertex in &wing.outline {
        edge_distance_pixels =
            edge_distance_pixels.min(calculate_segment_distance([previous, vertex], sample_position));
        previous = vertex;
    }
    let bone_distance_pixels = wing
        .bones
        .iter()
        .map(|&bone| calculate_segment_distance(bone, sample_position))
        .fold(f64::INFINITY, f64::min);
    let is_on_rim = edge_distance_pixels < RIM_PIXELS;
    let along_fraction = ((sample_position[0] - wing.shoulder[0])
        .hypot(sample_position[1] - wing.shoulder[1])
        / wing.span_pixels)
        .min(1.0);
    let fin_paint = dragon_coat.select_fin_paint(
        along_fraction,
        0.0,
        is_on_rim,
        calculate_dither_threshold(pixel_position),
    );
    if bone_distance_pixels < BONE_HALF_WIDTH_PIXELS {
        return Some(adjust_saturation(fin_paint.shade_ramp[0], fin_paint.saturation));
    }
    if is_on_rim {
        return Some(adjust_saturation(fin_paint.shade_ramp[1], fin_paint.saturation));
    }
    let surface_brightness = 0.3 + 0.4 * (bone_distance_pixels / BONE_FADE_PIXELS).min(1.0);
    Some(adjust_saturation(
        choose_shade(fin_paint.shade_ramp, surface_brightness, pixel_position),
        fin_paint.saturation,
    ))
}

fn shade_fin(
    fin_sample: &CurveSample,
    fin_kind: FinKind,
    pixel_position: [u32; 2],
    dragon_coat: &DragonCoat,
) -> [u8; 4] {
    const FIN_RIM_PIXELS: f64 = 0.6;
    const WING_BONE_SPACING_PIXELS: f64 = 2.6;
    const FIN_ROOT_SHADOW: f64 = 0.12;
    let is_on_rim = fin_sample.distance_pixels > fin_sample.half_width_pixels - FIN_RIM_PIXELS;
    if fin_kind == FinKind::Leg || (fin_kind == FinKind::Tail && fin_sample.along_fraction < 0.55) {
        let limb_ramp = dragon_coat.select_limb_ramp();
        return if is_on_rim {
            limb_ramp[0]
        } else {
            choose_shade(limb_ramp, 0.55, pixel_position)
        };
    }
    let fin_paint = dragon_coat.select_fin_paint(
        fin_sample.along_fraction,
        fin_sample.lateral_pixels / fin_sample.half_width_pixels,
        is_on_rim,
        calculate_dither_threshold(pixel_position),
    );
    if is_on_rim {
        return adjust_saturation(fin_paint.shade_ramp[1], fin_paint.saturation);
    }
    let surface_brightness = if fin_kind == FinKind::Wing {
        // Dark finger bones fan out across a lighter membrane.
        let bone_phase = (fin_sample.lateral_pixels / WING_BONE_SPACING_PIXELS).rem_euclid(1.0);
        if bone_phase < 0.22 { 0.3 } else { 0.86 }
    } else {
        0.8 - 0.25 * fin_sample.lateral_pixels.abs() / fin_sample.half_width_pixels
    };
    let root_shadow = FIN_ROOT_SHADOW * (1.0 - fin_sample.along_fraction).powi(2);
    adjust_saturation(
        choose_shade(
            fin_paint.shade_ramp,
            surface_brightness - root_shadow,
            pixel_position,
        ),
        fin_paint.saturation,
    )
}

/// The body is drawn as a chain of round segments: a wedge-shaped head that widens from
/// the snout to the jaw, a narrower neck, a deep chest, and a long tapering tail.
/// Young dragons have a bigger, blunter head on a thicker neck.
fn calculate_body_half_width(along_fraction: f64, dragon_anatomy: &DragonAnatomy) -> f64 {
    const CHEST_FRACTION: f64 = 0.34;
    const NECK_LENGTH_FRACTION: f64 = 0.06;
    const TAPER_START_FRACTION: f64 = 0.5;
    const TAPER_END_FRACTION: f64 = 0.96;
    const TAIL_WIDTH_FRACTION: f64 = 0.2;
    let head_end_fraction = dragon_anatomy.head_slant_fraction;
    let head_width_fraction = dragon_anatomy.head_width_fraction;
    let neck_width_fraction = dragon_anatomy.neck_width_fraction;
    let neck_start_fraction = head_end_fraction + 0.015;
    let chest_start_fraction = neck_start_fraction + NECK_LENGTH_FRACTION;
    let width_fraction = if along_fraction < head_end_fraction {
        let nose_width_fraction = dragon_anatomy.nose_width_fraction;
        nose_width_fraction
            + (head_width_fraction - nose_width_fraction)
                * (along_fraction / head_end_fraction * PI / 2.0).sin().powf(0.8)
    } else if along_fraction < chest_start_fraction {
        let neck_progress =
            ((along_fraction - head_end_fraction) / (chest_start_fraction - head_end_fraction))
                .clamp(0.0, 1.0);
        interpolate(
            head_width_fraction,
            neck_width_fraction,
            smooth_step(neck_progress * 1.6),
        )
    } else if along_fraction < CHEST_FRACTION {
        let chest_progress =
            (along_fraction - chest_start_fraction) / (CHEST_FRACTION - chest_start_fraction);
        interpolate(neck_width_fraction, 1.0, smooth_step(chest_progress))
    } else {
        (1.0 - ((along_fraction - TAPER_START_FRACTION).max(0.0)
            / (TAPER_END_FRACTION - TAPER_START_FRACTION))
            .powf(dragon_anatomy.tail_taper_exponent))
        .max(TAIL_WIDTH_FRACTION)
    };
    dragon_anatomy.body_width_pixels / 2.0 * width_fraction
}

fn calculate_forward_direction(curve_points: &[CurvePoint], point_index: usize) -> [f64; 2] {
    const DIRECTION_SPAN_SAMPLES: usize = 2;
    let earlier_point = curve_points[point_index.saturating_sub(DIRECTION_SPAN_SAMPLES)].position;
    let later_point =
        curve_points[(point_index + DIRECTION_SPAN_SAMPLES).min(curve_points.len() - 1)].position;
    let tangent_x = earlier_point[0] - later_point[0];
    let tangent_y = earlier_point[1] - later_point[1];
    let tangent_length_pixels = tangent_x.hypot(tangent_y);
    [
        tangent_x / tangent_length_pixels,
        tangent_y / tangent_length_pixels,
    ]
}

fn paint_outline(rgba_pixels: &mut [[u8; 4]]) {
    let original_pixels = rgba_pixels.to_vec();
    let frame_width = FRAME_WIDTH_PIXELS as usize;
    for (pixel_index, pixel_channels) in rgba_pixels.iter_mut().enumerate() {
        if original_pixels[pixel_index][3] != 0 {
            continue;
        }
        let pixel_x = pixel_index % frame_width;
        let pixel_y = pixel_index / frame_width;
        let touches_shape = (pixel_x > 0 && original_pixels[pixel_index - 1][3] != 0)
            || (pixel_x + 1 < frame_width && original_pixels[pixel_index + 1][3] != 0)
            || (pixel_y > 0 && original_pixels[pixel_index - frame_width][3] != 0)
            || (pixel_y + 1 < FRAME_HEIGHT_PIXELS as usize
                && original_pixels[pixel_index + frame_width][3] != 0);
        if touches_shape {
            *pixel_channels = OUTLINE_COLOR;
        }
    }
}

fn choose_shade(
    shade_ramp: &[[u8; 4]],
    surface_brightness: f64,
    pixel_position: [u32; 2],
) -> [u8; 4] {
    let shade_position = surface_brightness.clamp(0.0, 1.0) * (shade_ramp.len() - 1) as f64;
    let base_index = shade_position.floor() as usize;
    let shade_index = base_index
        + usize::from(shade_position.fract() > calculate_dither_threshold(pixel_position));
    shade_ramp[shade_index.min(shade_ramp.len() - 1)]
}

/// Scale a color's distance from its own gray; one leaves it unchanged.
fn adjust_saturation(pixel_color: [u8; 4], saturation: f64) -> [u8; 4] {
    let [red, green, blue, alpha] = pixel_color.map(f64::from);
    let gray = 0.3 * red + 0.59 * green + 0.11 * blue;
    let saturate = |channel: f64| {
        (gray + (channel - gray) * saturation)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [saturate(red), saturate(green), saturate(blue), alpha as u8]
}

/// The ordered-dither threshold at a pixel, from zero through one.
fn calculate_dither_threshold(pixel_position: [u32; 2]) -> f64 {
    let dither_x = pixel_position[0] as usize % DITHER_RANKS[0].len();
    let dither_y = pixel_position[1] as usize % DITHER_RANKS.len();
    (f64::from(DITHER_RANKS[dither_y][dither_x]) + 0.5)
        / (DITHER_RANKS.len() * DITHER_RANKS[0].len()) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paint only the mouth on an empty canvas centered on it.
    fn paint_mouth_alone(dragon_body: DragonBody, mouth_opening: f64) -> Vec<[u8; 4]> {
        let dragon_anatomy = calculate_anatomy(1_500);
        let body_spine = build_spine(&dragon_anatomy, &STILL_POSE);
        let mut rgba_pixels = vec![[0; 4]; (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize];
        let mouth_position = calculate_mouth_position(&body_spine);
        paint_mouth(
            &mut rgba_pixels,
            &body_spine,
            dragon_body,
            mouth_position,
            mouth_opening,
        );
        rgba_pixels
    }

    fn center_pixel(rgba_pixels: &[[u8; 4]]) -> [u8; 4] {
        rgba_pixels
            [(FRAME_HEIGHT_PIXELS / 2 * FRAME_WIDTH_PIXELS + FRAME_WIDTH_PIXELS / 2) as usize]
    }

    fn count_painted(rgba_pixels: &[[u8; 4]]) -> usize {
        rgba_pixels
            .iter()
            .filter(|pixel_channels| pixel_channels[3] != 0)
            .count()
    }

    #[test]
    fn the_mouth_opens_as_a_body_colored_rim_around_a_dark_opening() {
        assert_eq!(count_painted(&paint_mouth_alone(DragonBody::Emerald, 0.1)), 0);
        for (dragon_body, rim_color) in [
            (DragonBody::Emerald, [64, 160, 84, 255]),
            (DragonBody::Obsidian, [58, 68, 102, 255]),
        ] {
            let open_mouth = paint_mouth_alone(dragon_body, 1.0);
            assert_eq!(center_pixel(&open_mouth), OUTLINE_COLOR);
            assert!(open_mouth.contains(&rim_color), "{dragon_body:?}");
            assert!(
                open_mouth
                    .iter()
                    .all(|&pixel| pixel == [0; 4] || pixel == OUTLINE_COLOR || pixel == rim_color)
            );
        }
        let half_open = count_painted(&paint_mouth_alone(DragonBody::Emerald, 0.5));
        let wide_open = count_painted(&paint_mouth_alone(DragonBody::Emerald, 1.0));
        assert!(0 < half_open && half_open < wide_open);
    }
}
