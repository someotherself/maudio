# A comprehensive Rust audio library powered by miniaudio

### To learn more about miniaudio, check https://miniaud.io/ and https://github.com/mackron/miniaudio

# Miniaudio version
- The crate is currently locked to miniaudio version **0.11.23**

miniaudio does not guarantee a stable ABI, even between minor releases. The same applies to maudio.
See [CHANGELOG](/CHANGELOG.md) for more details.

# Building

### Compiling

By default this crate tries to compile the underlying C library and requires a C compiler on the target machine.

maudio has 2 ways to override this behaviour. 
- `supplybin` - simpler, but not suitable for libraries. Supplies static libraries via **.cargo/config.toml**.
- `external-lib` - requires a **build.rs** file to pass the location to the static libraries to maudio.

For more information see [README](/crates/maudio-sys/README.md) on maudio-sys.

LLVM / Clang
- Required to compile the C library and to generate Rust bindings; on Windows, this means installing LLVM (clang) in addition to Visual Studio.

### Minimum Supported Rust Version (MSRV)
The minimum supported Rust version is **1.70**

### Status of cross-platform compatibility

`maudio` is assumed compatible with all 32 and 64 bit Unix and Windows targets as long as they can pregenerate bindings
and compile the `miniaudio` C library.

The list below contains specific targets that have been tested, and have bindings and static libraries available.

Prebuilt static libraries can be found in the `Release` section on the Github repository.

| Targets | Pregen bindings exist | Tested | Static library exists |
| ------- | ------- | ------- | ------- |
|`x86_64-pc-windows-msvc` <br> `x86_64-pc-windows-gnu` <br> `aarch64-pc-windows-msvc` | Yes | Yes | Yes |
|`x86_64-unknown-linux-gnu` <br> `aarch64-unknown-linux-musl` <br> `aarch64-unknown-linux-gnu` <br> `i686-unknown-linux-gnu` | Yes | Yes | Yes |
| `aarch64-apple-darwin` <br> `x86_64-apple-darwin` | Yes | Yes | Yes |
| `x86_64-unknown-openbsd` <br> `x86_64-unknown-freebsd` <br> `x86_64-unknown-netbsd` <br> `x86_64-unknown-dragonfly` | Yes | Yes | Yes |
| `aarch64-apple-ios` <br> `aarch64-apple-ios-sim` | Yes | No | - |
| `aarch64-linux-android` | Yes | No | - |
| `wasm32-unknown-emscripten` | Yes | Runtime <br>tested* | - |

*requires Emscripten 3.1.68 and nightly for Zbuild.

## Supported audio backends

- WASAPI
- DirectSound
- WinMM
- Core Audio (Apple)
- ALSA
- PulseAudio
- JACK
- sndio (OpenBSD)
- audio(4) (NetBSD and OpenBSD)
- OSS (FreeBSD)
- AAudio (Android 8.0+)
- OpenSL|ES (Android only)
- Web Audio (Emscripten)

## How to use

See [Examples](./crates/maudio/examples/) for a tutorial style introduction into `maudio`

# Description

`maudio` offers access to both the high level audio and low level API's from miniaudio.

While exposing a very easy-to-use interface, the Engine only allows playback and does not support recording, loopback, or duplex operation, and lacks the flexibility and complexity of the low-level API.

## High Level API

The high level API is built around an audio **Engine**.

Despite the functionality contained in the high-level API, most applications only need to manage an Engine and their active Sounds. The engine owns the playback device, resource manager, and node graph, while each sound is automatically connected to the engine’s output. A typical application-level audio system can therefore be as small as:

```rust
struct Audio {
    engine: Engine,
    sounds: HashMap<u64, Sound>,
}
```
This is enough to load or stream sounds, play and pause them, loop or seek, control their volume and spatial properties, and keep them alive for as long as the application needs them. The lower-level components remain available when custom routing, DSP, capture, or direct device control is required.

Under the hood, the engine consists of:
- **ResourceManager**: It is responsible for loading sounds into memory or streaming them. It is also responsible for refence counting them to avoid loading sounds into memory multiple times.
It also has a **Decoder** and can decode audio either before or after it is loaded into memory.
- **NodeGraph**: It is a directed graph of audio processing units called Nodes. Nodes can be audio sources (such as sounds or waveforms), processing units (DSP, filters, splitters), or endpoints. Audio data flows through the graph from source nodes, through optional processing nodes, and finally into the endpoint.
- **Device**: An abstraction of a physical device. Represents the audio playback device and is responsible for driving the engine. Internally, it runs a callback on a dedicated audio thread, which continuously requests (pulls) audio frames from the engine. The engine, in turn, processes the node graph to produce the requested audio data.

By default, a `Sound` created from an Engine is automatically attached to the graph’s endpoint. This means audio is produced and mixed internally by the engine, and the user does not need to manually pull or read audio data. Other source nodes nodes will need to be manually connected.

While simple playback can be achieved without interacting directly with the NodeGraph, more advanced setups allow nodes to be manually connected, reordered, or routed through custom processing chains.

Almost all types in maudio are initialized using a builder pattern, allowing additional configuration at creation time while keeping default usage simple.

