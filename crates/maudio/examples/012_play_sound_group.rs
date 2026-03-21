use std::path::PathBuf;

use maudio::{
    engine::Engine,
    sound::{sound_builder::SoundBuilder, sound_group::SoundGroupBuilder},
    MaResult,
};

// Internally a SoundGroup is simply a Sound, without a data source.
// It is used to connect other Sounds to its input.
// In fact this is was happens when we create a Sound with a SoundGroup.
//
// The SoundGroup node is added as the initial attachment for
// the newly created Sound and starts to act like a DSP node.

fn main() -> MaResult<()> {
    let engine = Engine::new()?;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    // A `SoundGroup` allows multiple sounds to be controlled together.
    //
    // Groups are commonly used to organize sounds into categories such as
    // music, sound effects, or dialogue. Each group has its own volume and
    // effect processing which is applied to every sound attached to it.
    let mut group = SoundGroupBuilder::new(&engine)
        .volume_smooth_millis(250.0)
        .build()?;

    // Changing the group volume affects all sounds in the group.
    group.set_volume(0.5);

    // Create two sounds and attach them to the same group.
    let mut sound1 = SoundBuilder::new(&engine)
        .sound_group(&group)
        .file_path(&path)
        .build()?;

    let mut sound2 = SoundBuilder::new(&engine)
        .sound_group(&group)
        .file_path(&path)
        .build()?;

    // Sounds can still be controlled individually.
    sound1.play_sound()?;
    sound2.play_sound()?;

    std::thread::sleep(std::time::Duration::from_secs(2));

    // Modifying the group affects every sound attached to it.
    // Here we change the volume for both sounds at once.
    group.set_volume(1.0);

    std::thread::sleep(std::time::Duration::from_secs(4));

    // Group operations can also control playback for all sounds.
    group.stop()?;
    Ok(())
}
