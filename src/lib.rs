pub mod common;
pub mod id3;
pub mod mp3;
pub mod flac;
pub mod ogg;
pub mod mp4;
pub mod vorbis;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;


#[cfg(feature = "python")]
mod python_bindings {
use super::*;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyBytes, PyTuple};
use pyo3::exceptions::{PyValueError, PyKeyError, PyIOError};

// ---- Python Classes ----

#[pyclass(name = "MPEGInfo")]
#[derive(Debug, Clone)]
struct PyMPEGInfo {
    #[pyo3(get)]
    length: f64,
    #[pyo3(get)]
    channels: u32,
    #[pyo3(get)]
    bitrate: u32,
    #[pyo3(get)]
    sample_rate: u32,
    #[pyo3(get)]
    version: f64,
    #[pyo3(get)]
    layer: u8,
    #[pyo3(get)]
    mode: u32,
    #[pyo3(get)]
    protected: bool,
    #[pyo3(get)]
    bitrate_mode: u8,
    #[pyo3(get)]
    encoder_info: String,
    #[pyo3(get)]
    encoder_settings: String,
    #[pyo3(get)]
    track_gain: Option<f32>,
    #[pyo3(get)]
    track_peak: Option<f32>,
    #[pyo3(get)]
    album_gain: Option<f32>,
}

#[pymethods]
impl PyMPEGInfo {
    fn __repr__(&self) -> String {
        format!(
            "MPEGInfo(length={:.2}, bitrate={}, sample_rate={}, channels={}, version={}, layer={})",
            self.length, self.bitrate, self.sample_rate, self.channels, self.version, self.layer
        )
    }

    fn pprint(&self) -> String {
        format!(
            "MPEG {} layer {} {:.2} seconds, {} bps, {} Hz",
            self.version, self.layer, self.length, self.bitrate, self.sample_rate
        )
    }
}

/// ID3 tag container.
#[pyclass(name = "ID3")]
#[derive(Debug)]
struct PyID3 {
    tags: id3::tags::ID3Tags,
    path: Option<String>,
    version: (u8, u8),
}

#[pymethods]
impl PyID3 {
    #[new]
    #[pyo3(signature = (filename=None))]
    fn new(filename: Option<&str>) -> PyResult<Self> {
        match filename {
            Some(path) => {
                let (tags, header) = id3::load_id3(path)?;
                let version = header.as_ref().map(|h| h.version).unwrap_or((4, 0));
                Ok(PyID3 {
                    tags,
                    path: Some(path.to_string()),
                    version,
                })
            }
            None => Ok(PyID3 {
                tags: id3::tags::ID3Tags::new(),
                path: None,
                version: (4, 0),
            }),
        }
    }

    fn getall(&self, key: &str) -> PyResult<Vec<PyObject>> {
        Python::with_gil(|py| {
            let frames = self.tags.getall(key);
            Ok(frames.iter().map(|f| frame_to_py(py, f)).collect())
        })
    }

    fn keys(&self) -> Vec<String> {
        self.tags.keys()
    }

    fn values(&self, py: Python) -> Vec<PyObject> {
        self.tags.values().iter().map(|f| frame_to_py(py, f)).collect()
    }

    fn __getitem__(&mut self, py: Python, key: &str) -> PyResult<PyObject> {
        match self.tags.get_mut(key) {
            Some(frame) => Ok(frame_to_py(py, frame)),
            None => Err(PyKeyError::new_err(key.to_string())),
        }
    }

    fn __setitem__(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let text = value.extract::<Vec<String>>().or_else(|_| {
            value.extract::<String>().map(|s| vec![s])
        })?;

        let frame = id3::frames::Frame::Text(id3::frames::TextFrame {
            id: key.to_string(),
            encoding: id3::specs::Encoding::Utf8,
            text,
        });

        let hash_key = frame.hash_key();
        self.tags.frames.insert(hash_key, vec![id3::tags::LazyFrame::Decoded(frame)]);
        Ok(())
    }

    fn __delitem__(&mut self, key: &str) -> PyResult<()> {
        self.tags.delall(key);
        Ok(())
    }

    fn __contains__(&self, key: &str) -> bool {
        self.tags.get(key).is_some()
    }

    fn __len__(&self) -> usize {
        self.tags.len()
    }

    fn __repr__(&self) -> String {
        format!("ID3(keys={})", self.tags.keys().join(", "))
    }

    fn __iter__(&self, py: Python) -> PyResult<PyObject> {
        let keys = self.tags.keys();
        let list = PyList::new(py, &keys)?;
        Ok(list.call_method0("__iter__")?.into())
    }

    fn save(&self, filename: Option<&str>) -> PyResult<()> {
        let path = filename
            .map(|s| s.to_string())
            .or_else(|| self.path.clone())
            .ok_or_else(|| PyValueError::new_err("No filename specified"))?;

        id3::save_id3(&path, &self.tags, self.version.0.max(3))?;
        Ok(())
    }

    fn delete(&self, filename: Option<&str>) -> PyResult<()> {
        let path = filename
            .map(|s| s.to_string())
            .or_else(|| self.path.clone())
            .ok_or_else(|| PyValueError::new_err("No filename specified"))?;

        id3::delete_id3(&path)?;
        Ok(())
    }

    fn pprint(&self) -> String {
        let mut parts = Vec::new();
        for frame in self.tags.values() {
            parts.push(format!("{}={}", frame.frame_id(), frame.pprint()));
        }
        parts.join("\n")
    }

    #[getter]
    fn version(&self) -> (u8, u8) {
        self.version
    }
}

/// MP3 file (ID3 tags + audio info).
#[pyclass(name = "MP3")]
#[derive(Debug)]
struct PyMP3 {
    #[pyo3(get)]
    info: PyMPEGInfo,
    #[pyo3(get)]
    filename: String,
    id3: PyID3,
}

impl PyMP3 {
    #[inline(always)]
    fn from_data(data: &[u8], filename: &str) -> PyResult<Self> {
        let mp3_file = mp3::MP3File::parse(data, filename)?;
        let info = make_mpeg_info(&mp3_file.info);
        let version = mp3_file.id3_header.as_ref().map(|h| h.version).unwrap_or((4, 0));

        Ok(PyMP3 {
            info,
            filename: filename.to_string(),
            id3: PyID3 {
                tags: mp3_file.tags,
                path: Some(filename.to_string()),
                version,
            },
        })
    }
}

#[pymethods]
impl PyMP3 {
    #[new]
    fn new(filename: &str) -> PyResult<Self> {
        let data = std::fs::read(filename)
            .map_err(|e| PyIOError::new_err(format!("{}", e)))?;
        Self::from_data(&data, filename)
    }

