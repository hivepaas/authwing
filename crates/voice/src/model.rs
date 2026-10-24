//! The Whisper models voice can use, and where to get them.
//!
//! The model is a download of its own, not part of the app: the user turns
//! voice on and fetches one. The multilingual base model is the one to
//! get: on an M1 Pro it took about 95 ms per phrase, and the larger models
//! named no more accounts right in our measurements, at three to fourteen
//! times the time. The sums are those of the files as published, to check
//! a download against.

use std::path::{Path, PathBuf};

/// A Whisper model file in whisper.cpp's ggml format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    /// What config.toml calls it: `tiny`, `base`, `small`, `large`.
    pub name: &'static str,
    /// The file's name, here and on Hugging Face.
    pub file: &'static str,
    /// Where to download it.
    pub url: &'static str,
    /// Its size in bytes.
    pub bytes: u64,
    /// Its SHA-256, in lowercase hex.
    pub sha256: &'static str,
}

/// The model to offer: multilingual base, 148 MB, about 95 ms per phrase.
pub const RECOMMENDED: Model = BASE;

/// Multilingual tiny, 78 MB: faster, and wrong more often.
pub const TINY: Model = Model {
    name: "tiny",
    file: "ggml-tiny.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
    bytes: 77_691_713,
    sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
};

/// Multilingual base, 148 MB.
pub const BASE: Model = Model {
    name: "base",
    file: "ggml-base.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
    bytes: 147_951_465,
    sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
};

/// Multilingual small, quantized to 5 bits, 190 MB: about 280 ms per phrase.
pub const SMALL_Q5_1: Model = Model {
    name: "small",
    file: "ggml-small-q5_1.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
    bytes: 190_085_487,
    sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
};

/// Large v3 turbo, quantized to 5 bits, 574 MB: about 1.3 s per phrase.
pub const LARGE_V3_TURBO_Q5_0: Model = Model {
    name: "large",
    file: "ggml-large-v3-turbo-q5_0.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
    bytes: 574_041_195,
    sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
};

/// The models there are, smallest first.
pub const MODELS: &[Model] = &[TINY, BASE, SMALL_Q5_1, LARGE_V3_TURBO_Q5_0];

/// The model config.toml names, if it is one.
pub fn by_name(name: &str) -> Option<Model> {
    MODELS.iter().copied().find(|m| m.name == name.trim())
}

impl Model {
    /// Where the model is in `dir`, the folder models are downloaded to.
    pub fn path_in(&self, dir: &Path) -> PathBuf {
        dir.join(self.file)
    }

    /// Whether it is in `dir`, whole: a file of its size. Its sum was
    /// checked when it was downloaded.
    pub fn is_in(&self, dir: &Path) -> bool {
        std::fs::metadata(self.path_in(dir)).is_ok_and(|m| m.is_file() && m.len() == self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_are_described_consistently() {
        assert_eq!(RECOMMENDED, BASE);
        for model in MODELS {
            assert!(model.url.ends_with(&format!("/{}", model.file)));
            assert_eq!(model.sha256.len(), 64);
            assert!(
                model
                    .sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }
        assert!(MODELS.windows(2).all(|pair| pair[0].bytes < pair[1].bytes));
    }

    #[test]
    fn models_are_found_by_name() {
        for model in MODELS {
            assert_eq!(by_name(model.name), Some(*model));
        }
        assert_eq!(by_name(" base "), Some(BASE));
        assert_eq!(by_name(""), None);
        assert_eq!(by_name("medium"), None);
    }

    #[test]
    fn a_model_is_there_only_whole() {
        let dir = std::env::temp_dir().join(format!("authwing-models-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!TINY.is_in(&dir));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(TINY.path_in(&dir), b"half").expect("write");
        assert!(!TINY.is_in(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
