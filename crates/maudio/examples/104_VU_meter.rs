use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use maudio::{engine::engine_builder::EngineBuilder, sound::sound_builder::SoundBuilder, MaResult};

struct VuMeter {
    left: AtomicU32,
    right: AtomicU32,
}

impl VuMeter {
    fn new() -> Self {
        Self {
            left: AtomicU32::new(0.0f32.to_bits()),
            right: AtomicU32::new(0.0f32.to_bits()),
        }
    }

    fn store(&self, left: f32, right: f32) {
        self.left.store(left.to_bits(), Ordering::Relaxed);
        self.right.store(right.to_bits(), Ordering::Relaxed);
    }

    fn load(&self) -> (f32, f32) {
        let left = f32::from_bits(self.left.load(Ordering::Relaxed));
        let right = f32::from_bits(self.right.load(Ordering::Relaxed));
        (left, right)
    }
}

fn meter_bar(level: f32, width: usize) -> String {
    print!("level {}, width {}", level, width);
    let level = level.clamp(0.0, 1.0);
    let filled = (level * width as f32) as usize;

    let mut s = String::with_capacity(width);
    for i in 0..width {
        if i < filled {
            s.push('#');
        } else {
            s.push('-');
        }
    }

    s
}

fn main() -> MaResult<()> {
    let meter = Arc::new(VuMeter::new());
    let meter_cb = Arc::clone(&meter);

    let engine = EngineBuilder::new()
        .set_channels(2)
        .with_realtime_callback(move |samples, channels| {
            if channels == 0 {
                return;
            }

            let mut left_sum = 0.0f32;
            let mut right_sum = 0.0f32;
            let mut frames = 0usize;

            for frame in samples.chunks_exact(channels as usize) {
                let left = frame[0];
                let right = frame[1];

                left_sum += left * left;
                right_sum += right * right;
                frames += 1;
            }

            if frames > 0 {
                let left_rms = (left_sum / frames as f32).sqrt();
                let right_rms = (right_sum / frames as f32).sqrt();

                meter_cb.store(left_rms, right_rms);
            }
        })?;

    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maudio-sys/native/miniaudio/data/16-44100-stereo.flac"
    ));

    let notif = engine.get_data_notifier().unwrap();

    let mut sound = SoundBuilder::new(&engine).file_path(&path).build()?;

    sound.looping();
    sound.play_sound()?;

    for _ in 0..200 {
        let frames_proc = notif.take_delta();
        println!("processed frames: {frames_proc}");

        let (left, right) = meter.load();
        println!("{} - {}", left, right);

        let left_bar = meter_bar(left * 2.5, 30);
        let right_bar = meter_bar(right * 2.5, 30);

        print!("\rL [{}]  R [{}]", left_bar, right_bar);

        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    println!();

    Ok(())
}
