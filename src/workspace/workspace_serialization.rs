use std::io::Cursor;

use image::{ImageEncoder, ImageFormat, ImageReader};
use wgpu::*;

use super::Workspace;
use crate::device::{pad_to_multiple_of_256, GpuDevice};
use crate::workspace::LayerData;

impl Workspace<'_> {
    pub fn load(path: &str, gpu: &GpuDevice) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(debug_assertions)]
        println!("Loading workspace at {}...", path);

        let data = std::fs::read(path)?;
        let bincode_end = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize + 4;
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

            let mdata = (&mut data).take(len).collect::<Vec<u8>>();

            let reader = Cursor::new(mdata);
            let reader = ImageReader::with_format(reader, ImageFormat::Png);

            let image = reader.decode()?.into_rgba8();
            println!("c");
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

            let mask_len: [u8; 4] = (&mut data)
                .next_chunk::<4>()
                .expect("layer not provided with mask");
            let mask_len = u32::from_le_bytes(mask_len) as usize;

            let mask_data = (&mut data).take(mask_len).collect::<Vec<u8>>();

            let reader = Cursor::new(mask_data);
            let reader = ImageReader::with_format(reader, ImageFormat::Png);

            let mask_image = reader.decode()?.into_luma8();

            let mask_width = mask_image.width();
            let mask_height = mask_image.height();

            #[cfg(debug_assertions)]
            print!("Creating mask texture...\n");
            let mask_texture = gpu.render_state.device.create_texture(&TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: mask_width,
                    height: mask_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R8Unorm,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_SRC
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_DST,
                view_formats: &[TextureFormat::R8Unorm],
            });

            #[cfg(debug_assertions)]
            print!("Writing mask texture...\n");
            gpu.render_state.queue.write_texture(
                ImageCopyTexture {
                    texture: &mask_texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                mask_image.into_vec().as_slice(),
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(mask_width as u32),
                    rows_per_image: Some(mask_height as u32),
                },
                Extent3d {
                    width: mask_width,
                    height: mask_height,
                    depth_or_array_layers: 1,
                },
            );

            #[cfg(debug_assertions)]
            print!("Creating layer running total texture...\n");
            let running_total = gpu.render_state.device.create_texture(&TextureDescriptor {
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

            let layer_data = LayerData {
                texture: input_texture,
                mask: mask_texture,
                running_total: running_total,
            };
            let layer_data = Box::new(layer_data);

            this.layer_data.push(layer_data);
        }

        this.build_output_texture(gpu);

        Ok(this)
    }

    pub async fn save(&self, path: &str, gpu: &GpuDevice) {
        #[cfg(debug_assertions)]
        println!("Saving workspace at {}...", path);

        let mut data = bincode::serialize(&self).unwrap();
        let len = data.len() as u32;
        let len_bytes = len.to_le_bytes();
        let mut data = len_bytes
            .iter()
            .copied()
            .chain(data.into_iter())
            .collect::<Vec<u8>>();
        let mut images = Vec::new();

        for layer in self.layer_data.iter() {
            {
                let image = gpu.texture_to_image(&layer.texture, self.size.0).await;

                let mut data = Vec::new();
                let encoder = image::codecs::png::PngEncoder::new_with_quality(
                    &mut data,
                    image::codecs::png::CompressionType::Best,
                    image::codecs::png::FilterType::NoFilter,
                );

                encoder
                    .write_image(
                        &image.into_vec(),
                        self.size.0,
                        self.size.1,
                        image::ExtendedColorType::Rgba8,
                    )
                    .unwrap();

                images.push(data.to_vec());
            }
            {
                let mask_image = todo!();
            }
        }

        for image in images {
            let len = image.len() as u32;
            let len = len.to_le_bytes();
            data.extend_from_slice(&len);
            data.extend_from_slice(&image);
        }

        std::fs::write(path, data).unwrap();
    }
}