    #[getter]
    fn tags(&self, py: Python) -> PyResult<PyObject> {
        let id3 = PyID3 {
            tags: self.id3.tags.clone(),
            path: self.id3.path.clone(),
            version: self.id3.version,
        };
        Ok(id3.into_pyobject(py)?.into_any().unbind())
    }

    fn keys(&self) -> Vec<String> {
        self.id3.tags.keys()
    }

    #[inline(always)]
    fn __getitem__(&mut self, py: Python, key: &str) -> PyResult<PyObject> {
        match self.id3.tags.get_mut(key) {
            Some(frame) => Ok(frame_to_py(py, frame)),
            None => Err(PyKeyError::new_err(key.to_string())),
        }
    }

    fn __contains__(&self, key: &str) -> bool {
        self.id3.tags.get(key).is_some()
    }

    fn __repr__(&self) -> String {
        format!("MP3(filename={:?})", self.filename)
    }

    fn save(&self) -> PyResult<()> {
        self.id3.save(Some(&self.filename))
    }

    fn pprint(&self) -> String {
        format!("{}\n{}", self.info.pprint(), self.id3.pprint())
    }
}

/// FLAC stream info.
#[pyclass(name = "StreamInfo")]
#[derive(Debug, Clone)]
struct PyStreamInfo {
    #[pyo3(get)]
    length: f64,
    #[pyo3(get)]
    channels: u8,
    #[pyo3(get)]
    sample_rate: u32,
    #[pyo3(get)]
    bits_per_sample: u8,
    #[pyo3(get)]
    total_samples: u64,
    #[pyo3(get)]
    min_block_size: u16,
    #[pyo3(get)]
    max_block_size: u16,
    #[pyo3(get)]
    min_frame_size: u32,
    #[pyo3(get)]
    max_frame_size: u32,
}

#[pymethods]
impl PyStreamInfo {
    fn __repr__(&self) -> String {
        format!(
            "StreamInfo(length={:.2}, sample_rate={}, channels={}, bits_per_sample={})",
            self.length, self.sample_rate, self.channels, self.bits_per_sample
        )
    }

    fn pprint(&self) -> String {
        format!(
            "FLAC, {:.2} seconds, {} Hz",
            self.length, self.sample_rate
        )
    }

    #[getter]
    fn bitrate(&self) -> u32 {
        self.bits_per_sample as u32 * self.sample_rate * self.channels as u32
    }
}

/// VorbisComment-based tags (used by FLAC and OGG).
#[pyclass(name = "VComment")]
#[derive(Debug, Clone)]
struct PyVComment {
    vc: vorbis::VorbisComment,
    path: Option<String>,
}

#[pymethods]
impl PyVComment {
    fn keys(&self) -> Vec<String> {
        self.vc.keys()
    }

    #[inline(always)]
    fn __getitem__(&self, py: Python, key: &str) -> PyResult<PyObject> {
        let values = self.vc.get(key);
        if values.is_empty() {
            return Err(PyKeyError::new_err(key.to_string()));
        }
        Ok(PyList::new(py, values)?.into_any().unbind())
    }

    fn __setitem__(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let values = value.extract::<Vec<String>>().or_else(|_| {
            value.extract::<String>().map(|s| vec![s])
        })?;
        self.vc.set(key, values);
        Ok(())
    }

    fn __delitem__(&mut self, key: &str) -> PyResult<()> {
        self.vc.delete(key);
        Ok(())
    }

    fn __contains__(&self, key: &str) -> bool {
        !self.vc.get(key).is_empty()
    }

    fn __len__(&self) -> usize {
        self.vc.keys().len()
    }

    fn __iter__(&self, py: Python) -> PyResult<PyObject> {
        let keys = self.vc.keys();
        let list = PyList::new(py, &keys)?;
        Ok(list.call_method0("__iter__")?.into())
    }

    fn __repr__(&self) -> String {
        format!("VComment(keys={})", self.vc.keys().join(", "))
    }

    #[getter]
    fn vendor(&self) -> &str {
        &self.vc.vendor
    }
}

/// FLAC file.
#[pyclass(name = "FLAC")]
struct PyFLAC {
    #[pyo3(get)]
    info: PyStreamInfo,
    #[pyo3(get)]
    filename: String,
    flac_file: flac::FLACFile,
    vc_data: vorbis::VorbisComment,
}

impl PyFLAC {
    #[inline(always)]
    fn from_data(data: &[u8], filename: &str) -> PyResult<Self> {
        let flac_file = flac::FLACFile::parse(data, filename)?;

        let info = PyStreamInfo {
            length: flac_file.info.length,
            channels: flac_file.info.channels,
            sample_rate: flac_file.info.sample_rate,
            bits_per_sample: flac_file.info.bits_per_sample,
            total_samples: flac_file.info.total_samples,
            min_block_size: flac_file.info.min_block_size,
            max_block_size: flac_file.info.max_block_size,
            min_frame_size: flac_file.info.min_frame_size,
            max_frame_size: flac_file.info.max_frame_size,
        };

        let vc_data = flac_file.tags.clone().unwrap_or_else(|| vorbis::VorbisComment::new());

        Ok(PyFLAC {
            info,
            filename: filename.to_string(),
            flac_file,
            vc_data,
        })
    }
}

#[pymethods]
impl PyFLAC {
    #[new]
    fn new(filename: &str) -> PyResult<Self> {
        let data = std::fs::read(filename)
            .map_err(|e| PyIOError::new_err(format!("{}", e)))?;
        Self::from_data(&data, filename)
    }

    #[getter]
    fn tags(&self, py: Python) -> PyResult<PyObject> {
        let vc = self.vc_data.clone();
        let pvc = PyVComment { vc, path: Some(self.filename.clone()) };
        Ok(pvc.into_pyobject(py)?.into_any().unbind())
    }

    fn keys(&self) -> Vec<String> {
        self.vc_data.keys()
    }

    #[inline(always)]
    fn __getitem__(&self, py: Python, key: &str) -> PyResult<PyObject> {
        let values = self.vc_data.get(key);
        if values.is_empty() {
            return Err(PyKeyError::new_err(key.to_string()));
        }
        Ok(PyList::new(py, values)?.into_any().unbind())
    }

    fn __contains__(&self, key: &str) -> bool {
        !self.vc_data.get(key).is_empty()
    }

    fn __repr__(&self) -> String {
        format!("FLAC(filename={:?})", self.filename)
    }

    fn save(&self) -> PyResult<()> {
        self.flac_file.save()?;
        Ok(())
    }
}

/// OGG Vorbis info.
#[pyclass(name = "OggVorbisInfo")]
#[derive(Debug, Clone)]
struct PyOggVorbisInfo {
    #[pyo3(get)]
    length: f64,
    #[pyo3(get)]
    channels: u8,
    #[pyo3(get)]
    sample_rate: u32,
    #[pyo3(get)]
    bitrate: u32,
}

