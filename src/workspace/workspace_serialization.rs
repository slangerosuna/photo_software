use std::io::Cursor;

use image::{ImageFormat, ImageReader};
use wgpu::*;

use super::Workspace;
use crate::device::{pad_to_multiple_of_256, GpuDevice};

impl Workspace<'_> {
    pub fn load(path: &str, gpu: &GpuDevice) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read(path)?;
        let bincode_end = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let mut this: Self = bincode::deserialize(&data[4..bincode_end])?;

        let data = &data[bincode_end..];
        let mut data = data.into_iter().map(|a| *a);

        loop {
            let len: Result<[u8; 4], _> = data.next_chunk::<4>();
            if len.is_err() {
                break;
            }

            let len: [u8; 4] = len.unwrap();
            let len = u32::from_le_bytes(len) as usize;

            let data = (&mut data).take(len).collect::<Vec<u8>>();

            let reader = Cursor::new(data);
            let reader = ImageReader::with_format(reader, ImageFormat::Png);

            let image = reader.decode()?.into_rgba8();
            let width = image.width();
            let height = image.height();

            #[cfg(debug_assertions)]
            print!("Creating layer texture...\n");
            let input_texture = gpu.render_state.device.create_texture(&TextureDescriptor {
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
            print!("Writing layer texture...\n");
            gpu.render_state.queue.write_texture(
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

        this.build_output_texture(gpu);

        Ok(this)
    }

    fn save(&self, path: &str, gpu: &GpuDevice) {
        let mut data = bincode::serialize(&self).unwrap();
        let mut images = Vec::new();

        for texture in &self.textures {
            let size = self.size;
            let size: Extent3d = Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            };
            let buffer_size = (size.width * size.height * 4) as u64;
            let buffer_desc = BufferDescriptor {
                label: None,
                size: buffer_size,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            };
            let buffer = gpu.render_state.device.create_buffer(&buffer_desc);

            let mut encoder = gpu
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

            gpu.render_state.queue.submit(Some(encoder.finish()));
            let buffer_slice = buffer.slice(..);

            buffer_slice.map_async(MapMode::Read, |result| {
                if let Err(e) = result {
                    eprintln!("Failed to map buffer: {:?}", e);
                    return;
                }
            });
            gpu.render_state.device.poll(Maintain::Wait);

            let data = buffer_slice.get_mapped_range();

            images.push(data.to_vec());
        }

        for image in images {
            let len = image.len();
            let len = len.to_le_bytes();
            data.extend_from_slice(&len);
            data.extend_from_slice(&image);
        }

        let len = data.len();
        let len = len.to_le_bytes();
        data.splice(0..0, len.iter().copied());

        std::fs::write(path, data).unwrap();
    }
}
