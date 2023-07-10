use std::{borrow::Cow, path::PathBuf};

use image::ImageError;

#[derive(Debug)]
pub enum HdtrError {
    IO(std::io::Error),
    NoInputFilesSpecified,
    InputFileDoesNotExist(String),
    InputFileReadError(ImageError),

    DimensionMismatch {
        expected: (u32, u32),
        received: (u32, u32),
        details: Cow<'static, str>,
    },

    InvalidPipelineJson(serde_json::Error),
    PipelineError(Cow<'static, str>),
    NoSaveOperationSpecified,
    ErrorWritingFile(PathBuf),
    HDTR(Cow<'static, str>),
}

macro_rules! from_err {
    ($variant: ident, $t: ty) => {
        impl From<$t> for HdtrError {
            fn from(value: $t) -> Self {
                Self::$variant(value.into())
            }
        }
    };
}

from_err!(InvalidPipelineJson, serde_json::Error);
from_err!(IO, std::io::Error);
from_err!(InputFileReadError, ImageError);
from_err!(HDTR, &'static str);
from_err!(HDTR, String);