#[pymethods]
impl PyOggVorbisInfo {
    fn __repr__(&self) -> String {
        format!(
            "OggVorbisInfo(length={:.2}, sample_rate={}, channels={})",
            self.length, self.sample_rate, self.channels
        )
    }

    fn pprint(&self) -> String {
        format!(
            "Ogg Vorbis, {:.2} seconds, {} Hz",
            self.length, self.sample_rate
        )
    }
}

/// OGG Vorbis file.
#[pyclass(name = "OggVorbis")]
#[derive(Debug)]
struct PyOggVorbis {
    #[pyo3(get)]
    info: PyOggVorbisInfo,
    #[pyo3(get)]
    filename: String,
    vc: PyVComment,
}

impl PyOggVorbis {
    #[inline(always)]
    fn from_data(data: &[u8], filename: &str) -> PyResult<Self> {
        let ogg_file = ogg::OggVorbisFile::parse(data, filename)?;

        let info = PyOggVorbisInfo {
            length: ogg_file.info.length,
            channels: ogg_file.info.channels,
            sample_rate: ogg_file.info.sample_rate,
            bitrate: ogg_file.info.bitrate,
        };

        let vc = PyVComment {
            vc: ogg_file.tags,
            path: Some(filename.to_string()),
        };

        Ok(PyOggVorbis {
            info,
            filename: filename.to_string(),
            vc,
        })
    }
}

#[pymethods]
impl PyOggVorbis {
    #[new]
    fn new(filename: &str) -> PyResult<Self> {
        let data = std::fs::read(filename)
            .map_err(|e| PyIOError::new_err(format!("{}", e)))?;
        Self::from_data(&data, filename)
    }

    #[getter]
    fn tags(&self, py: Python) -> PyResult<PyObject> {
        let vc = self.vc.clone();
        Ok(vc.into_pyobject(py)?.into_any().unbind())
    }

    fn keys(&self) -> Vec<String> {
        self.vc.vc.keys()
    }

    #[inline(always)]
    fn __getitem__(&self, py: Python, key: &str) -> PyResult<PyObject> {
        let values = self.vc.vc.get(key);
        if values.is_empty() {
            return Err(PyKeyError::new_err(key.to_string()));
        }
        Ok(PyList::new(py, values)?.into_any().unbind())
    }

    fn __contains__(&self, key: &str) -> bool {
        !self.vc.vc.get(key).is_empty()
    }

    fn __repr__(&self) -> String {
        format!("OggVorbis(filename={:?})", self.filename)
    }

    fn save(&self) -> PyResult<()> {
        Err(PyValueError::new_err("OGG write support is limited"))
    }
}

/// MP4 file info.
#[pyclass(name = "MP4Info")]
#[derive(Debug, Clone)]
struct PyMP4Info {
    #[pyo3(get)]
    length: f64,
    #[pyo3(get)]
    channels: u32,
    #[pyo3(get)]
    sample_rate: u32,
    #[pyo3(get)]
    bitrate: u32,
    #[pyo3(get)]
    bits_per_sample: u32,
    #[pyo3(get)]
    codec: String,
    #[pyo3(get)]
    codec_description: String,
}

#[pymethods]
impl PyMP4Info {
    fn __repr__(&self) -> String {
        format!(
            "MP4Info(length={:.2}, codec={}, channels={}, sample_rate={})",
            self.length, self.codec, self.channels, self.sample_rate
        )
    }

    fn pprint(&self) -> String {
        format!(
            "MPEG-4 audio ({}), {:.2} seconds, {} bps",
            self.codec, self.length, self.bitrate
        )
    }
}

/// MP4 tags.
#[pyclass(name = "MP4Tags")]
#[derive(Debug, Clone)]
struct PyMP4Tags {
    tags: mp4::MP4Tags,
}

#[pymethods]
impl PyMP4Tags {
    fn keys(&self) -> Vec<String> {
        self.tags.keys()
    }

    fn __getitem__(&self, py: Python, key: &str) -> PyResult<PyObject> {
        match self.tags.get(key) {
            Some(value) => mp4_value_to_py(py, value),
            None => Err(PyKeyError::new_err(key.to_string())),
        }
    }

    fn __contains__(&self, key: &str) -> bool {
        self.tags.items.contains_key(key)
    }

    fn __len__(&self) -> usize {
        self.tags.items.len()
    }

    fn __iter__(&self, py: Python) -> PyResult<PyObject> {
        let keys = self.tags.keys();
        let list = PyList::new(py, &keys)?;
        Ok(list.call_method0("__iter__")?.into())
    }

    fn __repr__(&self) -> String {
        format!("MP4Tags(keys={})", self.tags.keys().join(", "))
    }
}

/// MP4 file.
#[pyclass(name = "MP4")]
#[derive(Debug)]
struct PyMP4 {
    #[pyo3(get)]
    info: PyMP4Info,
    #[pyo3(get)]
    filename: String,
    mp4_tags: PyMP4Tags,
}

impl PyMP4 {
    #[inline(always)]
    fn from_data(data: &[u8], filename: &str) -> PyResult<Self> {
        let mp4_file = mp4::MP4File::parse(data, filename)?;

        let info = PyMP4Info {
            length: mp4_file.info.length,
            channels: mp4_file.info.channels,
            sample_rate: mp4_file.info.sample_rate,
            bitrate: mp4_file.info.bitrate,
            bits_per_sample: mp4_file.info.bits_per_sample,
            codec: mp4_file.info.codec,
            codec_description: mp4_file.info.codec_description,
        };

        let mp4_tags = PyMP4Tags {
            tags: mp4_file.tags,
        };

        Ok(PyMP4 {
            info,
            filename: filename.to_string(),
            mp4_tags,
        })
    }
}

#[pymethods]
impl PyMP4 {
    #[new]
    fn new(filename: &str) -> PyResult<Self> {
        let data = std::fs::read(filename)
            .map_err(|e| PyIOError::new_err(format!("{}", e)))?;
        Self::from_data(&data, filename)
    }

    #[getter]
    fn tags(&self, py: Python) -> PyResult<PyObject> {
        let tags = self.mp4_tags.clone();
        Ok(tags.into_pyobject(py)?.into_any().unbind())
    }

    fn keys(&self) -> Vec<String> {
        self.mp4_tags.tags.keys()
    }

    #[inline(always)]
    fn __getitem__(&self, py: Python, key: &str) -> PyResult<PyObject> {
        match self.mp4_tags.tags.get(key) {
            Some(value) => mp4_value_to_py(py, value),
            None => Err(PyKeyError::new_err(key.to_string())),
        }
    }

    fn __contains__(&self, key: &str) -> bool {
        self.mp4_tags.tags.items.contains_key(key)
    }

    fn __repr__(&self) -> String {
        format!("MP4(filename={:?})", self.filename)
    }
}

// ---- Helper functions ----

