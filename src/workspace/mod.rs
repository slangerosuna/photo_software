use std::{borrow::Borrow, default::Default, io::Cursor};

use eframe::CreationContext;
use egui::{Image, PaintCallbackInfo, TextureId};
use egui_wgpu::CallbackTrait;
use image::{ImageBuffer, ImageFormat, ImageReader, Rgba};
use serde::{Deserialize, Serialize};
use wgpu::*;

use crate::GpuDevice;

impl Workspace {
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
                        bytes_per_row: Some(super::pad_to_multiple_of_256(4 * size.width)),
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

#[derive(Serialize, Deserialize)]
pub struct Workspace {
    pub size: (u32, u32),
    pub zoom: f32,
    pub pixel_at_center: (f32, f32),
    pub layers: Vec<LayerInfo>,

    #[serde(skip)]
    pub textures: Vec<Texture>,

    #[serde(skip)]
    pub output_texture: Option<Texture>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LayerInfo {
    name: String,
    visible: bool,
    opacity: f32,
    blend_mode: BlendMode,
}

pub struct LayerCreationInfo {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub init_texture: Option<Texture>,
    pub init_image: Option<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    pub init_rgba: Option<[u8; 4]>,
}

impl Default for LayerCreationInfo {
    fn default() -> Self {
        Self {
            name: "New Layer".to_string(),
            visible: true,
            opacity: 1.0,
            blend_mode: "normal".to_string(),
            init_texture: None,
            init_image: None,
            init_rgba: None,
        }
    }
}

impl From<LayerCreationInfo> for LayerInfo {
    fn from(info: LayerCreationInfo) -> Self {
        Self {
            name: info.name,
            visible: info.visible,
            opacity: info.opacity,
            blend_mode: info.blend_mode,
        }
    }
}

pub type BlendMode = String;

impl Default for Workspace {
    fn default() -> Self {
        Self {
            size: (512, 512),
            zoom: 1.0,
            pixel_at_center: (256.0, 256.0),
            layers: Vec::new(),
            textures: Vec::new(),
            output_texture: None,
        }
    }
}

impl Workspace {
    pub fn move_layer(&mut self, from: usize, to: usize) {
        let info = self.layers.remove(from);
        self.layers.insert(to, info);

        let texture = self.textures.remove(from);
        self.textures.insert(to, texture);
    }

    pub fn create_layer(
        &mut self,
        mut info: LayerCreationInfo,
        gpu: &GpuDevice,
        index: Option<usize>,
    ) {
        let texture = if info.init_texture.is_some() {
            info.init_texture.take().unwrap()
        } else {
            let texture_data = if info.init_image.is_some() {
                info.init_image.take().unwrap().into_vec()
            } else {
                let init_rgba: [u8; 4] = info.init_rgba.take().unwrap_or([255, 255, 255, 0]);
                let tex_data: Vec<[u8; 4]> = vec![init_rgba; (self.size.0 * self.size.1) as usize];

                let (ptr, len, capacity) = tex_data.into_raw_parts();

                let new_len = len * 4;
                let new_capacity = capacity * 4;

                unsafe { Vec::from_raw_parts(ptr as *mut u8, new_len, new_capacity) }
            };

            let layer_texture = gpu.render_state.device.create_texture(&TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: self.size.0,
                    height: self.size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::STORAGE_BINDING,
                view_formats: &[TextureFormat::Rgba8Unorm],
            });

            gpu.render_state.queue.write_texture(
                ImageCopyTexture {
                    texture: &layer_texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &texture_data,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * self.size.0),
                    rows_per_image: Some(self.size.1),
                },
                Extent3d {
                    width: self.size.0,
                    height: self.size.1,
                    depth_or_array_layers: 1,
                },
            );

            layer_texture
        };
        #[cfg(debug_assertions)]
        assert_eq!(
            None, info.init_image,
            "cannot have both init_texture and init_image"
        );
        #[cfg(debug_assertions)]
        assert_eq!(
            None, info.init_rgba,
            "cannot have both init_texture and init_rgba"
        );

        match index {
            Some(index) => {
                self.layers.insert(index, info.into());
                self.textures.insert(index, texture);
            }
            None => {
                self.layers.push(info.into());
                self.textures.push(texture);
            }
        }
    }

