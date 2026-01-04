#include "miniaudio/miniaudio.h"

#if MA_VERSION_MAJOR != 0 || MA_VERSION_MINOR != 11 || MA_VERSION_REVISION != 23
#error "Unsupported miniaudio version. Expected 0.11.23."
#endif

int miniaudio_version_check(void) { return 0; }