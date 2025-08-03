//! Do you wanna be a BIG SHOT?
//!
//! Do `cargo install bigshot`, bind `PrintScr` to `bigshot` and press Print Screen!
//!
//! `bigshot` is a quick alterative to `flameshot` because `flameshot` doesn't work on
//! `Hyprland`

//mod drv;

use std::{
    io::{Cursor, Write},
    mem::transmute,
    process::{Command, Stdio},
    time::Duration,
};

use image::{io::Reader, EncodableLayout, GenericImage};
use sdl3::{keyboard::Keycode, mouse::MouseButton, pixels::Color, render::Texture};

#[derive(Clone)]
struct Region {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}
impl Region {
    pub fn dot(x: u32, y: u32) -> Self {
        Self {
            x1: x,
            y1: y,
            x2: x,
            y2: y,
        }
    }

    pub fn stretch(&mut self, x: u32, y: u32) {
        self.x2 = x;
        self.y2 = y;
    }

    pub fn normalize(&mut self) {
        if self.x1 > self.x2 {
            std::mem::swap(&mut self.x1, &mut self.x2);
        }
        if self.y1 > self.y2 {
            std::mem::swap(&mut self.y1, &mut self.y2);
        }
    }

    pub fn is_zero(&self) -> bool {
        self.x1 == self.x2 || self.y1 == self.y2
    }
}

fn main() {
    // This may be racy, but I just want sdl3 to work
    unsafe {
        std::env::set_var("DISPLAY", "");
    }

    let mut pic = {
        let output = Command::new("grim")
            .args(["-c", "-t", "png", "-"])
            .output()
            .expect("failed to run grim");

        if !output.status.success() {
            panic!("{}", String::from_utf8_lossy(&output.stderr));
        }

        Reader::new(Cursor::new(output.stdout))
            .with_guessed_format()
            .expect("failed to guess format")
            .decode()
            .expect("image decoding error")
    };

    eprintln!("image format: {:?}", pic.color());

    let sdl = sdl3::init().expect("failed to initialize sdl");
    let video = sdl.video().expect("failed to initialize sdl video");
    let mut events = sdl.event_pump().expect("failed to initialize sdl events");

    let (min_x, min_y) = video
        .displays()
        .unwrap()
        .iter()
        .map(|x| x.get_bounds().unwrap())
        .map(|x| (x.x(), x.y()))
        .reduce(|x, a| (x.0.min(a.0), x.1.min(a.1)))
        .unwrap();

    let mut windows = {
        let mut windows = vec![];
        for display in video.displays().unwrap() {
            let mut window = video
                .window("bigshot", pic.width(), pic.height())
                .maximized()
                .borderless()
                .fullscreen()
                .build()
                .expect("failed to create sdl window")
                .into_canvas();

            let texture_creator = window.texture_creator();
            let rect = display.get_usable_bounds().unwrap();

            let texture = {
                let rgb8 = pic
                    .sub_image(
                        (rect.x() - min_x) as u32,
                        (rect.y() - min_y) as u32,
                        rect.width(),
                        rect.height(),
                    )
                    .to_image();
                let mut tex = texture_creator
                    .create_texture_static(
                        unsafe {
                            sdl3::pixels::PixelFormat::from_ll(
                                sdl3::pixels::PixelFormatEnum::ABGR8888.to_ll(),
                            )
                        },
                        rect.width(),
                        rect.height(),
                    )
                    .expect("failed to create texture");
                tex.update(None, rgb8.as_bytes(), rgb8.width() as usize * 4)
                    .unwrap();
                tex
            };
            let texture: Texture<'static> = unsafe { transmute(texture) };

            window
                .window_mut()
                .set_display_mode(display.get_mode().unwrap())
                .unwrap();
            window.window_mut().set_fullscreen(true).unwrap();

            windows.push((window, texture, (rect.x(), rect.y())));
        }
        windows
    };

    let mut region = None;
    let mut selecting_region = false;
    let mut terminate = true;

    'app: loop {
        while let Some(x) = events.poll_event() {
            use sdl3::event::Event as E;
            match x {
                E::Quit { .. } => break 'app,
                E::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    window_id,
                    ..
                } => {
                    let shift = windows
                        .iter()
                        .find(|x| x.0.window().id() == window_id)
                        .map(|x| x.2)
                        .unwrap();
                    region = Some(Region::dot(
                        (x + shift.0 as f32) as u32,
                        (y + shift.1 as f32) as u32,
                    ));
                    selecting_region = true;
                }
                E::MouseMotion {
                    x, y, window_id, ..
                } => {
                    let shift = windows
                        .iter()
                        .find(|x| x.0.window().id() == window_id)
                        .map(|x| x.2)
                        .unwrap();
                    if selecting_region {
                        if let Some(r) = &mut region {
                            r.stretch((x + shift.0 as f32) as u32, (y + shift.1 as f32) as u32);
                        }
                    }
                }
                E::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    window_id,
                    ..
                } => {
                    let shift = windows
                        .iter()
                        .find(|x| x.0.window().id() == window_id)
                        .map(|x| x.2)
                        .unwrap();
                    if let Some(r) = &mut region {
                        r.stretch((x + shift.0 as f32) as u32, (y + shift.1 as f32) as u32);
                        selecting_region = false;
                    }
                }
                E::KeyDown {
                    keycode: Some(Keycode::Return) | Some(Keycode::Return2),
                    ..
                } => {
                    terminate = false;
                    break 'app;
                }
                _ => (),
            }
        }

        for (window, texture, shift) in &mut windows {
            window.copy(texture, None, None).unwrap();
            if let Some(mut x) = region.clone() {
                x.normalize();
                window.set_draw_color(Color::MAGENTA);
                window
                    .draw_rect(sdl3::render::FRect {
                        x: x.x1 as f32 - shift.0 as f32,
                        y: x.y1 as f32 - shift.1 as f32,
                        w: x.x2 as f32 - x.x1 as f32 + 1.0,
                        h: x.y2 as f32 - x.y1 as f32 + 1.0,
                    })
                    .unwrap();
            }
            window.present();
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    if terminate {
        return;
    }

    if let Some(mut x) = region {
        x.normalize();
        if x.is_zero() {
            return;
        }

        let image = pic.sub_image(x.x1, x.y1, x.x2 - x.x1, x.y2 - x.y1);
        std::fs::remove_file("/tmp/doyouwannabeabigshot-tmp-img.png").ok();
        image
            .to_image()
            .save("/tmp/doyouwannabeabigshot-tmp-img.png")
            .unwrap();
        let buf = std::fs::read("/tmp/doyouwannabeabigshot-tmp-img.png").unwrap();
        std::fs::remove_file("/tmp/doyouwannabeabigshot-tmp-img.png").unwrap();

        let mut proc = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .spawn()
            .expect("failed to spawn wl-copy");

        let mut stdin = proc.stdin.take().unwrap();
        stdin.write_all(&buf).expect("failed to write to wl-copy");
        drop(stdin);

        if !proc.wait().unwrap().success() {
            panic!("failed to execute wl-copy");
        }
    }
}