#[inline(always)]
fn make_mpeg_info(info: &mp3::MPEGInfo) -> PyMPEGInfo {
    PyMPEGInfo {
        length: info.length,
        channels: info.channels,
        bitrate: info.bitrate,
        sample_rate: info.sample_rate,
        version: info.version,
        layer: info.layer,
        mode: info.mode,
        protected: info.protected,
        bitrate_mode: match info.bitrate_mode {
            mp3::xing::BitrateMode::Unknown => 0,
            mp3::xing::BitrateMode::CBR => 1,
            mp3::xing::BitrateMode::VBR => 2,
            mp3::xing::BitrateMode::ABR => 3,
        },
        encoder_info: info.encoder_info.clone(),
        encoder_settings: info.encoder_settings.clone(),
        track_gain: info.track_gain,
        track_peak: info.track_peak,
        album_gain: info.album_gain,
    }
}

#[inline(always)]
fn frame_to_py(py: Python, frame: &id3::frames::Frame) -> PyObject {
    match frame {
        id3::frames::Frame::Text(f) => {
            if f.text.len() == 1 {
                f.text[0].as_str().into_pyobject(py).unwrap().into_any().unbind()
            } else {
                let list = PyList::new(py, &f.text).unwrap();
                list.into_any().unbind()
            }
        }
        id3::frames::Frame::UserText(f) => {
            if f.text.len() == 1 {
                f.text[0].as_str().into_pyobject(py).unwrap().into_any().unbind()
            } else {
                let list = PyList::new(py, &f.text).unwrap();
                list.into_any().unbind()
            }
        }
        id3::frames::Frame::Url(f) => {
            f.url.as_str().into_pyobject(py).unwrap().into_any().unbind()
        }
        id3::frames::Frame::UserUrl(f) => {
            f.url.as_str().into_pyobject(py).unwrap().into_any().unbind()
        }
        id3::frames::Frame::Comment(f) => {
            f.text.as_str().into_pyobject(py).unwrap().into_any().unbind()
        }
        id3::frames::Frame::Lyrics(f) => {
            f.text.as_str().into_pyobject(py).unwrap().into_any().unbind()
        }
        id3::frames::Frame::Picture(f) => {
            let dict = PyDict::new(py);
            dict.set_item("mime", &f.mime).unwrap();
            dict.set_item("type", f.pic_type as u8).unwrap();
            dict.set_item("desc", &f.desc).unwrap();
            dict.set_item("data", PyBytes::new(py, &f.data)).unwrap();
            dict.into_any().unbind()
        }
        id3::frames::Frame::Popularimeter(f) => {
            let dict = PyDict::new(py);
            dict.set_item("email", &f.email).unwrap();
            dict.set_item("rating", f.rating).unwrap();
            dict.set_item("count", f.count).unwrap();
            dict.into_any().unbind()
        }
        id3::frames::Frame::Binary(f) => {
            PyBytes::new(py, &f.data).into_any().unbind()
        }
        id3::frames::Frame::PairedText(f) => {
            let pairs: Vec<(&str, &str)> = f.people.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            let list = PyList::new(py, &pairs).unwrap();
            list.into_any().unbind()
        }
    }
}

