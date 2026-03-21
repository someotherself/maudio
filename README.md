# Rust bindings to the miniaudio library

### To learn more about miniaudio, check https://miniaud.io/ and https://github.com/mackron/miniaudio

# Miniaudio version
- The crate is currently locked to miniaudio version **0.11.23**

# Building

### Compiling
To build this crate, the underlying C library must be compiled (and optionally bindings generated).

LLVM / Clang
- Required to compile the C library and to generate Rust bindings; on Windows, this means installing LLVM (clang) in addition to Visual Studio.

### Minimum Supported Rust Version (MSRV)
The minimum supported Rust version depends on how the crate is built.
- Normal build (default, without --generate-bindings): **1.64**
- Generating bindings (with --generate-bindings enabled): **1.70**

### Platforms / Bindings
- Building and testing has only been done on Windows/Linux/MacOS. While miniaudio offers compatibility with Windows, macOS, Linux, BSD, iOS, Android and Web, more testing is needed to ensure `maudio` compatibility with all of them.
- Pre-generated bindings exist for Windows and Linux.
- On MacOS, `--generate-bindings` feature must be used for now.

## How to use

See [Examples](./crates/maudio/examples/) for a tutorial style introduction into `maudio`

# Description

Currently, `maudio` offers a high level audio interface accessed via an Engine.

While exposing a very easy-to-use interface, the Engine only allows playback and does not support recording, loopback, or duplex operation, and lacks the flexibility and complexity of the low-level API.

## High Level API

The high level API is built around an audio **Engine**. Under the hood, the engine consists of:
- **ResourceManager**: It is responsible for loading sounds into memory or streaming them. It is also responsible for refence counting them to avoid loading sounds into memory multiple times.
It also has a **Decoder** and can decode audio either before or after it is loaded into memory.
- **NodeGraph**: It is a directed graph of audio processing units called Nodes. Nodes can be audio sources (such as sounds or waveforms), processing units (DSP, filters, splitters), or endpoints. Audio data flows through the graph from source nodes, through optional processing nodes, and finally into the endpoint.
- **Device**: An abstraction of a physical device. Represents the audio playback device and is responsible for driving the engine. Internally, it runs a callback on a dedicated audio thread, which continuously requests (pulls) audio frames from the engine. The engine, in turn, processes the node graph to produce the requested audio data.

By default, sounds created from an Engine are automatically attached to the graph’s endpoint and played in a push-based manner. This means audio is produced and mixed internally by the engine, and the user does not need to manually pull or read audio data.

While simple playback can be achieved without interacting directly with the NodeGraph, more advanced setups allow nodes to be manually connected, reordered, or routed through custom processing chains.

Almost all types in maudio are initialized using a builder pattern, allowing additional configuration at creation time while keeping default usage simple.

## Low Level API

In addition to the high level `Engine` API, `maudio` exposes a low level interface for working directly with the core audio building blocks.
While the high level API provides a ready-to-use playback system, the low level API gives you the components needed to build your own. This includes manual control over devices, audio graphs, data sources, decoding, and resource management.

The two APIs are closely related: the high level engine is built using many of the same concepts exposed by the low level API, but organizes them into a simpler, playback-focused workflow.

The low level API includes:

- **Context** for initializing the audio backend and enumerating devices.
- **Device** for creating playback, capture, loopback, or duplex streams with direct control over the audio callback.
- **Decoder** for reading audio from encoded formats.
- **Data sources** as a unified interface for producing PCM frames.
- **Audio buffers** for working with decoded PCM data in memory.
- **Utility primitives** such as ring buffers, fences, and notification systems for real-time and asynchronous coordination.

Use the low level API when you need full control over how audio is generated, processed, or delivered, or when building abstractions on top of `maudio`.

### Supported (native) PCM formats:
- u8, i16, i24 (3-byte packed LE), i32, and f32.

24-bit audio can be used either as packed 3-byte samples (native) or as i32 (automatic conversion done by maudio, only supported for the high level API).

# Examples using the High Level API

```rust
    let engine = Engine::new().unwrap();
    // A Sound cannot be initialized without an existing engine.
    // However, the other Nodes only need a NodeGraph.
    let mut sound = engine.new_sound_from_file(&path).unwrap();
    sound.play_sound().unwrap();
    // block thread while music plays
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
    let mut sound = SoundBuilder::new(&engine).data_source(&waveform)
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
    let node_graph = engine.as_node_graph().unwrap();

    // Create a custom node (low pass filter node)
    let mut lpf = LpfNodeBuilder::new(&node_graph, 2, SampleRate::Sr48000, 800.0, 1).build()?;

    // The ENDPOINT
    let mut end_node = node_graph.endpoint().unwrap();

    // The SOURCE (sound)
    let mut source = engine.new_sound_from_file(&path)?;
    let mut source_node = source.as_node(); // Gets a `NodeRef` to the `Sound`

    // Disconnect the source
    source_node.detach_all_outputs()?;

    // Wire the new node in. LpfNode can pass around as a NodeRef implicitly.
    // attach_output_bus takes in the output bus of the current node and input bus of the upstream node (in this case, the lpf node)
    source_node.attach_output_bus(0, &mut lpf, 0)?;
    lpf.attach_output_bus(0, &mut end_node, 0)?;

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

    let decoder = DecoderBuilder::new(Format::F32, 2, SampleRate::Sr44100)
        .ref_from_memory(MUSIC_FILE)
        .unwrap();

    let mut sound = engine.new_sound_from_source(&decoder).unwrap();
    // Play sound...
}
```