## Low Level API

In addition to the high level `Engine` API, `maudio` exposes a low level interface for working directly with the core audio building blocks.
While the high level API provides a ready-to-use playback system, the low level API gives you the components needed to build your own. This includes manual control over devices, audio graphs, data sources, decoding, and resource management.

The two APIs are closely related: the high level engine is built using many of the same concepts exposed by the low level API, but organizes them into a simpler, playback-focused workflow.

The low level API includes:

- **Context** for initializing the audio backend and enumerating devices with stable id.
- **Device** for creating playback, capture, loopback, or duplex streams with direct control over the audio callback.
- **Decoder** for reading audio from encoded formats (mp3, flac, wav and ogg).
- **Encoder** for saving PCM frames into an encoded format (wav supported).
- **Data sources** as a unified interface for producing PCM frames.
- **Audio buffers** for working with decoded PCM data in memory.
- **Utility primitives** such as ring buffers, fences, and notification systems for real-time and asynchronous coordination.

## Custom types
- **Custom nodes**: Allows creating nodes with functionality beyond what already exists in miniaudio. This includes sources, passthrough (for inspecting frames), transformers (for dsp) or resampling nodes
- **Custom audio sources**: Allows exposing any source of PCM frames through miniaudio’s standard data-source interface. Custom sources can support operations such as reading, seeking, looping, and querying their format, and can be used anywhere another data source—such as a decoder—would be accepted.
- **Custom decoders**: Allows using any decoding library to create a decoder that integrates seamlessly with the rest of the library. This can extend the supported formats beyond mp3, flac, wav and ogg (for example adding symphonia to maudio).

Use the low level API when you need full control over how audio is generated, processed, or delivered, or when building abstractions on top of `maudio`.

### Supported (native) PCM formats:
- u8, i16, i24 (3-byte packed LE), i32, and f32.

24-bit audio can be used either as packed 3-byte samples (native) or as i32 (automatic conversion done by maudio, only supported for the high level API).

# Examples using the High Level API

```rust
    let engine = Engine::new().unwrap();
    // A Sound cannot be initialized without an existing engine.
    // However, the other Nodes only need a NodeGraph.
    let sound = engine.new_sound_from_file(&path).unwrap();
    sound.play_sound().unwrap();
    // block thread while music plays
```
Sounds can also be streamed, or fully decoded in memory:
```rust
    let sound = engine.new_sound_from_file_with_flags(&path, SoundFlags::DECODE, None)?;
    let sound = engine.new_sound_from_file_with_flags(&path, SoundFlags::STREAM, None)?;
```

`WaveForm` is a `DataSource`. A data source can be wrapped by a `Sound` or by a `SourceNode`.
A `SourceNode` or any `Node` needs to be piped into the `NodeGraph` manually.

Maudio also comes with a variety of custom nodes with the more common functionalities.

```rust
    let engine = Engine::new().unwrap();
    let waveform = WaveFormBuilder::new(
        2, // channels
        SampleRate::Sr44100,
        WaveFormType::Sine,
        0.8, // amplitude
        200.0, // frequency
    )
    .build_f32() // The engine is f32 natively
    .unwrap();
    let sound = SoundBuilder::new(&engine).data_source(&waveform)
        .start_playing() // equivalent to `sound.play_sound()`
        .build()
        .unwrap();
    thread::sleep(Duration::from_secs(3));
    sound.stop_sound().unwrap();
```

- The endpoint is the final node in a `NodeGraph`. It mixes all the inputs and is where the sound is extracted from the node graph.
- By default, when starting a `Sound`, it is automatically connected to the `endpoint` (unless `SoundFlags::NO_DEFAULT_ATTACHMENT` is used). Other node types will need to be connected manually to the `endpoint` or a `SplitterNode`.
- An `Engine` or `NodeGraph` can give a reference to the `endpoint` simply called a `NodeRef`.
- In the below example, DSP nodes or even a`SplitterNode` can be added in between the source and endpoint.

```rust
    let engine = Engine::new()?;
    let node_graph = engine.as_node_graph();

    // Create a custom node (low pass filter node)
    let lpf = LpfNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, 800.0, 1).build()?;

    // The ENDPOINT
    let end_node = node_graph.endpoint();

    // The SOURCE (sound)
    let source = engine.new_sound_from_file(&path)?;
    let source_node = source.as_node(); // Gets a `NodeRef` to the `Sound`

    // Disconnect the source
    source_node.detach_all_outputs()?;

    // Wire the new node in. LpfNode can pass around as a NodeRef implicitly.
    // attach_output_bus takes in the output bus of the current node and input bus of the upstream node (in this case, the lpf node)
    source_node.attach_output_bus(0, &lpf, 0)?;
    lpf.attach_output_bus(0, &end_node, 0)?;

    source.play_sound()?;
    println!("Stopping in 5 seconds...");
    thread::sleep(Duration::from_secs(5));
    source.stop_sound()?;

    Ok(())
```

