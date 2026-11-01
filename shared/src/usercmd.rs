//! The input command's look angles, as id Tech 5 and 6 build them: once per command the game's
//! usercmd builder adds the keyboard, mouse and stick look input into the player's accumulated
//! view angles (degrees: pitch positive down, yaw positive left), then packs them into the command
//! as shorts. A game's look hooks the builder and, after it, hands the angles to the look and
//! writes back what it returns ([`Commands::rewrite`]), so movement, weapons and the game camera
//! all follow. Each game supplies where its build keeps them ([`Commands`]).

use monaka_hook::mem;

/// Degrees to the command's shorts, as the builders pack them (65536 / 360).
pub const ANGLE_SCALE: f32 = 65536.0 / 360.0;

/// Where a build keeps the command's look angles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commands {
    /// The players' accumulated view angles: the first `accumulator` bytes into the generator, the
    /// next `stride` on, each with its pitch and yaw (f32) at `pitch` and `yaw` bytes in.
    pub accumulator: usize,
    pub stride: usize,
    pub pitch: usize,
    pub yaw: usize,
    /// The command's angles as shorts (pitch, yaw, roll), this many bytes into a command.
    pub angles: usize,
    /// The generator's own copy of the command, this many bytes into the generator.
    pub generator_command: usize,
    /// Players the generator keeps angles for.
    pub players: usize,
}

impl Commands {
    /// After the builder built the command `out` of `player` in `generator`: the player's
    /// accumulated angles handed to `built`, and the angles it returns written back into them and
    /// packed into the command and the generator's copy (`scale`: degrees to shorts, [`packed`]).
    /// Nothing is written when it returns none.
    pub fn rewrite(&self, generator: usize, out: usize, player: usize, scale: f32, built: impl FnOnce(f32, f32) -> Option<(f32, f32)>) {
        if player >= self.players {
            return;
        }
        let angles = generator + self.accumulator + player * self.stride;
        let (Some(pitch), Some(yaw)) = (mem::read::<f32>(angles + self.pitch), mem::read::<f32>(angles + self.yaw)) else {
            return;
        };
        let Some((pitch, yaw)) = built(pitch, yaw) else { return };
        mem::write(angles + self.pitch, pitch);
        mem::write(angles + self.yaw, yaw);
        let pair = packed(pitch, yaw, scale);
        for command in [out, generator + self.generator_command] {
            mem::write(command + self.angles, pair);
        }
    }
}

/// An angle in degrees as the command's short at `scale`, the way the builder packs it (wrapping
/// past 180).
pub fn short(angle: f32, scale: f32) -> i16 {
    (angle * scale) as i32 as i16
}

/// The command's angles: pitch then yaw, adjacent shorts.
pub fn packed(pitch: f32, yaw: f32, scale: f32) -> u32 {
    short(pitch, scale) as u16 as u32 | (short(yaw, scale) as u16 as u32) << 16
}

#[cfg(test)]
mod tests;
