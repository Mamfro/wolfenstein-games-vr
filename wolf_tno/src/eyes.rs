//! How the eyes are made and placed, from the run's options (`wolf-options.txt` beside the DLL;
//! both games take the same): the eye site's settings beside the shared ones ([`Shared`]).

use monaka_contract::manifest::{Kind, Manifest, Opt, Shown, SteamFiles, choice, opt};
use monaka_core::options::{Options, Shared};
use monaka_core::rendering::{FrameGenerator, Offer, STEREO_OFF, StereoMethod, frame_generation, stereo_method};
use monaka_contract::protocol::HeadPose;

/// The eye that sits where the game camera is, so it looks straight down the weapon's sights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DominantEye {
    /// Both eyes either side of the game camera (`dominant_eye=none`; Monaka's setting offers only
    /// left and right).
    None,
    Left,
    Right,
}

/// How the eyes are made: both in one frame by the engine's side-by-side layout, each drawn on its
/// own ([`TWO_PASS_STEREO`]) or both by one pass ([`SINGLE_PASS_STEREO`]), or one per frame with the
/// other warped from it by its depth (AFW).
pub const STEREO: Opt = stereo_method(&[TWO_PASS_STEREO, SINGLE_PASS_STEREO, StereoMethod::Afw.experimental(), STEREO_OFF], StereoMethod::SameFrame).help(STEREO_HELP);

const STEREO_HELP: &str = "Same Frame Stereo renders both eyes in one game frame: Two Pass draws the scene for each eye, Single Pass once for both (faster, NVIDIA only, and the first level load freezes for about 23 seconds). AFW (Frame-warp) renders one eye per frame and warps it into the other eye with its depth. A method marked experimental is not yet this game's default because it is still being proven.";

/// Same-frame stereo as it always was: the engine draws the scene for each eye.
pub const TWO_PASS_STEREO: Offer = (StereoMethod::SameFrame.value(), "Same Frame Stereo (Two Pass)");

/// Single pass ([`crate::single_pass`]): same-frame stereo with the engine's scene programs
/// converted to draw both eyes at once. Everything else takes it as [`TWO_PASS_STEREO`]; any other
/// method leaves single pass off.
pub const SINGLE_PASS_STEREO: Offer = ("single", "Same Frame Stereo (Single Pass)");

/// Depth of field and film grain, which the single pass otherwise leaves out of the game's shaders
/// (no cvar turns them off, so the other stereo methods keep them).
pub const FILM_EFFECTS: Opt = opt("film_effects", "Depth of field and film grain", "The game only gets them right in one eye.", Kind::Switch { default: false })
    .on(Shown::Rendering)
    .only_when("stereo", SINGLE_PASS_STEREO.0);

/// Whether the run's stereo method is [`SINGLE_PASS_STEREO`].
pub fn single_pass_on(options: &Options) -> bool {
    options.text(STEREO.id).is_some_and(|method| method.trim().eq_ignore_ascii_case(SINGLE_PASS_STEREO.0))
}

/// FSR upscaling ([`EyeSettings::fsr`]): with frame warping, each eye drawn smaller and upscaled.
pub const FSR: Opt = opt("fsr", "FSR upscaling", "Renders each eye smaller, then upscales it. Needs Dying Light 2's FidelityFX DLLs.", Kind::Switch { default: false })
    .on(Shown::Rendering)
    .only_when("stereo", "afw")
    // AMD's FidelityFX runtime as Dying Light 2 installs it (Monaka ships none).
    .needs_steam_files(SteamFiles {
        game_folder: "Dying Light 2",
        path: r"ph\work\bin\x64",
        file: "amd_fidelityfx_upscaler_dx12.dll",
        line: "fsr_dlls",
        missing: "FSR is off: it needs AMD's FidelityFX DLLs, which come with Dying Light 2 (not installed).",
    });
pub const FSR_SCALE: Opt = opt("fsr_scale", "FSR quality", "How much smaller each eye renders.", choice(&[("1.5", "Quality"), ("1.7", "Balanced"), ("2", "Performance"), ("3", "Ultra performance")], "1.5"))
    .on(Shown::Rendering)
    .only_when("fsr", "1");
pub const FSR_SHARPNESS: Opt =
    opt("fsr_sharpness", "FSR sharpness", "0 turns sharpening off.", Kind::Number { min: 0.0, max: 1.0, step: 0.05, default: 0.3 }).on(Shown::Rendering).only_when("fsr", "1");

/// Frame generation ([`EyeSettings::framegen`]): with frame warping, in-between pairs of the warped
/// pairs by AMD's FSR frame generation. Offered with FSR upscaling, whose option brings AMD's DLLs.
pub const FRAMEGEN: Opt = frame_generation(&[FrameGenerator::Off.offer(), FrameGenerator::Fsr.experimental()], FrameGenerator::Off).only_when("fsr", "1");