Sound files can be embeded into the binary and played using a `Decoder` data source.
```rust
const MUSIC_FILE: &[u8] = include_bytes!("path/to/file.mp3");

fn main() {
    let engine = Engine::new().unwrap();

    let decoder = DecoderBuilder::new_f32()
        .channels(2)
        .sample_rate(SampleRate::Sr44100)
        .from_memory(MUSIC_FILE)
        .unwrap();

    let sound = engine.new_sound_from_source(&decoder).unwrap();
    // Play sound...
}
```
# Examples using the Low Level API

Playback device

A playback device exposes a `&mut out` slice where we pass in pcm frames for playback.

```rust
    let mut decoder = DecoderBuilder::new_f32().channels(2).sample_rate(SampleRate::Sr44100).from_file(&path)?;

    let mut device = DeviceBuilder::playback()
        .f32()
        .playback_channels(2)
        .sample_rate(SampleRate::Sr44100)
        .with_callback(move |_, out| {
            let frames_read = decoder.read_pcm_frames_into(out).unwrap_or(0);

            let samples_read = frames_read * data_format.channels as usize;

            if samples_read < out.len() {
                out[samples_read..].fill(0);
            }
        })?;

    device.device_start()?;
```

Capture device

A playback device exposes a `&input` slice where we pass in pcm frames for playback.

```rust
    let mut encoder = EncoderBuilder::new_f32(2, SampleRate::Sr44100)
        .wav()
        .build_path(&dst_path)?;

    let mut device = DeviceBuilder::capture()
        .f32()
        .capture_channels(2)
        .sample_rate(SampleRate::Sr44100)
        .with_callback(move |_, input| {
              let Ok(_) = encoder.write_pcm_frames(input) else {
                  return;
              };
    })?;

    device.device_start()?;
```

Playback device with a node graph for advanced dsp

```rust
    // When initializing a node graph, we must pick the channels count of the endpoint
    let node_graph = NodeGraphBuilder::new(2).build()?;
    let mut reader = node_graph.try_acquire_reader()?;

    let mut device = DeviceBuilder::playback()
        .f32()
        .playback_channels(2)
        .sample_rate(SampleRate::Sr44100)
        .with_callback(move |_, out| {
            // We read directly from the node_graph's endpoint into the device
            // The node graph always outputs silence if there are no sources
            // and always satisfies the device's requested frame count
            let _ = reader.read_pcm_frames_into(out);
        })?;

    // Use the node_graph to create other nodes such as sources or dsp

    device.device_start()?;
```

Capture device with a node graph 

It is also possible to use a capture device in combination with a Node Graph.
This may be useful when doing dsp, or mixing the capture audio with other sounds.

In this scenario, the capture device is not connected to the endpoint. Instead, it is connected as a source. In this example, we are not adding any dsp, or other source.

A more robust version of this can use the `PcmRingBuffer`.

```rust
    // We will request 1000 frames at a time from the engine
    // For consistency purposes. Can be adjusted.
    let frames: usize = 1000; 

    let node_graph = NodeGraphBuilder::new(channels).build()?;
    let mut reader = node_graph.try_acquire_reader()?;
    let endpoint = node_graph.endpoint();

    // This gives us an audio buffer that does not have a source
    // Later, we can bind it to the input buffer of the device callback
    let buffer_base = AudioBufferBuilder::base_ref_f32(channels, frames as u64)?;

    // Add the AudioBuffer to a Source Node and connect it to the endpoint input bus
    let mut src_node = AttachedSourceNodeBuilder::new(&node_graph, buffer_base).build()?;

    // The source node must live in the callback. Connect it to a splitter and store that for later use
    let splitter = SplitterNodeBuilder::new(&node_graph, channels).build()?;
    src_node.attach_output_bus(0, &splitter, 0)?;
    splitter.attach_output_bus(0, &endpoint, 0)?;

    // The splitter can now be moved and store somewhere if needed

    let mut encoder = EncoderBuilder::new_f32(channels, sample_rate)
        .wav()
        .build_path(path)?;

    // We need an intermediary buffer between the endpoint and the encoder
    // We should make sure we can fit all 1000 frames (2000 samples)
    // Allocations inside the device callback should be avoided
    let mut out_buff = vec![0.0; frames * channels as usize];

    let device = builder
        .capture_channels(channels)
        .sample_rate(sample_rate)
        .period_size_frames(frames as u32)
        .fixed_callback_size(true)
        .with_callback(move |_, input: &[f32]| {
            // Bind the Audio Buffer to the input buffer each time the callback runs
            let Ok(_bound_buffer) = src_node.source_mut().bind(input) else {
                eprintln!("Failed to bind capture buffer");
                return;
            };

            let output = &mut out_buff[..input.len()];

            // Pull frames from the node graph into the temporary buffer
            let frames_read = match reader.read_pcm_frames_into(output) {
                Ok(frames_read) => frames_read,
                Err(err) => {
                    eprintln!("Node graph read failed: {err:?}");
                    return;
                }
            };

            let samples_read = frames_read * channels as usize;

            // Finally, the encode writes to file
            if let Err(err) = encoder.write_pcm_frames(&output[..samples_read]) {
                eprintln!("Encoder write failed: {err:?}");
            }
        })?;

    device.device_start()?;
```