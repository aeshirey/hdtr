use image::{DynamicImage, GenericImage, GenericImageView, Pixel, RgbImage};
use pipeline::MaskType;
use std::path::{Path, PathBuf};

mod err;
pub mod pipeline;
pub use err::HdtrError;

pub struct InputImage {
    pub path: PathBuf,
    pub im: DynamicImage,
}

impl InputImage {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, HdtrError> {
        let path = path.as_ref().into();
        //
        let im = image::open(&path)?;
        Ok(Self { path, im })
    }
}

pub struct InputImages {
    pub images: Vec<InputImage>,
    pub masks: Vec<DynamicImage>,
    pub width: u32,
    pub height: u32,
}

impl InputImages {
    pub fn new<P: AsRef<std::path::Path>>(paths: &[P]) -> Result<Self, HdtrError> {
        let mut it = paths.iter();

        let input_image = InputImage::new(it.next().expect("Non-empty slice"))?;
        let width = input_image.im.width();
        let height = input_image.im.height();
        let mut images = vec![input_image];

        for p in it {
            let input_image = InputImage::new(p)?;
            assert_eq!(width, input_image.im.width());
            assert_eq!(height, input_image.im.height());
            images.push(input_image);
        }

        let masks = Self::default_masks(&images, width, height);

        Ok(Self {
            images,
            masks,
            width,
            height,
        })
    }

    fn default_masks(images: &[InputImage], width: u32, height: u32) -> Vec<DynamicImage> {
        let mut masks = Vec::new();

        // The precise width (with fractional part) of each stripe. This avoids accumulating
        // remainders that aren't handled.
        let width_f = width as f64 / images.len() as f64;

        for i in 0..images.len() {
            let mut canvas = RgbImage::new(width, height);

            let x_start = (width_f * (i as f64)) as u32;
            let x_end = (width_f * ((i + 1) as f64)) as u32;

            for x in x_start..x_end {
                for y in 0..height {
                    let p = *Pixel::from_slice(&[255, 255, 255]);
                    canvas.put_pixel(x, y, p);
                }
            }

            masks.push(DynamicImage::ImageRgb8(canvas));
        }

        masks
    }

    pub fn normalize_masks(&mut self) {
        // Sum up the contribution of each mask at each pixel
        let sums = {
            let mut sums = vec![0u32; (self.width * self.height) as usize];

            for x in 0..self.width {
                for y in 0..self.height {
                    let idx = (self.width * y + x) as usize;
                    for i in 0..self.images.len() {
                        let value = self.masks[i].get_pixel(x, y).to_rgb().0[0];
                        sums[idx] += value as u32;
                    }
                }
            }
            sums
        };

        // Modify every mask to be [0,255] according to how much it contributed
        for x in 0..self.width {
            for y in 0..self.height {
                let idx = (self.width * y + x) as usize;
                let denominator = sums[idx];

                for i in 0..self.masks.len() {
                    let numerator = self.masks[i].get_pixel(x, y).to_rgb().0[0];

                    let scaled = (255. * numerator as f64 / denominator as f64) as u8;
                    let rgb = [scaled, scaled, scaled, 255];
                    let pixel = Pixel::from_slice(&rgb[..]);
                    self.masks[i].put_pixel(x, y, *pixel);
                }
            }
        }
    }

    pub fn save<P: AsRef<Path>>(&self, destination: P) -> Result<(), HdtrError> {
        let mut canvas = RgbImage::new(self.width, self.height);

        for x in 0..self.width {
            for y in 0..self.height {
                let (mut r_out, mut g_out, mut b_out) = (0., 0., 0.);

                for i in 0..self.masks.len() {
                    // Input pixel
                    let p = self.images[i].im.get_pixel(x, y).to_rgb();
                    // mask pixel
                    let pm = self.masks[i].get_pixel(x, y).to_rgb();

                    // Add to the output the value of this pixel multiplied by [0, 1]
                    r_out += p[0] as f64 * (pm[0] as f64 / 255.);
                    g_out += p[1] as f64 * (pm[1] as f64 / 255.);
                    b_out += p[2] as f64 * (pm[2] as f64 / 255.);
                }

                let r = r_out as u8;
                let g = g_out as u8;
                let b = b_out as u8;
                let rgb = [r, g, b];
                let p = Pixel::from_slice(&rgb[..]);
                canvas.put_pixel(x, y, *p);
            }
        }

        canvas.save(destination)?;

        Ok(())
    }

    pub fn save_masks(&self) -> Result<(), HdtrError> {
        for (im, m) in self.images.iter().zip(&self.masks) {
            let parent = im
                .path
                .parent()
                .expect("Couldn't get parent directory for image");

            let file_stem = im
                .path
                .file_stem()
                .and_then(|osstr| osstr.to_str())
                .expect("Couldn't get file name for image");

            let mask_filename = format!("{file_stem}_mask.png");
            let mask_path = parent.join(mask_filename);

            m.save(&mask_path)
                .map_err(|_| HdtrError::ErrorWritingFile(mask_path))?;
        }

        Ok(())
    }

