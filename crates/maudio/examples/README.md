-> Basic examples using the High Level API
001_play_sound              - Build an Engine and Sound without a Builder.
002_sound_engine_builder    - Using the EngineBuilder. Explaining MaResult.
003_sound_builder           - Using SoundBuilder. Small intro to thread-safety
004_multiple_sounds         - Start and control of multiple sounds.
005_set_start_stop          - Scheduled start / stop of sound
006_embed_file              - Explaining sound sources, using a Decoder.
007_sound_end_notif         - Intro to simple callbacks. Using the EndNotifier.
008_node_to_graph           - Customize a node graph on an engine.
009_play_waveform           - Further graph customization, multiple sources and dsp.
010_sound_async_fence       - Intro to async. Use a Fence
011_sound_seek              - seek_to_pcm and seek_to_second.
012_play_sound_group        - Create and use a SoundGroup
013_sound_spatialization    - Basic sound spatialization.
014_sound_group_mixing      - Using groups for a game
015_decoder_from_file       - Decode audio from a Read + Seek source (std::io::File)

-> Medium to advanced examples for the High Level Api
101_engine_state_callback   - Use the state notifier callback on an engine.
102_engine_proc_notifier    - Reacting to the engine processing frames.
103_engine_proc_cb          - Using the device proc callback on the engine.
104_VU_meter                - Building a simple VU meter using the proc callback.
105_engine_read_pcm         - Intro into read_pcm frames on an engine.
106_engine_record_to_file   - Record the output of the engine to a file using the encoder