/// FSR upscaling settings (`fsr_*` options).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fsr {
    /// Eye size over render size per side (`fsr_scale`; 1.5 is AMD's Quality, 2 Performance).
    pub scale: f32,
    /// RCAS sharpening 0..1 (`fsr_sharpness`; 0 turns it off).
    pub sharpness: Option<f32>,
    /// 1 or -1: whether the dispatch is told the content shift or its opposite (`fsr_jitter_sign`,
    /// command line only).
    pub jitter_sign: f32,
}

/// Whether the run's `stereo` option leaves stereo on (`stereo=off` is off).
pub fn stereo_on(options: &Options) -> bool {
    options.text("stereo").map(str::trim) != Some(monaka_core::rendering::STEREO_OFF.0)
}

/// The game's world units per metre (the player's view height of 87 units suggests 50).
pub const UNITS_PER_METRE: f32 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeSettings {
    /// Stereo: the engine draws the eyes; otherwise the whole frame goes to both eyes.
    pub stereo: bool,
    /// How the eyes are made, with stereo on ([`STEREO`]).
    pub stereo_method: StereoMethod,
    /// FSR upscaling (`fsr=1`, with frame warping): each eye drawn smaller and upscaled.
    pub fsr: Option<Fsr>,
    /// Frame generation (`framegen=fsr`, with frame warping): in-between pairs of the warped pairs.
    pub framegen: bool,
    /// Frame warping: the dominant eye drawn most frames, the other every refresh, instead of the
    /// eyes taking turns (`afw_dominant=1`, command line only: the gaps beside near things then fill
    /// from a frame up to a refresh old, which shows moving hands where they were).
    pub afw_dominant: bool,
    /// World units per metre ([`UNITS_PER_METRE`]).
    pub units_per_metre: f32,
    /// Projection shift per eye in screen units (the engine's stereoRender_screenSeparation),
    /// used only when the headset's own projection is not.
    pub screen_separation: f32,
    pub dominant_eye: DominantEye,
    /// While aiming down sights (the game camera zoomed in), the eyes slide onto the game camera
    /// whatever `head_position` says, so the sights line up.
    pub sight_lock: bool,
    /// Keep god rays and lens flares, which the engine draws in the right eye only.
    pub god_rays: bool,
    pub flares: bool,
    /// Keep screen-space reflections, which the engine traces with the wrong eye's camera.
    pub reflections: bool,
    /// Keep dim shadows, which can cover one eye with a large straight-edged shadow the other eye
    /// does not have.
    pub dim_shadows: bool,
}

impl EyeSettings {
    pub fn from_options(options: &Options, manifest: &Manifest) -> Self {
        let eyes = [("left", DominantEye::Left), ("right", DominantEye::Right), ("none", DominantEye::None), ("off", DominantEye::None), ("0", DominantEye::None)];
        let dominant_default = match manifest.choice_default("dominant_eye") {
            Some("left") => DominantEye::Left,
            _ => DominantEye::Right,
        };
        Self {
            stereo: stereo_on(options),
            stereo_method: StereoMethod::chosen(&STEREO, options),
            afw_dominant: options.switch("afw_dominant", false),
            fsr: options.switch("fsr", false).then(|| Fsr {
                scale: options.number("fsr_scale", 1.0, 3.0, 1.5),
                sharpness: Some(options.number("fsr_sharpness", 0.0, 1.0, 0.3)).filter(|&s| s > 0.0),
                jitter_sign: if options.number("fsr_jitter_sign", -1.0, 1.0, 1.0) < 0.0 { -1.0 } else { 1.0 },
            }),
            framegen: FrameGenerator::chosen(&FRAMEGEN, options) == FrameGenerator::Fsr,
            units_per_metre: UNITS_PER_METRE,
            screen_separation: options.number("screen_separation", -1.0, 1.0, 0.0),
            dominant_eye: options.choice("dominant_eye", &eyes, dominant_default),
            sight_lock: true,
            god_rays: options.switch("god_rays", false),
            flares: options.switch("flares", false),
            reflections: options.switch("reflections", false),
            dim_shadows: options.switch("dim_shadows", false),
        }
    }

    /// The engine's per-eye separation: world units from the centre to each eye, half the eye
    /// distance for `pose` ([`Shared::separation_for`]; before any head pose, an empty one).
    pub fn engine_separation(&self, shared: &Shared, pose: &HeadPose) -> f32 {
        shared.separation_for(pose) * 0.5 * self.units_per_metre
    }
}

#[cfg(test)]
mod tests;