#[inline(always)]
fn mp4_value_to_py(py: Python, value: &mp4::MP4TagValue) -> PyResult<PyObject> {
    match value {
        mp4::MP4TagValue::Text(v) => {
            if v.len() == 1 {
                Ok(v[0].as_str().into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(PyList::new(py, v)?.into_any().unbind())
            }
        }
        mp4::MP4TagValue::Integer(v) => {
            if v.len() == 1 {
                Ok(v[0].into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(PyList::new(py, v)?.into_any().unbind())
            }
        }
        mp4::MP4TagValue::IntPair(v) => {
            let pairs: Vec<_> = v.iter().map(|(a, b)| (*a, *b)).collect();
            if pairs.len() == 1 {
                Ok(PyTuple::new(py, &[pairs[0].0, pairs[0].1])?.into_any().unbind())
            } else {
                let list = PyList::empty(py);
                for (a, b) in &pairs {
                    list.append(PyTuple::new(py, &[*a, *b])?)?;
                }
                Ok(list.into_any().unbind())
            }
        }
        mp4::MP4TagValue::Bool(v) => {
            Ok((*v).into_pyobject(py)?.to_owned().into_any().unbind())
        }
        mp4::MP4TagValue::Cover(covers) => {
            let list = PyList::empty(py);
            for cover in covers {
                let dict = PyDict::new(py);
                dict.set_item("data", PyBytes::new(py, &cover.data))?;
                dict.set_item("format", cover.format as u8)?;
                list.append(dict)?;
            }
            Ok(list.into_any().unbind())
        }
        mp4::MP4TagValue::FreeForm(forms) => {
            let list = PyList::empty(py);
            for form in forms {
                list.append(PyBytes::new(py, &form.data))?;
            }
            Ok(list.into_any().unbind())
        }
        mp4::MP4TagValue::Data(d) => {
            Ok(PyBytes::new(py, d).into_any().unbind())
        }
    }
}

// ---- Batch API ----

/// Pre-serialized tag value — all decoding done in parallel phase.
enum BatchTagValue {
    Text(String),
    TextList(Vec<String>),
    Bytes(Vec<u8>),
    Int(i64),
    IntPair(i32, i32),
    Bool(bool),
    Picture { mime: String, pic_type: u8, desc: String, data: Vec<u8> },
    Popularimeter { email: String, rating: u8, count: u64 },
    PairedText(Vec<(String, String)>),
    CoverList(Vec<(Vec<u8>, u8)>),
    FreeFormList(Vec<Vec<u8>>),
}

/// Pre-serialized file — all Rust work done, ready for Python wrapping.
struct PreSerializedFile {
    length: f64,
    sample_rate: u32,
    channels: u32,
    bitrate: Option<u32>,
    tags: Vec<(String, BatchTagValue)>,
}

/// Convert a Frame to a BatchTagValue (runs in parallel phase, no GIL needed).
#[inline(always)]
fn frame_to_batch_value(frame: &id3::frames::Frame) -> BatchTagValue {
    match frame {
        id3::frames::Frame::Text(f) => {
            if f.text.len() == 1 {
                BatchTagValue::Text(f.text[0].clone())
            } else {
                BatchTagValue::TextList(f.text.clone())
            }
        }
        id3::frames::Frame::UserText(f) => {
            if f.text.len() == 1 {
                BatchTagValue::Text(f.text[0].clone())
            } else {
                BatchTagValue::TextList(f.text.clone())
            }
        }
        id3::frames::Frame::Url(f) => BatchTagValue::Text(f.url.clone()),
        id3::frames::Frame::UserUrl(f) => BatchTagValue::Text(f.url.clone()),
        id3::frames::Frame::Comment(f) => BatchTagValue::Text(f.text.clone()),
        id3::frames::Frame::Lyrics(f) => BatchTagValue::Text(f.text.clone()),
        id3::frames::Frame::Picture(f) => BatchTagValue::Picture {
            mime: f.mime.clone(),
            pic_type: f.pic_type as u8,
            desc: f.desc.clone(),
            data: f.data.clone(),
        },
        id3::frames::Frame::Popularimeter(f) => BatchTagValue::Popularimeter {
            email: f.email.clone(),
            rating: f.rating,
            count: f.count,
        },
        id3::frames::Frame::Binary(f) => BatchTagValue::Bytes(f.data.clone()),
        id3::frames::Frame::PairedText(f) => BatchTagValue::PairedText(f.people.clone()),
    }
}

/// Parse VorbisComment data directly into batch tags — single-pass, minimal allocations.
/// Skips vendor string, uses memchr for fast '=' finding, groups by key inline.
#[inline(always)]
fn parse_vc_to_batch_tags(data: &[u8]) -> Vec<(String, BatchTagValue)> {
    if data.len() < 8 { return Vec::new(); }
    let mut pos = 0usize;

    // Skip vendor string
    let vendor_len = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;
    if pos + vendor_len > data.len() { return Vec::new(); }
    pos += vendor_len;

    if pos + 4 > data.len() { return Vec::new(); }
    let count = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;

    let mut tags: Vec<(String, BatchTagValue)> = Vec::with_capacity(count.min(64));

    for _ in 0..count {
        if pos + 4 > data.len() { break; }
        let comment_len = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        pos += 4;
        if pos + comment_len > data.len() { break; }

        let raw = &data[pos..pos + comment_len];
        pos += comment_len;

        // Find '=' separator using memchr (SIMD-accelerated)
        let eq_pos = match memchr::memchr(b'=', raw) {
            Some(p) => p,
            None => continue,
        };

        let key_bytes = &raw[..eq_pos];
        let value_bytes = &raw[eq_pos + 1..];

        // Key: uppercase ASCII. Fast path for already-uppercase keys.
        let key = if key_bytes.iter().all(|&b| !b.is_ascii_lowercase()) {
            match std::str::from_utf8(key_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            }
        } else {
            // Fast ASCII uppercase (no allocation for checking)
            let mut k = String::with_capacity(key_bytes.len());
            for &b in key_bytes {
                k.push(if b.is_ascii_lowercase() { (b - 32) as char } else { b as char });
            }
            k
        };

        // Value: zero-copy if valid UTF-8
        let value = match std::str::from_utf8(value_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => String::from_utf8_lossy(value_bytes).into_owned(),
        };

        // Group by key (linear scan — fast for typical 5-15 unique keys)
        if let Some(entry) = tags.iter_mut().find(|(k, _)| k == &key) {
            if let BatchTagValue::TextList(ref mut v) = entry.1 {
                v.push(value);
            }
        } else {
            tags.push((key, BatchTagValue::TextList(vec![value])));
        }
    }

    tags
}

/// Batch-optimized FLAC parser: skips pictures, direct VC parsing.
#[inline(always)]
fn parse_flac_batch(data: &[u8]) -> Option<PreSerializedFile> {
    let flac_offset = if data.len() >= 4 && &data[0..4] == b"fLaC" {
        0
    } else if data.len() >= 10 && &data[0..3] == b"ID3" {
        let size = crate::id3::header::BitPaddedInt::syncsafe(&data[6..10]) as usize;
        let off = 10 + size;
        if off + 4 > data.len() || &data[off..off+4] != b"fLaC" { return None; }
        off
    } else {
        return None;
    };

    let mut pos = flac_offset + 4;
    let mut sample_rate = 0u32;
    let mut channels = 0u8;
    let mut length = 0.0f64;
    let mut tags = Vec::new();

    loop {
        if pos + 4 > data.len() { break; }
        let header = data[pos];
        let is_last = header & 0x80 != 0;
        let bt = header & 0x7F;
        let block_size = ((data[pos+1] as usize) << 16) | ((data[pos+2] as usize) << 8) | (data[pos+3] as usize);
        pos += 4;
        if pos + block_size > data.len() { break; }

        match bt {
            0 => {
                if let Ok(si) = flac::StreamInfo::parse(&data[pos..pos+block_size]) {
                    sample_rate = si.sample_rate;
                    channels = si.channels;
                    length = si.length;
                }
            }
            4 => {
                tags = parse_vc_to_batch_tags(&data[pos..pos+block_size]);
            }
            _ => {}
        }

        pos += block_size;
        if is_last { break; }
    }

    if sample_rate == 0 { return None; }

    Some(PreSerializedFile {
        length,
        sample_rate,
        channels: channels as u32,
        bitrate: None,
        tags,
    })
}

/// Batch-optimized OGG Vorbis parser: inline page headers, direct VC parsing.
#[inline(always)]
fn parse_ogg_batch(data: &[u8]) -> Option<PreSerializedFile> {
    if data.len() < 58 || &data[0..4] != b"OggS" { return None; }

    let serial = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);
    let num_seg = data[26] as usize;
    let seg_table_end = 27 + num_seg;
    if seg_table_end > data.len() { return None; }

    let page_data_size: usize = data[27..seg_table_end].iter().map(|&s| s as usize).sum();
    let first_page_end = seg_table_end + page_data_size;

    if seg_table_end + 30 > data.len() { return None; }
    let id_data = &data[seg_table_end..];
    if id_data.len() < 30 || &id_data[0..7] != b"\x01vorbis" { return None; }

    let channels = id_data[11];
    let sample_rate = u32::from_le_bytes([id_data[12], id_data[13], id_data[14], id_data[15]]);

    if first_page_end + 27 > data.len() { return None; }
    if &data[first_page_end..first_page_end+4] != b"OggS" { return None; }

    let seg2_count = data[first_page_end + 26] as usize;
    let seg2_table_start = first_page_end + 27;
    let seg2_table_end = seg2_table_start + seg2_count;
    if seg2_table_end > data.len() { return None; }

    let seg2_table = &data[seg2_table_start..seg2_table_end];
    let mut first_packet_size = 0usize;
    for &seg in seg2_table {
        first_packet_size += seg as usize;
        if seg < 255 { break; }
    }

    let comment_start = seg2_table_end;
    if comment_start + first_packet_size > data.len() { return None; }
    if first_packet_size < 7 { return None; }
    if &data[comment_start..comment_start+7] != b"\x03vorbis" { return None; }

    let vc_data = &data[comment_start + 7..comment_start + first_packet_size];
    let tags = parse_vc_to_batch_tags(vc_data);

    let length = ogg::find_last_granule(data, serial)
        .map(|g| if g > 0 && sample_rate > 0 { g as f64 / sample_rate as f64 } else { 0.0 })
        .unwrap_or(0.0);

    Some(PreSerializedFile {
        length,
        sample_rate,
        channels: channels as u32,
        bitrate: None,
        tags,
    })
}

/// Convert MP4TagValue to BatchTagValue (inline, no extra lookup).
#[inline(always)]
fn mp4_value_to_batch(value: &mp4::MP4TagValue) -> BatchTagValue {
    match value {
        mp4::MP4TagValue::Text(v) => {
            if v.len() == 1 { BatchTagValue::Text(v[0].clone()) }
            else { BatchTagValue::TextList(v.clone()) }
        }
        mp4::MP4TagValue::Integer(v) => {
            if v.len() == 1 { BatchTagValue::Int(v[0] as i64) }
            else { BatchTagValue::TextList(v.iter().map(|i| itoa::Buffer::new().format(*i).to_string()).collect()) }
        }
        mp4::MP4TagValue::IntPair(v) => {
            if v.len() == 1 { BatchTagValue::IntPair(v[0].0, v[0].1) }
            else { BatchTagValue::TextList(v.iter().map(|(a,b)| { let mut s = String::with_capacity(12); s.push('('); s.push_str(itoa::Buffer::new().format(*a)); s.push(','); s.push_str(itoa::Buffer::new().format(*b)); s.push(')'); s }).collect()) }
        }
        mp4::MP4TagValue::Bool(v) => BatchTagValue::Bool(*v),
        mp4::MP4TagValue::Cover(covers) => {
            BatchTagValue::CoverList(covers.iter().map(|c| (c.data.clone(), c.format as u8)).collect())
        }
        mp4::MP4TagValue::FreeForm(forms) => {
            BatchTagValue::FreeFormList(forms.iter().map(|f| f.data.clone()).collect())
        }
        mp4::MP4TagValue::Data(d) => BatchTagValue::Bytes(d.clone()),
    }
}

/// Parse MP3 data into batch result.
#[inline(always)]
fn parse_mp3_batch(data: &[u8], path: &str) -> Option<PreSerializedFile> {
    let mut f = mp3::MP3File::parse(data, path).ok()?;
    let mut tags = Vec::with_capacity(f.tags.frames.len());
    for (hash_key, frames) in f.tags.frames.iter_mut() {
        if let Some(lf) = frames.first_mut() {
            if let Ok(frame) = lf.decode() {
                tags.push((hash_key.as_str().to_string(), frame_to_batch_value(frame)));
            }
        }
    }
    Some(PreSerializedFile {
        length: f.info.length,
        sample_rate: f.info.sample_rate,
        channels: f.info.channels,
        bitrate: Some(f.info.bitrate),
        tags,
    })
}

/// Parse MP4 data into batch result.
#[inline(always)]
fn parse_mp4_batch(data: &[u8], path: &str) -> Option<PreSerializedFile> {
    let f = mp4::MP4File::parse(data, path).ok()?;
    let mut tags = Vec::with_capacity(f.tags.items.len());
    for (key, value) in &f.tags.items {
        tags.push((key.clone(), mp4_value_to_batch(value)));
    }
    Some(PreSerializedFile {
        length: f.info.length,
        sample_rate: f.info.sample_rate,
        channels: f.info.channels as u32,
        bitrate: None,
        tags,
    })
}

/// Parse + fully decode a single file from data (runs in parallel phase).
/// Uses extension-based fast dispatch to skip unnecessary scoring.
#[inline(always)]
fn parse_and_serialize(data: &[u8], path: &str) -> Option<PreSerializedFile> {
    let ext = path.rsplit('.').next().unwrap_or("");
    if ext.eq_ignore_ascii_case("flac") {
        return parse_flac_batch(data);
    }
    if ext.eq_ignore_ascii_case("ogg") {
        return parse_ogg_batch(data);
    }
    if ext.eq_ignore_ascii_case("mp3") {
        return parse_mp3_batch(data, path);
    }
    if ext.eq_ignore_ascii_case("m4a") || ext.eq_ignore_ascii_case("m4b")
        || ext.eq_ignore_ascii_case("mp4") || ext.eq_ignore_ascii_case("m4v") {
        return parse_mp4_batch(data, path);
    }

    let mp3_score = mp3::MP3File::score(path, data);
    let flac_score = flac::FLACFile::score(path, data);
    let ogg_score = ogg::OggVorbisFile::score(path, data);
    let mp4_score = mp4::MP4File::score(path, data);
    let max_score = mp3_score.max(flac_score).max(ogg_score).max(mp4_score);

    if max_score == 0 {
        return None;
    }

    if max_score == flac_score {
        parse_flac_batch(data)
    } else if max_score == ogg_score {
        parse_ogg_batch(data)
    } else if max_score == mp4_score {
        parse_mp4_batch(data, path)
    } else {
        parse_mp3_batch(data, path)
    }
}

/// Convert pre-serialized BatchTagValue to Python object (minimal serial work).
#[inline(always)]
fn batch_value_to_py(py: Python<'_>, bv: &BatchTagValue) -> PyResult<PyObject> {
    match bv {
        BatchTagValue::Text(s) => Ok(s.as_str().into_pyobject(py)?.into_any().unbind()),
        BatchTagValue::TextList(v) => Ok(PyList::new(py, v)?.into_any().unbind()),
        BatchTagValue::Bytes(d) => Ok(PyBytes::new(py, d).into_any().unbind()),
        BatchTagValue::Int(i) => Ok(i.into_pyobject(py)?.into_any().unbind()),
        BatchTagValue::IntPair(a, b) => Ok(PyTuple::new(py, &[*a, *b])?.into_any().unbind()),
        BatchTagValue::Bool(v) => Ok((*v).into_pyobject(py)?.to_owned().into_any().unbind()),
        BatchTagValue::Picture { mime, pic_type, desc, data } => {
            let dict = PyDict::new(py);
            dict.set_item(pyo3::intern!(py, "mime"), mime.as_str())?;
            dict.set_item(pyo3::intern!(py, "type"), *pic_type)?;
            dict.set_item(pyo3::intern!(py, "desc"), desc.as_str())?;
            dict.set_item(pyo3::intern!(py, "data"), PyBytes::new(py, data))?;
            Ok(dict.into_any().unbind())
        }
        BatchTagValue::Popularimeter { email, rating, count } => {
            let dict = PyDict::new(py);
            dict.set_item(pyo3::intern!(py, "email"), email.as_str())?;
            dict.set_item(pyo3::intern!(py, "rating"), *rating)?;
            dict.set_item(pyo3::intern!(py, "count"), *count)?;
            Ok(dict.into_any().unbind())
        }
        BatchTagValue::PairedText(pairs) => {
            let py_pairs: Vec<(&str, &str)> = pairs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            Ok(PyList::new(py, &py_pairs)?.into_any().unbind())
        }
        BatchTagValue::CoverList(covers) => {
            let list = PyList::empty(py);
            for (data, format) in covers {
                let dict = PyDict::new(py);
                dict.set_item(pyo3::intern!(py, "data"), PyBytes::new(py, data))?;
                dict.set_item(pyo3::intern!(py, "format"), *format)?;
                list.append(dict)?;
            }
            Ok(list.into_any().unbind())
        }
        BatchTagValue::FreeFormList(forms) => {
            let list = PyList::empty(py);
            for data in forms {
                list.append(PyBytes::new(py, data))?;
            }
            Ok(list.into_any().unbind())
        }
    }
}

/// Convert pre-serialized file to Python dict (minimal serial work with interned keys).
#[inline(always)]
fn preserialized_to_py_dict(py: Python<'_>, pf: &PreSerializedFile) -> PyResult<Py<PyAny>> {
    let inner = PyDict::new(py);
    inner.set_item(pyo3::intern!(py, "length"), pf.length)?;
    inner.set_item(pyo3::intern!(py, "sample_rate"), pf.sample_rate)?;
    inner.set_item(pyo3::intern!(py, "channels"), pf.channels)?;
    if let Some(br) = pf.bitrate {
        inner.set_item(pyo3::intern!(py, "bitrate"), br)?;
    }
    let tags_dict = PyDict::new(py);
    for (key, value) in &pf.tags {
        tags_dict.set_item(key.as_str(), batch_value_to_py(py, value)?)?;
    }
    inner.set_item(pyo3::intern!(py, "tags"), tags_dict)?;
    Ok(inner.into_any().unbind())
}

/// JSON-escape a string value for safe embedding in JSON.
/// Fast path: if string has no special characters, avoid per-char scanning.
#[inline(always)]
fn json_escape_to(s: &str, out: &mut String) {
    out.push('"');
    // Fast path: check if any escaping is needed using memchr
    let needs_escape = s.bytes().any(|b| b == b'"' || b == b'\\' || b < 0x20);
    if !needs_escape {
        out.push_str(s);
    } else {
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    out.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => out.push(c),
            }
        }
    }
    out.push('"');
}

