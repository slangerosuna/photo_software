#![allow(incomplete_features)]
#![allow(unused)] // TODO: remove when done
#![feature(generic_const_exprs)]
#![feature(iter_next_chunk)]
#![feature(iter_intersperse)]
#![feature(vec_into_raw_parts)]

pub mod kernel;
mod workspace;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use eframe::egui;
use egui::{load::SizedTexture, panel, Image, Layout, Pos2, Rect, Sense, TextureId, Vec2};
use egui_wgpu::{RenderState, WgpuConfiguration};
use tokio::runtime::Runtime;
use workspace::{LayerCreationInfo, Workspace};

fn main() -> eframe::Result {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .unwrap();

    let rt_arc = Arc::new(runtime);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: WgpuConfiguration {
            device_descriptor: Arc::new(|_| DeviceDescriptor {
                required_features: Features::default()
                    | Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                ..Default::default()
            }),
            ..Default::default()
        },

        ..Default::default()
    };

    eframe::run_native(
        "Joyful Create",
        options,
        Box::new(|cc| {
            let render_state = cc.wgpu_render_state.clone().unwrap();
            let gpu = rt_arc.block_on(GpuDevice::new(render_state)).unwrap();

            let mut workspace = Workspace {
                ..Default::default()
            };
            workspace.create_layer(
                LayerCreationInfo {
                    name: "Background".to_string(),
                    init_image: Some(image::open("other.png").unwrap().to_rgba8()),
                    ..Default::default()
                },
                &gpu,
                None,
            );
            workspace.build_output_texture(&gpu);

            let output_tex = workspace.register_output_texture(cc);
            let app = App::new(gpu, rt_arc.clone(), output_tex, workspace);

            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}

pub struct App {
    gpu: GpuDevice,
    runtime: Arc<Runtime>,
    output_tex: TextureId,
    workspace: Workspace,
}

impl App {
    pub fn new(
        gpu: GpuDevice,
        runtime: Arc<Runtime>,
        output_tex: TextureId,
        workspace: Workspace,
    ) -> Self {
        Self {
            gpu,
            runtime,
            output_tex,
            workspace,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.heading("Left Panel");
            ui.label("This is a simple egui app.");
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let size: (u32, u32) = self.workspace.size;
            let zoom: f32 = self.workspace.zoom;
            let size: Vec2 = Vec2::new(size.0 as f32 * zoom, size.1 as f32 * zoom);

            let pixel_at_center: (f32, f32) = self.workspace.pixel_at_center;
            let pixel_at_center: Vec2 =
                Vec2::new(pixel_at_center.0 as f32, pixel_at_center.1 as f32);
            let center: Vec2 = pixel_at_center * zoom;

            let image = Image::new((self.output_tex, size));

            let panel_rect = ui.min_rect();
            let panel_center = panel_rect.center();

            let top_left = panel_center - center;
            let bottom_right = top_left + size;

            let image_rect = Rect::from_min_max(top_left, bottom_right);

            let image = image.sense(Sense::click());
            if ui.put(image_rect, image).clicked() {
                let image = image::open("input.png").unwrap().to_rgba8();
                self.workspace.create_layer(
                    LayerCreationInfo {
                        init_image: Some(image),
                        blend_mode: "normal".to_string(),
                        opacity: 0.5,
                        ..Default::default()
                    },
                    &self.gpu,
                    None,
                );
                self.workspace.recalculate_output_texture(&self.gpu);
            }
        });
    }
}

use image::{GenericImageView, ImageBuffer, Rgba};
use wgpu::*;

pub struct GpuDevice {
    pub render_state: RenderState,
    pub shaders: HashMap<String, ShaderModule>,
}

#[inline]
pub fn pad_to_multiple_of_256(n: u32) -> u32 {
    (n + 255) & !255
}

fn gather_all_files(root: PathBuf) -> Vec<PathBuf> {
    let read_dir = std::fs::read_dir(root).unwrap();
    let mut files = Vec::new();

    for entry in read_dir {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            files.extend(gather_all_files(path));
        } else {
            files.push(path);
        }
    }

    files
}

impl GpuDevice {
    pub async fn new(render_state: RenderState) -> Option<Self> {
        let mut shaders = HashMap::new();
        let files = gather_all_files(PathBuf::from("./shaders"));
        for file in files {
            let file_extension = file.extension().unwrap().to_str().unwrap().to_string();
            if file_extension == "wgsl" {
                let module = render_state
                    .device
                    .create_shader_module(ShaderModuleDescriptor {
                        label: None,
                        source: ShaderSource::Wgsl(
                            std::fs::read_to_string(file.clone()).unwrap().into(),
                        ),
                    });
                let relative_file = file
                    .strip_prefix("./shaders")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .split('.')
                    .next()
                    .unwrap()
                    .to_string();
                #[cfg(debug_assertions)]
                print!("Loaded shader: {}\n", relative_file);
                shaders.insert(relative_file, module);
            }
        }

        Some(Self {
            render_state,
            shaders,
        })
    }

    pub async fn texture_to_image(
        &self,
        texture: &Texture,
        width: u32,
    ) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let size = texture.size();
        #[cfg(debug_assertions)]
        print!(
            "Converting texture to image with size {}x{}...\n",
            width, size.height
        );
        let buffer_size = (size.width * size.height * 4) as u64;
        let buffer_desc = BufferDescriptor {
            label: None,
            size: buffer_size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };
        let buffer = self.render_state.device.create_buffer(&buffer_desc);

        let mut encoder = self
            .render_state
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            ImageCopyBuffer {
                buffer: &buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(pad_to_multiple_of_256(4 * size.width)),
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );

        self.render_state.queue.submit(Some(encoder.finish()));
        let buffer_slice = buffer.slice(..);

        buffer_slice.map_async(MapMode::Read, |result| {
            if let Err(e) = result {
                eprintln!("Failed to map buffer: {:?}", e);
                return;
            }
        });
        self.render_state.device.poll(Maintain::Wait);

        let data = buffer_slice.get_mapped_range();

        let image = ImageBuffer::from_raw(size.width, size.height, data.to_vec()).unwrap();
        //crop off the padding
        image.view(0, 0, width, size.height).to_image()
    }
}
