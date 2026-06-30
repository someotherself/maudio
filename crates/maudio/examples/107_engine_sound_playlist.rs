use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use maudio::{
    engine::Engine,
    sound::{notifier::EndNotifier, sound_builder::SoundBuilder, Sound},
    MaResult,
};

// This example shows a very simple playlist using Sounds as audio sources
//
// The Sound is the easiest Node type to play audio with.
// - it creates and owns the underlying data source for us.
// - they are automatically connected to the node graph
// - they give us access to an `EndNotifier` to poll when a sound has finished playing.

pub enum Command {
    Next,
    Shutdown,
    Add { path: PathBuf },
}

struct PlayList<'a> {
    current: Option<Sound<'a>>,
    current_notifier: Option<EndNotifier>,
    queue: VecDeque<PathBuf>,
}

impl<'a> PlayList<'a> {
    fn new() -> Self {
        Self {
            current: None,
            current_notifier: None,
            queue: VecDeque::new(),
        }
    }

    fn add_sound(&mut self, path: impl AsRef<Path>) {
        self.queue.push_back(path.as_ref().to_path_buf());
    }

    fn play_next(&mut self, engine: &'a Engine) -> MaResult<()> {
        self.current = None;
        self.current_notifier = None;

        let Some(path) = self.queue.pop_front() else {
            return Ok(());
        };

        let (mut sound, notif) = SoundBuilder::new(engine)
            .file_path(&path)
            .with_end_notifier()?;

        sound.play_sound()?;

        self.current = Some(sound);
        self.current_notifier = Some(notif);

        Ok(())
    }

    fn run(&mut self, engine: &'a Engine) -> MaResult<()> {
        if self.current.is_none() {
            self.play_next(engine)?;
        }

        while self.current.is_some() {
            // `take()` returns true once when the current sound reaches the end.
            // It also clears the flag, so the playlist only advances once per end event.
            if self
                .current_notifier
                .as_ref()
                .map_or(false, |notif| notif.take())
            {
                self.play_next(engine)?;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        Ok(())
    }
}

fn main() -> MaResult<()> {
    if cfg!(not(feature = "vorbis")) {
        println!("Run using: cargo run --features vorbis --example 107_engine_sound_playlist");
        return Ok(());
    }
    let path_1 = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));
    let path_2 = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/48000-stereo.ogg"
    ));

    let engine = Engine::new()?;
    // engine.set_volume(0.2)?;
    let mut playlist = PlayList::new();

    playlist.add_sound(path_1);
    playlist.add_sound(path_2);

    playlist.run(&engine)?;

    Ok(())
}
