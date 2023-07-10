use std::path::Path;

use colored::*;
use hdtr::{pipeline::Pipeline, HdtrError, InputImage};

fn main() -> Result<(), HdtrError> {
    let mut example_images = None;
    let mut pipeline = None;
    let mut check_pipeline = false;

    for arg in std::env::args().skip(1) {
        if arg == "--help" || arg == "-h" {
            usage();
            return Ok(());
        } else if arg.ends_with(".json") {
            let json = std::fs::read_to_string(&arg)?;
            pipeline = Some(serde_json::from_str::<Pipeline>(&json)?);
        } else if arg == "--example" {
            example_images = Some(Vec::new());
        } else if arg == "--check" {
            check_pipeline = true;
        } else if example_images.is_some() && InputImage::new(&arg).is_ok() {
            example_images.as_mut().unwrap().push(arg);
        } else {
            eprintln!("Unexpected argument: {arg}");
        }
    }

    match (example_images, pipeline) {
        (None, None) => {
            usage();
            Ok(())
        }
        (None, Some(p)) if check_pipeline => {
            p.validate()?;
            println!("No problems found in pipeline. This does not guarantee success -- image files must be valid and the same dimensions, for example.");
            Ok(())
        }
        (None, Some(p)) => p.execute(),
        (Some(i), None) if i.is_empty() => save_example(None),
        (Some(i), None) => save_example(Some(i)),
        (Some(_), Some(_)) => {
            eprintln!("--example may not be specified with a pipeline file");
            usage();
            Ok(())
        }
    }
}

fn usage() {
    let exe = std::env::args().next();
    let exe = exe
        .as_ref()
        .and_then(|a| Path::new(a).file_name())
        .and_then(|osstr| osstr.to_str())
        .unwrap_or("hdtr");

    println!("Usage:");
    println!(
        "    {} --help                      -- Shows this help output",
        exe.green(),
    );

    println!(
        "    {} {}               -- Runs the specified {} file, generating an HDTR image",
        exe.green(),
        "pipeline.json".yellow(),
        "pipeline".yellow()
    );

    println!(
        "    {} --check {}       -- Validates the specified {} file but does not generate an image",
        exe.green(),
        "pipeline.json".yellow(),
        "pipeline".yellow()
    );

    println!(
        "    {} --example                   -- Creates a sample pipeline pipeline file",
        exe.green(),
    );

    println!(
        "    {} --example {} {} {} -- Creates a sample pipeline pipeline file from the specified {}",
        exe.green(),
        "1.jpg".cyan(),
        "2.png".cyan(),
        "3.bmp".cyan(),
        "input files".cyan(),
    );

    println!();

    println!("To perform HDTR processing, images are expected to have exactly equal dimensions.");
}

fn save_example(images: Option<Vec<String>>) -> Result<(), HdtrError> {
    const EXAMPLE_FILE_STEM: &str = "example_pipeline";

    for num in 1u32.. {
        let filename = format!("{EXAMPLE_FILE_STEM}{num}.json");
        if !Path::new(&filename).exists() {
            Pipeline::save_example(&filename, images)?;

            println!("Created sample pipeline @ '{}'", filename.green());
            return Ok(());
        }
    }

    unreachable!()
}
