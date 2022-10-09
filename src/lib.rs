mod ray;
mod scene;
mod shapes;
mod thread_pool;

use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{mpsc, Arc},
};

use nalgebra::{Point3, Unit, Vector2, Vector3};

use ray::Ray;
use scene::Scene;

pub fn run() -> Result<(), Box<dyn Error>> {
    let scene: Arc<Scene> = Arc::new(Scene::open("scenes/cornell.toml")?);

    let workers = std::thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());
    let (sender, receiver) = mpsc::sync_channel(workers.get() * 2);

    let pool = thread_pool::Static::build(workers)?;
    for _ in 0..scene.config.samples {
        let sender = sender.clone();
        let scene = scene.clone();
        pool.submit(move || sender.send(pathtrace_sample(&scene)).unwrap());
    }

    drop(sender);

    let mut image = vec![[0.0, 0.0, 0.0]; (scene.config.width * scene.config.height) as usize];
    while let Ok(sample) = receiver.recv() {
        for (image_pixel, sample_pixel) in image.iter_mut().zip(sample) {
            for (image_color, sample_color) in image_pixel.iter_mut().zip(sample_pixel) {
                *image_color += sample_color;
            }
        }
    }
    for pixel in image.iter_mut() {
        for color in pixel.iter_mut() {
            *color /= scene.config.samples as f64;
        }
    }

    let file = std::fs::File::create("output.png")?;
    let mut encoder =
        png::Encoder::new(file, scene.config.width as u32, scene.config.height as u32);
    encoder.set_color(png::ColorType::Rgb);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(
        &image
            .iter()
            .flatten()
            .map(|color| (color * 255.0) as u8)
            .collect::<Vec<u8>>(),
    )?;
    writer.finish()?;

    Ok(())
}

fn pathtrace_sample(scene: &Scene) -> Vec<[f64; 3]> {
    (0..scene.config.height)
        .flat_map(|y| (0..scene.config.width).map(move |x| pathtrace_pixel(scene, x, y)))
        .collect()
}

fn pathtrace_pixel(scene: &Scene, x: i32, y: i32) -> [f64; 3] {
    let camera_position = Point3::from(scene.config.camera.position);

    let pixel_on_screen = Vector2::new(
        x as f64 / scene.config.width as f64,
        y as f64 / scene.config.height as f64,
    );

    let ray = Ray {
        position: camera_position,
        direction: Unit::new_normalize(Vector3::new(pixel_on_screen[0], pixel_on_screen[1], -1.0)),
    };

    radiance(scene, &ray)
}

fn radiance(scene: &Scene, ray: &Ray) -> [f64; 3] {
    let closest_match = scene
        .objects
        .iter()
        .filter_map(|o| Some((o, o.shape.intersect(ray)?)))
        .min_by(|(_, a), (_, b)| a.distance.partial_cmp(&b.distance).unwrap());

    if closest_match.is_none() {
        return scene.config.background_color;
    }
    let (object, intersection) = closest_match.unwrap();

    object.diffusion
}
