# Miniaudio version
- The crate is currently locked to miniaudio version **0.11.23**

By default, maudio-sys compiles the bundled miniaudio source during the build.

This can be changed by the following feature:
- `supplybin`: Link with a user-supplied, precompiled static miniaudio library.

## Pregenerated bindings

Generation of raw bindings (with Binden) can be forced using the "generate-bindings" feature.
Altough, this is mainly used for development work.

The default branch will try to find and use existing pregenerated bindings, and if they don't exist, will generate them.
Pregenerated bindings exist for the following targets:
- x86_64-unknown-linux-gnu
- x86_64-pc-windows-msvc
- x86_64-pc-windows-gnu
- x86_64-apple-darwin
- aarch64-unknown-linux-musl
- aarch64-unknown-linux-gnu
- aarch64-pc-windows-msvc
- aarch64-apple-darwin

## How to compile and create static miniaudio libraries

The supplybin feature can be used when you want to compile miniaudio separately instead of requiring maudio-sys to compile it during the Cargo build.

This allows crates that depend on maudio to build without requiring a native C compiler.

The supplied libraries must:

- be supplied separately for each targeted operating system, architecture, and toolchain
- use the same version of miniaudio.h as maudio-sys. miniaudio does not guarantee a stable ABI even between minor releases;
- be compiled with configuration options compatible with the enabled maudio features (example, vobis);
- be named according to the conventions of the target toolchain.

Static libraries are target-specific. For example, a library built for x86_64-unknown-linux-gnu cannot be used with x86_64-pc-windows-msvc.

### Create the implementation file

Create a file named miniaudio.c. Recommended content:

```C
#define STB_VORBIS_HEADER_ONLY
#include "miniaudio/extras/stb_vorbis.c"

#define MINIAUDIO_IMPLEMENTATION
#include "miniaudio/miniaudio.h"

#ifdef MAUDIO_ENABLE_VORBIS
    #undef STB_VORBIS_HEADER_ONLY
    #include "miniaudio/extras/stb_vorbis.c"
#endif
```

The following example covers common 64-bit Linux, Windows, and macOS targets. It is not comprehensive.
Other targets can be used by creating a directory whose name exactly matches Cargo’s target triple and supplying a compatible library.
See Rust [Platform Support](https://doc.rust-lang.org/rustc/platform-support.html) domentation for more info.

The folder structure should follow this structure:
```text
native-libs/
└── miniaudio-0.11.23
    ├── x86_64-unknown-linux-gnu/
    ├── aarch64-unknown-linux-gnu/
    ├── x86_64-pc-windows-msvc/
    ├── aarch64-pc-windows-msvc/
    ├── x86_64-pc-windows-gnu/
    ├── x86_64-apple-darwin/
    └── aarch64-apple-darwin/
```

Inside the target folder, maudio expects a folder specific for the current miniaudio version, in the format `miniaudio-{version}`. The libraries must be updated when the miniaudio version changes in maudio, and this folder structure is meant to enforce that.

You can create the folder structure above using:
```bash
mkdir -p native-libs/miniaudio-0.11.23/{x86_64-unknown-linux-gnu,aarch64-unknown-linux-gnu,x86_64-pc-windows-msvc,aarch64-pc-windows-msvc,x86_64-pc-windows-gnu,x86_64-apple-darwin,aarch64-apple-darwin}
```
### Linux

Compile the source into an object file and place it in a static archive:

```bash
gcc -std=c99 -O2 -fPIC \
    -DMA_NO_VORBIS=1 \
    -c miniaudio.c \
    -o miniaudio.o

ar rcs libminiaudio.a miniaudio.o
```

The resulting library is: `libminiaudio.a`

You can inspect the archive with:
```bash
ar t libminiaudio.a
nm --defined-only libminiaudio.a | grep ' ma_'
```
### Windows with MSVC

Open an x64 Native Tools Command Prompt for Visual Studio and run:

```bash
cl /nologo /O2 /DMA_NO_VORBIS=1 /O2 /c miniaudio.c /Fominiaudio.obj
lib /nologo /OUT:miniaudio.lib miniaudio.obj
```
The resulting miniaudio.lib is suitable for the x86_64-pc-windows-msvc Rust target.

### Windows with MinGW-w64

From a MinGW-w64 environment, run:
```bash

x86_64-w64-mingw32-gcc -std=c00 -O2 \
    -DMA_NO_VORBIS=1 \
    -c miniaudio.c \
    -o miniaudio.o

x86_64-w64-mingw32-ar rcs libminiaudio.a miniaudio.o
```

This library is suitable for the corresponding Windows GNU Rust target, such as x86_64-pc-windows-gnu. It is not interchangeable with the MSVC library.

### macOS

Compile the source with Clang and create the archive with libtool:
```bash
clang -std=c00 -O2 \
    -DMA_NO_VORBIS=1 \
    -c miniaudio.c \
    -o miniaudio.o

libtool -static \
    -o libminiaudio.a \
    miniaudio.o
```
Build separate libraries for x86_64-apple-darwin and aarch64-apple-darwin, unless you deliberately create a universal library containing both architectures.

### Vorbis support

To build with vorbis support, replace the no-vorbis definition with the wrapper definition used by maudio-sys. 
Replace:
DMA_NO_VORBIS=1
with:
DMAUDIO_ENABLE_VORBIS=1

### Configure Cargo

Enable the supplybin feature:

[dependencies]
```toml
[dependencies]
maudio = { version = "", features = ["supplybin"] }
```
Then create .cargo/config.toml in the project or workspace root:
```toml
[env]
MAUDIO_EXTERNAL_LIB_DIR = {
    value = ".../native-libs/",
    relative = true
}
```
MAUDIO_EXTERNAL_LIB_DIR must point to the root directory containing the target-specific subdirectories.
During the build, maudio-sys selects the subdirectory matching Cargo’s current target.

The expected filename depends on the target:

| Target toolchain	| Expected filename |
| ------|-------------|
| Linux	| libminiaudio.a |
| macOS	| libminiaudio.a |
| Windows GNU	| libminiaudio.a |
| Windows MSVC	| miniaudio.lib |

The library name itself is fixed. MAUDIO_EXTERNAL_LIB_DIR should contain only the directory, not the complete path to the library file.