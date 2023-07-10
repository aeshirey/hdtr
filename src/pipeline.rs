use crate::{HdtrError, InputImage, InputImages};
use image::{DynamicImage, GenericImageView, Pixel, RgbImage};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub enum Mask {
    Default,
    Path(String),
}

#[derive(Serialize, Deserialize)]
pub struct PipelineInputImage {
    image: String,
    mask: Option<String>,
}

impl PipelineInputImage {
    pub(crate) fn load(&self) -> Result<(InputImage, DynamicImage), HdtrError> {
        let image = InputImage::new(&self.image)?;

        let expected = image.im.dimensions();
        let mask_filename = match &self.mask {
            Some(f) => f,
            None => return Ok((image, default_mask(expected))),
        };

        let mask = image::open(mask_filename)?;

        // check the dimensions
        let received = mask.dimensions();
        if expected != received {
            Err(HdtrError::DimensionMismatch {
                expected,
                received,
                details: format!(
                    "{} and its mask {mask_filename} have different dimensions",
                    self.image
                )
                .into(),
            })
        } else {
            Ok((image, mask))
        }
    }
}

impl<S: Into<String>> From<S> for PipelineInputImage {
    fn from(value: S) -> Self {
        PipelineInputImage {
            image: value.into(),
            mask: None,
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone)]
pub enum MaskType {
    VerticalFlat,
    HorizontalFlat,
    VerticalLogistic { k: f64 },
    HorizontalLogistic { k: f64 },
}

#[derive(Serialize, Deserialize)]
pub struct Pipeline {
    pub filenames: Vec<PipelineInputImage>,
    pub generate_masks: Option<MaskType>,
    pub normalize_masks: Option<bool>,
    pub save_masks: Option<bool>,
    pub save: String,
}

impl Pipeline {
    pub fn save_example<P: AsRef<Path>>(
        destination: P,
        images: Option<Vec<String>>,
    ) -> Result<(), std::io::Error> {
        let filenames = match images {
            Some(fs) => fs.into_iter().map(|f| f.into()).collect(),
            None => vec![
                "image01.png".into(),
                "image02.png".into(),
                "image03.png".into(),
                "image04.png".into(),
            ],
        };

        let ex = Pipeline {
            filenames,
            generate_masks: Some(MaskType::VerticalLogistic { k: 0.01 }),
            normalize_masks: Some(true),
            save_masks: Some(false),
            save: "blended.png".to_string(),
        };

        let json = serde_json::to_string_pretty(&ex).unwrap();

        std::fs::write(destination, json)?;

        Ok(())
    }

    /// Validates that the pipeline seems okay
    pub fn validate(&self) -> Result<(), HdtrError> {
        if self.filenames.is_empty() {
            return Err(HdtrError::NoInputFilesSpecified);
        }

        for file in &self.filenames {
            if !std::path::Path::new(&file.image).exists() {
                return Err(HdtrError::InputFileDoesNotExist(file.image.to_string()));
            }

            if let Some(mask) = &file.mask {
                if !std::path::Path::new(mask).exists() {
                    return Err(HdtrError::InputFileDoesNotExist(mask.to_string()));
                }
            }
        }

        Ok(())
    }

    pub fn execute(&self) -> Result<(), HdtrError> {
        self.validate()?;

        let mut it = self.filenames.iter();

        let (im, m) = it.next().unwrap().load()?;

        let expected = im.im.dimensions();

        let mut images = vec![im];
        let mut masks = vec![m];

        for filename in it {
            let (im, m) = filename.load()?;

            let received = im.im.dimensions();

            if expected != received {
                return Err(HdtrError::DimensionMismatch {
                    expected,
                    received,
                    details: format!("{} has different dimensions than expected", filename.image)
                        .into(),
                });
            }

            images.push(im);
            masks.push(m);
        }

        let mut images = InputImages {
            images,
            masks,
            width: expected.0,
            height: expected.1,
        };

        if let Some(mask_type) = self.generate_masks {
            images.generate_masks(mask_type);
        }

        if self.normalize_masks == Some(true) {
            images.normalize_masks();
        }

        if self.save_masks == Some(true) {
            images.save_masks()?;
        }

        images.save(&self.save)?;

        Ok(())
    }
}

#[test]
fn test_save_example() {
    Pipeline::save_example("test_pipeline.json", None).unwrap();
}

fn default_mask((width, height): (u32, u32)) -> DynamicImage {
    let mut canvas = RgbImage::new(width, height);
    let black = *Pixel::from_slice(&[0, 0, 0]);

    for x in 0..width {
        for y in 0..height {
            canvas.put_pixel(x, y, black);
        }
    }

    DynamicImage::ImageRgb8(canvas)
}
