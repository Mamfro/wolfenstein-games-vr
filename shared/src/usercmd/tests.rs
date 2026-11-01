use super::*;

#[test]
fn shorts_wrap_like_the_game() {
    assert_eq!(short(90.0, ANGLE_SCALE), 16384);
    assert_eq!(short(-90.0, ANGLE_SCALE), -16384);
    assert_eq!(short(270.0, ANGLE_SCALE), -16384, "past 180 wraps to negative");
}

#[test]
fn angles_pack_pitch_then_yaw_as_the_builder_does() {
    // 90 degrees is 0x4000; -90 wraps to 0xc000.
    assert_eq!(packed(90.0, -90.0, ANGLE_SCALE), 0xc000_4000);
    assert_eq!(packed(0.0, 0.0, ANGLE_SCALE), 0);
}

#[test]
fn the_rewrite_writes_the_looks_angles_back_and_packs_them_twice() {
    // A generator of two players, the command outside it.
    let layout = Commands { accumulator: 0xc, stride: 0x20, pitch: 4, yaw: 8, angles: 0x6, generator_command: 0x50, players: 2 };
    let mut generator = [0u8; 0x60];
    let mut command = [0u8; 0x10];
    let (base, out) = (generator.as_mut_ptr() as usize, command.as_mut_ptr() as usize);
    let angles = base + 0xc + 0x20;
    mem::write(angles + 4, 10.0f32);
    mem::write(angles + 8, -20.0f32);
    let mut seen = None;
    layout.rewrite(base, out, 1, ANGLE_SCALE, |pitch, yaw| {
        seen = Some((pitch, yaw));
        Some((pitch + 80.0, yaw - 70.0))
    });
    assert_eq!(seen, Some((10.0, -20.0)), "the look got player 1's angles");
    assert_eq!((mem::read::<f32>(angles + 4), mem::read::<f32>(angles + 8)), (Some(90.0), Some(-90.0)));
    assert_eq!(mem::read::<u32>(out + 6), Some(0xc000_4000), "the command's shorts");
    assert_eq!(mem::read::<u32>(base + 0x50 + 6), Some(0xc000_4000), "the generator's copy");
    // A player past the generator's, or a look that returns none, writes nothing.
    layout.rewrite(base, out, 2, ANGLE_SCALE, |_, _| panic!("no such player"));
    layout.rewrite(base, out, 1, ANGLE_SCALE, |_, _| None);
    assert_eq!(mem::read::<f32>(angles + 4), Some(90.0));
}