    pub fn set_mask(&mut self, index: usize, mask: DynamicImage) {
        assert!(index < self.masks.len(), "Invalid mask index");
        assert_eq!(self.width, mask.width());
        assert_eq!(self.height, mask.height());

        self.masks[index] = mask;
    }

    pub(crate) fn generate_masks(&mut self, mask_type: MaskType) {
        for i in 0..self.masks.len() {
            self.masks[i] = self.generate_mask(i, mask_type);
        }
    }

    fn generate_mask(&self, image_num: usize, mask_type: MaskType) -> DynamicImage {
        let mut canvas = RgbImage::new(self.width, self.height);

        let white = *Pixel::from_slice(&[255, 255, 255]);

        let width_f = self.width as f64 / self.images.len() as f64;
        let height_f = self.height as f64 / self.images.len() as f64;

        match mask_type {
            MaskType::VerticalFlat => {
                // The precise width (with fractional part) of each stripe. This avoids accumulating
                // remainders that aren't handled.

                let x_start = (width_f * (image_num as f64)) as u32;
                let x_end = (width_f * ((image_num + 1) as f64)) as u32;

                for x in x_start..x_end {
                    for y in 0..self.height {
                        canvas.put_pixel(x, y, white);
                    }
                }
            }
            MaskType::HorizontalFlat => {
                // Similar to above but with banded height

                let y_start = (height_f * (image_num as f64)) as u32;
                let y_end = (height_f * ((image_num + 1) as f64)) as u32;

                for x in 0..self.width {
                    for y in y_start..y_end {
                        canvas.put_pixel(x, y, white);
                    }
                }
            }
            MaskType::VerticalLogistic { k } => {
                // Where should the most intense part be?
                let center_x = (image_num as f64 * width_f + width_f / 2.) as u32;

                for x in 0..self.width {
                    // Get the absolute distance from the center of this slice
                    let distance_x = (x as f64 - center_x as f64).abs();

                    let logit = logistic(distance_x, k * width_f);
                    let p = ((1. - logit) * 255.) as u8;
                    let p = [p, p, p];

                    let p = *Pixel::from_slice(&p);

                    for y in 0..self.height {
                        canvas.put_pixel(x, y, p);
                    }
                }
            }
            MaskType::HorizontalLogistic { k } => {
                // Where should the most intense part be?
                let center_y = (image_num as f64 * height_f + height_f / 2.) as u32;

                for y in 0..self.height {
                    // Get the absolute distance from the center of this slice
                    let distance_y = (y as f64 - center_y as f64).abs();

                    let logit = logistic(distance_y, k * height_f);
                    let p = ((1. - logit) * 255.) as u8;
                    let p = [p, p, p];

                    let p = *Pixel::from_slice(&p);

                    for x in 0..self.width {
                        canvas.put_pixel(x, y, p);
                    }
                }
            }
        }

        DynamicImage::ImageRgb8(canvas)
    }

    pub fn create_masks<F>(&mut self, f: F)
    where
        F: Fn(usize, u32, u32) -> u8,
    {
        for i in 0..self.masks.len() {
            let mut canvas = RgbImage::new(self.width, self.height);

            for x in 0..self.width {
                for y in 0..self.height {
                    let p = f(i, x, y);
                    let slice = [p, p, p];
                    let p = Pixel::from_slice(&slice[..]);
                    canvas.put_pixel(x, y, *p);
                }
            }

            self.masks[i] = DynamicImage::ImageRgb8(canvas);
        }
    }

    pub fn create_mask<F>(&mut self, index: usize, f: F)
    where
        F: Fn(u32, u32) -> u8,
    {
        assert!(index < self.masks.len(), "Invalid mask index");
        let mut canvas = RgbImage::new(self.width, self.height);

        for x in 0..self.width {
            for y in 0..self.height {
                let p = f(x, y);
                //let p = im.get_pixel(x, y);
                let slice = [p, p, p];
                let p = Pixel::from_slice(&slice[..]);

                canvas.put_pixel(x, y, *p);
            }
        }

        self.masks[index] = DynamicImage::ImageRgb8(canvas);
    }
}

/// `k` is the steepness and should probably be roughly 0.01.
/// For larger values (eg, 0.1), the band drops off quickly, meaning we have a narrow slice.
/// For smaller values (eg, 0.001), the band is so wide that it almost smooshes everything together.
pub fn logistic(distance: f64, k: f64) -> f64 {
    let sup = -k * distance;
    1. / (sup.exp() + 1.)
}
