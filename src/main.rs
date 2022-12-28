use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use lazy_static::lazy_static;
use palette::Hsl;
use palette::IntoColor;
use palette::Srgb;
use rand::distributions::Uniform;
use rand::prelude::*;
use regex::Regex;
use resvg::usvg_text_layout::fontdb;
use resvg::usvg_text_layout::TreeTextToPath;
use tiny_skia::Color;
use tiny_skia::PixmapPaint;
use tiny_skia::Transform;
use usvg::Options;
use usvg::ScreenSize;

#[derive(Parser, Debug)]
struct Opts {
    #[arg(value_parser)]
    inputdir: PathBuf,
    #[arg(value_parser)]
    outputdir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();

    svg2png(
        opts.inputdir.canonicalize()?,
        opts.outputdir.canonicalize()?,
    )?;

    Ok(())
}

lazy_static! {
    static ref RE_SVG: Regex = Regex::new(r".*\.svg$").unwrap();
}

fn svg2png(input: PathBuf, output: PathBuf) -> Result<()> {
    let opt = usvg::Options::default();
    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();

    let mut rng = rand::thread_rng();
    let dist: Uniform<f32> = Uniform::new(0., 360.);

    let inputs = fs::read_dir(input)?
        .into_iter()
        .filter_map(|ent| ent.ok().map(|ent| ent.path()))
        .collect::<Vec<_>>();
    for path in inputs.iter() {
        let opath = output.join(format!(
            "{}.png",
            path.file_stem()
                .ok_or(anyhow::anyhow!("no file stem"))?
                .to_owned()
                .into_string()
                .map_err(|_| anyhow::anyhow!("bad os string"))?
        ));
        match svg2png1(path.clone(), opath, &mut rng, &dist, &opt, &fontdb) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("error handling {}: {}", path.display(), e);
            }
        };
    }
    Ok(())
}

fn svg2png1(
    path: PathBuf,
    opath: PathBuf,
    rng: &mut ThreadRng,
    dist: &Uniform<f32>,
    opt: &Options,
    fontdb: &fontdb::Database,
) -> Result<()> {
    let svg_data = std::fs::read(path)?;
    let mut tree = usvg::Tree::from_data(&svg_data, &opt)?;
    tree.convert_text(&fontdb, opt.keep_named_groups);

    // set render size to $size - ($margin * 2)
    // set actual output size to $size
    // this gives icons with spacing around them
    let margin = 32;
    let size = 256;
    let pixmap_size = ScreenSize::new(size, size).ok_or(anyhow::anyhow!("screen size error"))?;
    let render_size = ScreenSize::new(size - (margin * 2), size - (margin * 2))
        .ok_or(anyhow::anyhow!("render size error"))?;

    // pixmap = render target for the svg itself
    // bgpixmap = render target for background which we then blend with $pixmap
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
    let mut bgpixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

    // randomly generate a hue
    let h = rng.sample(dist);
    // pastel-ize it a bit
    let hsl = Hsl::new(h, 0.75, 0.75);
    // generate a color sample and render it to background pixmap as full fill
    let color: Srgb = hsl.into_color();
    let c = Color::from_rgba(color.red, color.green, color.blue, 1.)
        .ok_or(anyhow::anyhow!("color create error"))?;
    bgpixmap.fill(c);

    // do the render
    resvg::render(
        &tree,
        usvg::FitTo::Size(render_size.width(), render_size.height()),
        tiny_skia::Transform::from_translate(margin as f32, margin as f32),
        pixmap.as_mut(),
    )
    .ok_or(anyhow::anyhow!("error rendering svg layer"))?;

    // composite
    bgpixmap
        .draw_pixmap(
            0,
            0,
            pixmap.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        )
        .ok_or(anyhow::anyhow!("error rendering svg layer onto background"))?;

    bgpixmap.save_png(opath)?;
    Ok(())
}
