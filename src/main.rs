use anyhow::Ok;
use anyhow::Result;
use file_format::FileFormat;
use neoffmpeg_rs as ffmpeg;
use neoffmpeg_rs::format::Pixel;
use rfd::AsyncFileDialog;
use rfd::AsyncMessageDialog;
use slint::Image;
use std::path::Path;
mod player;
slint::include_modules!();
fn main() -> Result<()> {
    smol::block_on(async {
        let app = Main::new().expect("Failed to create Slint Runtime");
        let app_weak = app.as_weak();
        let app_weak_two = app.as_weak();
        app.on_Message(move || {
            smol::spawn(async {
                AsyncMessageDialog::new()
                    .set_title("CatMP: v0.1\n Made with ❤️")
                    .show()
                    .await;
            })
            .detach();
        });
        app.on_filemanager(move || {
            let weak = app_weak.clone();
            smol::spawn(async move {
                let file = AsyncFileDialog::new()
                    .add_filter(
                        "Media",
                        &[
                            "mp4", "mp3", "png", "jpeg", "jpg", "mkv", "flv", "ogv", "ogg", "rrc",
                            "gifv", "mng", "mov", "avi", "qt", "wmv", "yuv", "rm", "asf", "amv",
                            "m4p", "m4v", "mpg", "mp2", "mpeg", "mpe", "mpv", "3gp", "3g2", "mxf",
                            "roq", "nsv", "f4v", "f4p", "f4a", "f4b", "mod",
                        ],
                    )
                    .pick_file()
                    .await
                    .unwrap();
                // TODO: fix a bug where it panics here if you press cancel in the rfd window the
                // app will panic
                let path = file.path().to_string_lossy().to_string();
                let path_clone = path.clone();
                weak.upgrade_in_event_loop(move |app| {
                    app.set_file(path.into());
                    let path = Path::new(&path_clone);
                    let fmt = FileFormat::from_file(&path).unwrap();
                    // TODO: add more image types "https://github.com/slint-ui/slint/discussions/7218"
                    if fmt.extension() == "png"
                        || fmt.extension() == "jpeg"
                        || fmt.extension() == "jpg"
                        || fmt.extension() == "svg"
                    {
                        let image = Image::load_from_path(&path).unwrap();
                        app.set_frame(image);
                        app.set_is_image(true);
                    } else {
                        app.set_is_image(false);
                    }
                })
                .unwrap();
            })
            .detach();
        });

        app.on_play(move || {
            app_weak_two
                .upgrade_in_event_loop(move |app| {
                    println!("hi");
                    let path = app.get_file().to_string();
                    let _volume = app.get_volume() as f32 / 100.0;
                    let playing = app.get_playing();
                    let mut to_rgba_rescaler: Option<ffmpeg::software::scaling::Context> = None;
                    if playing && !path.is_empty() && !app.get_is_image() {
                        let mut player = player::Player::start(path.into(), {
                            let app_weak = app.as_weak();
                            move |new_frame| {
                                // TODO: use OpenGL bridge

                                let rebuild_rescaler = to_rgba_rescaler.as_ref().is_none_or(
                                    |existing_rescaler: &ffmpeg::software::scaling::Context| {
                                        existing_rescaler.input().format != new_frame.format()
                                    },
                                );

                                if rebuild_rescaler {
                                    to_rgba_rescaler = Some(rgba_rescaler_for_frame(new_frame));
                                }

                                let rescaler = to_rgba_rescaler.as_mut().unwrap();

                                let mut rgb_frame = ffmpeg::util::frame::Video::empty();
                                rescaler.run(new_frame, &mut rgb_frame).unwrap();

                                let pixel_buffer = video_frame_to_pixel_buffer(&rgb_frame);
                                app_weak
                                    .upgrade_in_event_loop(|app| {
                                        app.set_frame(slint::Image::from_rgb8(pixel_buffer))
                                    })
                                    .unwrap();
                            }
                        })
                        .unwrap();

                        app.on_play(move || {
                            player.toggle_pause_playing();
                        });
                    }
                })
                .unwrap();
        });
        app.run().expect("Failed to run Slint Runtime");
    });
    Ok(())
}

fn rgba_rescaler_for_frame(
    frame: &ffmpeg::util::frame::Video,
) -> ffmpeg::software::scaling::Context {
    ffmpeg::software::scaling::Context::get(
        frame.format(),
        frame.width(),
        frame.height(),
        Pixel::RGB24,
        frame.width(),
        frame.height(),
        ffmpeg::software::scaling::Flags::BILINEAR,
    )
    .unwrap()
}

fn video_frame_to_pixel_buffer(
    frame: &ffmpeg::util::frame::Video,
) -> slint::SharedPixelBuffer<slint::Rgb8Pixel> {
    let mut pixel_buffer =
        slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(frame.width(), frame.height());

    let ffmpeg_line_iter = frame.data(0).chunks_exact(frame.stride(0));
    let slint_pixel_line_iter = pixel_buffer
        .make_mut_bytes()
        .chunks_mut(frame.width() as usize * core::mem::size_of::<slint::Rgb8Pixel>());

    for (source_line, dest_line) in ffmpeg_line_iter.zip(slint_pixel_line_iter) {
        dest_line.copy_from_slice(&source_line[..dest_line.len()])
    }

    pixel_buffer
}