    pub fn build_output_texture(&mut self, gpu: &GpuDevice) {
        self.output_texture = Some(gpu.render_state.device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: self.size.0,
                height: self.size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::STORAGE_BINDING,
            view_formats: &[TextureFormat::Rgba8Unorm],
        }));

        self.recalculate_output_texture(gpu);
    }

    pub fn recalculate_output_texture(&mut self, gpu: &GpuDevice) {
        let empty_texture_data = vec![0u8; (self.size.0 * self.size.1 * 4) as usize];
        gpu.render_state.queue.write_texture(
            ImageCopyTexture {
                texture: self.output_texture.as_ref().unwrap(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &empty_texture_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.size.0),
                rows_per_image: Some(self.size.1),
            },
            Extent3d {
                width: self.size.0,
                height: self.size.1,
                depth_or_array_layers: 1,
            },
        );

        for i in 0..self.layers.len() {
            let layer_info = &self.layers[i];
            if !layer_info.visible {
                continue;
            }

            let layer_texture = &self.textures[i];

            #[cfg(debug_assertions)]
            println!("Applying layer {:?}...", layer_info);

            let shader = gpu
                .shaders
                .get(format!("blend_modes/{}", layer_info.blend_mode).as_str())
                .unwrap();

            let bind_group_layout =
                gpu.render_state
                    .device
                    .create_bind_group_layout(&BindGroupLayoutDescriptor {
                        entries: &[
                            BindGroupLayoutEntry {
                                binding: 0,
                                visibility: ShaderStages::COMPUTE,
                                ty: BindingType::StorageTexture {
                                    access: StorageTextureAccess::ReadOnly,
                                    format: TextureFormat::Rgba8Unorm,
                                    view_dimension: TextureViewDimension::D2,
                                },
                                count: None,
                            },
                            BindGroupLayoutEntry {
                                binding: 1,
                                visibility: ShaderStages::COMPUTE,
                                ty: BindingType::StorageTexture {
                                    access: StorageTextureAccess::ReadWrite,
                                    format: TextureFormat::Rgba8Unorm,
                                    view_dimension: TextureViewDimension::D2,
                                },
                                count: None,
                            },
                            BindGroupLayoutEntry {
                                binding: 2,
                                visibility: ShaderStages::COMPUTE,
                                ty: BindingType::Buffer {
                                    ty: BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: Some(std::num::NonZeroU64::new(std::mem::size_of::<f32>() as u64).unwrap()),
                                },
                                count: None,
                            },
                        ],
                        label: None,
                    });
            let opacity = layer_info.opacity;
            let data = vec![opacity];
            let buffer = gpu.render_state.device.create_buffer(&BufferDescriptor {
                label: None,
                size: (data.len() * std::mem::size_of::<f32>()) as u64,
                usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });

            gpu.render_state
                .queue
                .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));
            let bind_group = gpu
                .render_state
                .device
                .create_bind_group(&BindGroupDescriptor {
                    layout: &bind_group_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(
                                &layer_texture.create_view(&TextureViewDescriptor::default()),
                            ),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(
                                &self
                                    .output_texture
                                    .as_ref()
                                    .unwrap()
                                    .create_view(&TextureViewDescriptor::default()),
                            ),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: BindingResource::Buffer(BufferBinding {
                                buffer: &buffer,
                                offset: 0,
                                size: None,
                            }),
                        },
                    ],
                    label: None,
                });

            let pipeline_layout =
                gpu.render_state
                    .device
                    .create_pipeline_layout(&PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let pipeline =
                gpu.render_state
                    .device
                    .create_compute_pipeline(&ComputePipelineDescriptor {
                        label: None,
                        layout: Some(&pipeline_layout),
                        module: shader,
                        entry_point: "main",
                        compilation_options: Default::default(),
                        cache: None,
                    });

            let mut encoder = gpu
                .render_state
                .device
                .create_command_encoder(&CommandEncoderDescriptor { label: None });
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups((self.size.0 + 7) / 8, (self.size.1 + 7) / 8, 1);
            }
            gpu.render_state.queue.submit(Some(encoder.finish()));
        }
    }
    pub fn register_output_texture(&self, cc: &CreationContext) -> TextureId {
        let renderer = cc.wgpu_render_state.as_ref().unwrap().renderer.clone();
        let device = cc.wgpu_render_state.as_ref().unwrap().device.clone();
        let texture_view = self
            .output_texture
            .as_ref()
            .unwrap()
            .create_view(&TextureViewDescriptor::default());
        let mut renderer = renderer.write();
        let id =
            renderer.register_native_texture(device.borrow(), &texture_view, FilterMode::Linear);
        drop(renderer);
        id
    }
}