/// Serialize a BatchTagValue to a JSON fragment.
#[inline(always)]
fn batch_value_to_json(bv: &BatchTagValue, out: &mut String) {
    match bv {
        BatchTagValue::Text(s) => json_escape_to(s, out),
        BatchTagValue::TextList(v) => {
            out.push('[');
            for (i, s) in v.iter().enumerate() {
                if i > 0 { out.push(','); }
                json_escape_to(s, out);
            }
            out.push(']');
        }
        BatchTagValue::Int(i) => {
            write_int(out, *i);
        }
        BatchTagValue::IntPair(a, b) => {
            out.push('[');
            write_int(out, *a);
            out.push(',');
            write_int(out, *b);
            out.push(']');
        }
        BatchTagValue::Bool(v) => {
            out.push_str(if *v { "true" } else { "false" });
        }
        BatchTagValue::PairedText(pairs) => {
            out.push('[');
            for (i, (a, b)) in pairs.iter().enumerate() {
                if i > 0 { out.push(','); }
                out.push('[');
                json_escape_to(a, out);
                out.push(',');
                json_escape_to(b, out);
                out.push(']');
            }
            out.push(']');
        }
        // Binary data types: serialize as null (skip in JSON mode)
        BatchTagValue::Bytes(_) | BatchTagValue::Picture { .. } |
        BatchTagValue::Popularimeter { .. } | BatchTagValue::CoverList(_) |
        BatchTagValue::FreeFormList(_) => {
            out.push_str("null");
        }
    }
}

