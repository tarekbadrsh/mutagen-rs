use pyo3::create_exception;
use pyo3::exceptions::PyException;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MutagenError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ID3 error: {0}")]
    ID3(String),

    #[error("ID3 no header found")]
    ID3NoHeader,

    #[error("ID3 unsupported version: {0}")]
    ID3UnsupportedVersion(String),

    #[error("ID3 bad unsynch data")]
    ID3BadUnsynchData,

    #[error("ID3 bad compressed data")]
    ID3BadCompressedData,

    #[error("ID3 warning: {0}")]
    ID3Warning(String),

    #[error("MP3 error: {0}")]
    MP3(String),

    #[error("MP3 header not found")]
    HeaderNotFoundError(String),

    #[error("FLAC error: {0}")]
    FLAC(String),

    #[error("FLAC no header found")]
    FLACNoHeader,

    #[error("FLAC vorbis unset error: {0}")]
    FLACVorbisUnset(String),

    #[error("OGG error: {0}")]
    Ogg(String),

    #[error("MP4 error: {0}")]
    MP4(String),

    #[error("MP4 stream info error: {0}")]
    MP4StreamInfo(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Value error: {0}")]
    ValueError(String),
}

// Python exception types matching mutagen's exception hierarchy
create_exception!(mutagen_rs, MutagenPyError, PyException);
create_exception!(mutagen_rs, ID3Error, MutagenPyError);
create_exception!(mutagen_rs, ID3NoHeaderError, ID3Error);
create_exception!(mutagen_rs, ID3UnsupportedVersionError, ID3Error);
create_exception!(mutagen_rs, ID3BadUnsynchData, ID3Error);
create_exception!(mutagen_rs, ID3BadCompressedData, ID3Error);
create_exception!(mutagen_rs, ID3Warning, MutagenPyError);
create_exception!(mutagen_rs, MP3Error, MutagenPyError);
create_exception!(mutagen_rs, HeaderNotFoundError, MP3Error);
create_exception!(mutagen_rs, FLACError, MutagenPyError);
create_exception!(mutagen_rs, FLACNoHeaderError, FLACError);
create_exception!(mutagen_rs, FLACVorbisError, FLACError);
create_exception!(mutagen_rs, OggError, MutagenPyError);
create_exception!(mutagen_rs, MP4Error, MutagenPyError);
create_exception!(mutagen_rs, MP4StreamInfoError, MP4Error);

impl From<MutagenError> for pyo3::PyErr {
    fn from(err: MutagenError) -> pyo3::PyErr {
        match err {
            MutagenError::Io(e) => pyo3::exceptions::PyIOError::new_err(e.to_string()),
            MutagenError::ID3(msg) => ID3Error::new_err(msg),
            MutagenError::ID3NoHeader => ID3NoHeaderError::new_err("No ID3 header found"),
            MutagenError::ID3UnsupportedVersion(msg) => {
                ID3UnsupportedVersionError::new_err(msg)
            }
            MutagenError::ID3BadUnsynchData => {
                self::ID3BadUnsynchData::new_err("Bad unsynch data")
            }
            MutagenError::ID3BadCompressedData => {
                self::ID3BadCompressedData::new_err("Bad compressed data")
            }
            MutagenError::ID3Warning(msg) => self::ID3Warning::new_err(msg),
            MutagenError::MP3(msg) => self::MP3Error::new_err(msg),
            MutagenError::HeaderNotFoundError(msg) => self::HeaderNotFoundError::new_err(msg),
            MutagenError::FLAC(msg) => self::FLACError::new_err(msg),
            MutagenError::FLACNoHeader => self::FLACNoHeaderError::new_err("No FLAC header found"),
            MutagenError::FLACVorbisUnset(msg) => self::FLACVorbisError::new_err(msg),
            MutagenError::Ogg(msg) => self::OggError::new_err(msg),
            MutagenError::MP4(msg) => self::MP4Error::new_err(msg),
            MutagenError::MP4StreamInfo(msg) => self::MP4StreamInfoError::new_err(msg),
            MutagenError::InvalidData(msg) => pyo3::exceptions::PyValueError::new_err(msg),
            MutagenError::Encoding(msg) => pyo3::exceptions::PyValueError::new_err(
                format!("Encoding error: {}", msg),
            ),
            MutagenError::ValueError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        }
    }
}

pub type Result<T> = std::result::Result<T, MutagenError>;
