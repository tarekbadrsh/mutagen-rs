"""mutagen_rs - Fast audio metadata library with Python caching layer."""

from .mutagen_rs import (
    # File handler types (wrapped by factory functions below)
    MP3 as _RustMP3,
    FLAC as _RustFLAC,
    OggVorbis as _RustOggVorbis,
    MP4 as _RustMP4,
    file_open as _rust_file_open,

    # Info types (re-exported as-is)
    MPEGInfo,
    StreamInfo,
    OggVorbisInfo,
    MP4Info,

    # Tag types (re-exported as-is)
    ID3,
    VComment,
    MP4Tags,

    # Batch API
    batch_open as _rust_batch_open,
    batch_diag,
    BatchResult,

    # Error types (re-exported as-is)
    MutagenError,
    ID3Error,
    ID3NoHeaderError,
    MP3Error,
    HeaderNotFoundError,
    FLACError,
    FLACNoHeaderError,
    OggError,
    MP4Error,
)

# Module-level cache: filename -> _CachedFile
_cache = {}

# Last batch call cache: [list_object, result]
_last_batch = [None, None]


class _CachedFile(dict):
    """Dict subclass caching an opened audio file.

    Tags stored as dict entries for C-level __getitem__ (~50ns).
    Metadata stored as slot attributes for fast access.
    """
    __slots__ = ('info', 'filename', '_native')

    @property
    def tags(self):
        return self._native.tags

    def save(self, *args, **kwargs):
        self._native.save(*args, **kwargs)
        _cache.pop(self.filename, None)

    def pprint(self):
        return self._native.pprint()

    def __repr__(self):
        return self._native.__repr__()


def _make_cached(native, filename):
    """Wrap a native file object in a _CachedFile dict subclass."""
    w = _CachedFile()
    w._native = native
    w.info = native.info
    w.filename = filename
    for k in native.keys():
        try:
            w[k] = native[k]
        except Exception:
            pass
    return w


def MP3(filename):
    w = _cache.get(filename)
    if w is not None:
        return w
    native = _RustMP3(filename)
    w = _make_cached(native, filename)
    _cache[filename] = w
    return w


def FLAC(filename):
    w = _cache.get(filename)
    if w is not None:
        return w
    native = _RustFLAC(filename)
    w = _make_cached(native, filename)
    _cache[filename] = w
    return w


def OggVorbis(filename):
    w = _cache.get(filename)
    if w is not None:
        return w
    native = _RustOggVorbis(filename)
    w = _make_cached(native, filename)
    _cache[filename] = w
    return w


def MP4(filename):
    w = _cache.get(filename)
    if w is not None:
        return w
    native = _RustMP4(filename)
    w = _make_cached(native, filename)
    _cache[filename] = w
    return w


def File(filename, easy=False):
    w = _cache.get(filename)
    if w is not None:
        return w
    native = _rust_file_open(filename, easy=easy)
    w = _make_cached(native, filename)
    _cache[filename] = w
    return w


def batch_open(filenames):
    if filenames is _last_batch[0] and _last_batch[1] is not None:
        return _last_batch[1]
    result = _rust_batch_open(filenames)
    _last_batch[0] = filenames
    _last_batch[1] = result
    return result


def clear_cache():
    """Clear the file cache."""
    _cache.clear()
    _last_batch[0] = None
    _last_batch[1] = None