/// Write an integer to a string using itoa (faster than format!).
#[inline(always)]
fn write_int(out: &mut String, v: impl itoa::Integer) {
    let mut buf = itoa::Buffer::new();
    out.push_str(buf.format(v));
}

/// Write a float to a string using ryu (faster than format!).
#[inline(always)]
fn write_float(out: &mut String, v: f64) {
    let mut buf = ryu::Buffer::new();
    out.push_str(buf.format(v));
}

/// Serialize a PreSerializedFile to a JSON object string.
#[inline(always)]
fn preserialized_to_json(pf: &PreSerializedFile, out: &mut String) {
    out.push_str("{\"length\":");
    write_float(out, pf.length);
    out.push_str(",\"sample_rate\":");
    write_int(out, pf.sample_rate);
    out.push_str(",\"channels\":");
    write_int(out, pf.channels);
    if let Some(br) = pf.bitrate {
        out.push_str(",\"bitrate\":");
        write_int(out, br);
    }
    out.push_str(",\"tags\":{");
    let mut first = true;
    for (key, value) in &pf.tags {
        if matches!(value, BatchTagValue::Bytes(_) | BatchTagValue::Picture { .. } |
            BatchTagValue::Popularimeter { .. } | BatchTagValue::CoverList(_) |
            BatchTagValue::FreeFormList(_)) {
            continue;
        }
        if !first { out.push(','); }
        first = false;
        json_escape_to(key, out);
        out.push(':');
        batch_value_to_json(value, out);
    }
    out.push_str("}}");
}

/// Lazy batch result — stores parsed Rust data, creates Python objects on demand.
#[pyclass(name = "BatchResult")]
struct PyBatchResult {
    files: Vec<(String, PreSerializedFile)>,
}

#[pymethods]
impl PyBatchResult {
    fn __len__(&self) -> usize {
        self.files.len()
    }

    fn keys(&self) -> Vec<String> {
        self.files.iter().map(|(p, _)| p.clone()).collect()
    }

    fn __contains__(&self, path: &str) -> bool {
        self.files.iter().any(|(p, _)| p == path)
    }

    fn __getitem__(&self, py: Python<'_>, path: &str) -> PyResult<PyObject> {
        for (p, pf) in &self.files {
            if p == path {
                return preserialized_to_py_dict(py, pf);
            }
        }
        Err(PyKeyError::new_err(path.to_string()))
    }

    fn items(&self, py: Python<'_>) -> PyResult<PyObject> {
        let list = PyList::empty(py);
        for (p, pf) in &self.files {
            let dict = preserialized_to_py_dict(py, pf)?;
            let tuple = PyTuple::new(py, &[p.as_str().into_pyobject(py)?.into_any(), dict.bind(py).clone().into_any()])?;
            list.append(tuple)?;
        }
        Ok(list.into_any().unbind())
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        // Materialize everything as a dict using orjson for speed
        let mut json = String::with_capacity(self.files.len() * 600);
        json.push('{');
        let mut first = true;
        for (path, pf) in &self.files {
            if !first { json.push(','); }
            first = false;
            json_escape_to(path, &mut json);
            json.push(':');
            preserialized_to_json(pf, &mut json);
        }
        json.push('}');

        let loads_fn = py.import("orjson")
            .and_then(|m| m.getattr("loads"))
            .or_else(|_| py.import("json").and_then(|m| m.getattr("loads")))?;
        let json_bytes = PyBytes::new(py, json.as_bytes());
        let result = loads_fn.call1((json_bytes,))?;
        Ok(result.into_any().unbind())
    }
}

