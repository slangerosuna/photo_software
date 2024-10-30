use std::{collections::HashMap, path::PathBuf};

use egui_wgpu::RenderState;
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

        let shaders_dir = std::env::current_exe().expect("Can't find path to executable");
        let shaders_dir = format!("{}/joyful_create_shaders", shaders_dir.parent().unwrap().display());
        let files = gather_all_files(PathBuf::from(&shaders_dir));

        println!("Files: {:?}", files);

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
                    .strip_prefix(&shaders_dir)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
                    .strip_suffix(".wgsl")
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
