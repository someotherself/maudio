
# maudio v0.1.12 - 22 Aug 2026 (non breaking)
- methods on Sound change to take &self
- Send, Sync added to Sound
- Send added to DataSource
- docs and examples improved

# maudio v0.1.11 - 18 Aug 2026 (breaking)
- bug fix in DataSource vtable
- Backend feature removed (no-wasapi, no-alsa etc) (breaking)
- (get) cursor_pcm fixed for Decoder

# maudio v0.1.10 - 15 Aug 2026 (non-breaking)
- external-lib feature added

# maudio v0.1.9 - 15 Aug 2026 (breaking)
- adding ability to change channel counts for nodes
- changed for paths are handled on windows / non-windows targets
- MA_NO_NODE_GRAPH and MA_NO_RESOURCE_MANAGER added when no-engine is passed
- resource manager API re-design (breaking)
- Bindgen output file added for iOS. cc / bindgen flags added for iOS
- emscripten output file added for iOS. cc / bindgen flags added for emscripten
- dragonfly, netbds, freebsd, openbsd, i686 linux binding files added
- resampler, channel converted added

# maudio v0.1.8 - 03 Aug 2026 (non-breaking)
- maudio-sys fixed and version bump