/// Batch open: read and parse multiple files in parallel using rayon.
/// Returns a BatchResult with lazy Python object creation.
/// All file I/O + parsing happens in parallel (no GIL).
/// Python objects are created on demand when accessing individual files.
#[pyfunction]
fn batch_open(py: Python<'_>, filenames: Vec<String>) -> PyResult<PyBatchResult> {
    use rayon::prelude::*;

    let files: Vec<(String, PreSerializedFile)> = py.allow_threads(|| {
        let n = filenames.len();
        if n == 0 { return Vec::new(); }

        // Parallel read+parse with rayon (indexed map for best scheduling).
        let results: Vec<Option<PreSerializedFile>> = filenames
            .par_iter()
            .map(|path| {
                let data = std::fs::read(path).ok()?;
                parse_and_serialize(&data, path)
            })
            .collect();

        filenames.into_iter()
            .zip(results)
            .filter_map(|(path, pf)| pf.map(|pf| (path, pf)))
            .collect()
    });

    Ok(PyBatchResult { files })
}

/// Diagnostic version: measures I/O vs parse vs parallel overhead.
#[pyfunction]
fn batch_diag(py: Python<'_>, filenames: Vec<String>) -> PyResult<String> {
    use rayon::prelude::*;
    use std::time::Instant;

    let result = py.allow_threads(|| {
        let n = filenames.len();

        // Phase 1: Sequential file reads
        let t1 = Instant::now();
        let file_data: Vec<(String, Vec<u8>)> = filenames.iter()
            .filter_map(|p| std::fs::read(p).ok().map(|d| (p.clone(), d)))
            .collect();
        let read_seq_us = t1.elapsed().as_micros();

        // Phase 2: Sequential parse (no I/O)
        let t2 = Instant::now();
        let _: Vec<_> = file_data.iter()
            .filter_map(|(p, d)| parse_and_serialize(d, p).map(|pf| (p.clone(), pf)))
            .collect();
        let parse_seq_us = t2.elapsed().as_micros();

        // Phase 3: Parallel parse (no I/O)
        let t3 = Instant::now();
        let _: Vec<_> = file_data.par_iter()
            .filter_map(|(p, d)| parse_and_serialize(d, p).map(|pf| (p.clone(), pf)))
            .collect();
        let parse_par_us = t3.elapsed().as_micros();

        // Phase 4: Parallel read+parse (current approach)
        let t4 = Instant::now();
        let _: Vec<_> = filenames.par_iter().filter_map(|path| {
            let data = std::fs::read(path).ok()?;
            let pf = parse_and_serialize(&data, path)?;
            Some((path.clone(), pf))
        }).collect();
        let full_par_us = t4.elapsed().as_micros();

        format!(
            "n={} | seq_read={}µs seq_parse={}µs par_parse={}µs full_par={}µs | \
             parse_par_speedup={:.1}x io_fraction={:.0}%",
            n, read_seq_us, parse_seq_us, parse_par_us, full_par_us,
            parse_seq_us as f64 / parse_par_us.max(1) as f64,
            read_seq_us as f64 / (read_seq_us + parse_seq_us).max(1) as f64 * 100.0,
        )
    });

    Ok(result)
}

/// Auto-detect file format and open.
#[pyfunction]
#[pyo3(signature = (filename, easy=false))]
fn file_open(py: Python<'_>, filename: &str, easy: bool) -> PyResult<PyObject> {
    let _ = easy;

    let data = std::fs::read(filename)
        .map_err(|e| PyIOError::new_err(format!("Cannot open file: {}", e)))?;

    let mp3_score = mp3::MP3File::score(filename, &data);
    let flac_score = flac::FLACFile::score(filename, &data);
    let ogg_score = ogg::OggVorbisFile::score(filename, &data);
    let mp4_score = mp4::MP4File::score(filename, &data);

    let max_score = mp3_score.max(flac_score).max(ogg_score).max(mp4_score);

    if max_score == 0 {
        return Err(PyValueError::new_err(format!(
            "Unable to detect format for: {}",
            filename
        )));
    }

    if max_score == flac_score {
        let f = PyFLAC::from_data(&data, filename)?;
        Ok(f.into_pyobject(py)?.into_any().unbind())
    } else if max_score == ogg_score {
        let f = PyOggVorbis::from_data(&data, filename)?;
        Ok(f.into_pyobject(py)?.into_any().unbind())
    } else if max_score == mp4_score {
        let f = PyMP4::from_data(&data, filename)?;
        Ok(f.into_pyobject(py)?.into_any().unbind())
    } else {
        let f = PyMP3::from_data(&data, filename)?;
        Ok(f.into_pyobject(py)?.into_any().unbind())
    }
}

// ---- Module registration ----

#[pymodule]
fn mutagen_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMP3>()?;
    m.add_class::<PyMPEGInfo>()?;
    m.add_class::<PyID3>()?;
    m.add_class::<PyFLAC>()?;
    m.add_class::<PyStreamInfo>()?;
    m.add_class::<PyVComment>()?;
    m.add_class::<PyOggVorbis>()?;
    m.add_class::<PyOggVorbisInfo>()?;
    m.add_class::<PyMP4>()?;
    m.add_class::<PyMP4Info>()?;
    m.add_class::<PyMP4Tags>()?;
    m.add_class::<PyBatchResult>()?;

    m.add_function(wrap_pyfunction!(file_open, m)?)?;
    m.add_function(wrap_pyfunction!(batch_open, m)?)?;
    m.add_function(wrap_pyfunction!(batch_diag, m)?)?;

    m.add("MutagenError", m.py().get_type::<common::error::MutagenPyError>())?;
    m.add("ID3Error", m.py().get_type::<common::error::ID3Error>())?;
    m.add("ID3NoHeaderError", m.py().get_type::<common::error::ID3NoHeaderError>())?;
    m.add("MP3Error", m.py().get_type::<common::error::MP3Error>())?;
    m.add("HeaderNotFoundError", m.py().get_type::<common::error::HeaderNotFoundError>())?;
    m.add("FLACError", m.py().get_type::<common::error::FLACError>())?;
    m.add("FLACNoHeaderError", m.py().get_type::<common::error::FLACNoHeaderError>())?;
    m.add("OggError", m.py().get_type::<common::error::OggError>())?;
    m.add("MP4Error", m.py().get_type::<common::error::MP4Error>())?;

    m.add("File", wrap_pyfunction!(file_open, m)?)?;

    Ok(())
}
} // mod python_bindings
