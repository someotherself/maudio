#define STB_VORBIS_HEADER_ONLY
#include "miniaudio/extras/stb_vorbis.c"

#define MINIAUDIO_IMPLEMENTATION
#include "miniaudio/miniaudio.h"

#ifdef MAUDIO_ENABLE_VORBIS
    #undef STB_VORBIS_HEADER_ONLY
    #include "miniaudio/extras/stb_vorbis.c"
#endif
