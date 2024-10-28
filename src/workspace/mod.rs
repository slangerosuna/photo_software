use std::{io::Cursor, default::Default};

use egui::PaintCallbackInfo;
use egui_wgpu::CallbackTrait;
use image::{ImageReader, ImageFormat};
use serde::{Deserialize, Serialize};
use wgpu::*;

use crate::GpuDevice;

mod texture_serialization;

impl Workspace {
    pub fn load(
        path: &str,
        gpu: &GpuDevice
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read(path)?;
        let bincode_end = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let mut this = bincode::deserialize(&data[4..bincode_end])?;

        let data = &data[bincode_end..];
        let mut data = data.into_iter().map(|a| *a);

        loop {
            let len: Result<[u8; 4], _> = data.next_chunk::<4>();
            if len.is_err() { break; }

            let len: [u8; 4] = len.unwrap();
            let len = u32::from_le_bytes(len) as usize;

            let data = (&mut data).take(len).collect::<Vec<u8>>();

            let reader = Cursor::new(data);
            let reader = ImageReader::with_format(reader, ImageFormat::Png);

            let image = reader.decode()?.into_rgba8();
            let width = image.width();
            let height = image.height();

            #[cfg(debug_assertions)]
            print!("Creating input texture...\n");
            let input_texture = gpu.device.create_texture(&TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: width,
                    height: height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_SRC
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_DST,
                view_formats: &[TextureFormat::Rgba8Unorm],
            });

            #[cfg(debug_assertions)]
            print!("Writing input texture...\n");
            gpu.queue.write_texture(
                ImageCopyTexture {
                    texture: &input_texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                image.into_vec().as_slice(),
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width as u32),
                    rows_per_image: Some(height as u32),
                },
                Extent3d {
                    width: width,
                    height: height,
                    depth_or_array_layers: 1,
                },
            );
        }

        Ok(this)
    }

    fn save(
        &self,
        path: &str,
        gpu: &GpuDevice
    ) {

    }
}

#[derive(Serialize, Deserialize)]
pub struct Workspace {
    size: (u32, u32),
    zoom: f32,
    offset: (f32, f32),
    layers: Vec<LayerInfo>,

    #[serde(skip)]
    textures: Vec<Texture>,

    #[serde(skip)]
    output_texture: Option<Texture>,
}

#[derive(Serialize, Deserialize)]
pub struct LayerInfo {
    name: String,
    visible: bool,
    opacity: f32,
    blend_mode: BlendMode,
}

#[derive(Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    //TODO: Add and implement more blend modes
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            size: (512, 512),
            zoom: 1.0,
            offset: (0.0, 0.0),
            layers: Vec::new(),
            textures: Vec::new(),
            output_texture: None,
        }
    }
}

impl CallbackTrait for Workspace {
    fn paint<'a>(
        &'a self,
        info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &'a egui_wgpu::CallbackResources,
    ) {

    }
}