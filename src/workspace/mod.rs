use eframe::CreationContext;
use egui::{Image, PaintCallbackInfo, TextureId};
use egui_wgpu::CallbackTrait;
use image::{ImageBuffer, ImageFormat, ImageReader, Rgba};
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, default::Default, io::Cursor};
use util::{DeviceExt, TextureDataOrder};
use wgpu::*;

pub mod layer_info;
pub mod tools;
pub mod workspace_serialization;

use layer_info::*;
use tools::*;
pub use workspace_serialization::*;

use crate::GpuDevice;

#[derive(Serialize, Deserialize)]
pub struct Workspace<'a> {
    pub size: (u32, u32),
    pub zoom: f32,
    pub pixel_at_center: (f32, f32),
    pub layers: Vec<LayerInfo>,

    #[serde(skip)]
    pub layer_data: Vec<Box<LayerData>>,

    #[serde(skip)]
    pub output_texture: Option<Texture>,

    #[serde(skip)]
    pub selected_tool: &'a dyn Tool,

    #[serde(skip)]
    pub eternal_blank: Option<Texture>,
}

impl Default for Workspace<'_> {
    fn default() -> Self {
        Self {
            size: (512, 512),
            zoom: 1.0,
            pixel_at_center: (256.0, 256.0),
            layers: Vec::new(),
            layer_data: Vec::new(),
            output_texture: None,
            selected_tool: &SelectTool,
            eternal_blank: None,
        }
    }
}

pub struct LayerData {
    texture: Texture,
    mask: Texture,
    running_total: Texture,
}

impl Workspace<'_> {
    pub fn move_layer(&mut self, from: usize, to: usize, gpu: &GpuDevice) {
        let info = self.layers.remove(from);
        self.layers.insert(to, info);

        let layer_data = self.layer_data.remove(from);
        self.layer_data.insert(to, layer_data);

        self.recalculate_output_texture(gpu, if from < to { from } else { to });
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
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_SRC,
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

        let mask = if info.init_mask_texture.is_some() {
            info.init_mask_texture.take().unwrap()
        } else if info.init_mask_image.is_some() {
            let mask_data = info.init_mask_image.take().unwrap().into_vec();

            let mask_texture = gpu.render_state.device.create_texture(&TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: self.size.0,
                    height: self.size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R8Unorm,
                usage: TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_SRC,
                view_formats: &[TextureFormat::R8Unorm],
            });

            gpu.render_state.queue.write_texture(
                ImageCopyTexture {
                    texture: &mask_texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &mask_data,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.size.0),
                    rows_per_image: Some(self.size.1),
                },
                Extent3d {
                    width: self.size.0,
                    height: self.size.1,
                    depth_or_array_layers: 1,
                },
            );

            mask_texture
        } else {
            // fill with 100% opacity
            let mask_data: Vec<u8> = vec![255; (self.size.0 * self.size.1) as usize];
            gpu.render_state.device.create_texture_with_data(
                &gpu.render_state.queue,
                &TextureDescriptor {
                    label: None,
                    size: Extent3d {
                        width: self.size.0,
                        height: self.size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::R8Unorm,
                    usage: TextureUsages::RENDER_ATTACHMENT
                        | TextureUsages::TEXTURE_BINDING
                        | TextureUsages::COPY_DST
                        | TextureUsages::STORAGE_BINDING
                        | TextureUsages::COPY_SRC,
                    view_formats: &[TextureFormat::R8Unorm],
                },
                TextureDataOrder::LayerMajor,
                &mask_data,
            )
        };

        let running_total = gpu.render_state.device.create_texture(&TextureDescriptor {
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
                | TextureUsages::STORAGE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[TextureFormat::Rgba8Unorm],
        });

        let layer_data = LayerData {
            texture,
            mask,
            running_total,
        };
        let layer_data = Box::new(layer_data);

        match index {
            Some(index) => {
                self.layers.insert(index, info.into());
                self.layer_data.insert(index, layer_data);
                self.recalculate_output_texture(gpu, index);
            }
            None => {
                self.layers.push(info.into());
                self.layer_data.push(layer_data);
                self.recalculate_output_texture(gpu, self.layers.len() - 1);
            }
        }
    }

    pub fn build_output_texture(&mut self, gpu: &GpuDevice) {
        #[cfg(debug_assertions)]
        println!("Creating eternal blank texture...");
        self.eternal_blank = Some(gpu.render_state.device.create_texture(&TextureDescriptor {
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
                | TextureUsages::COPY_SRC
                | TextureUsages::STORAGE_BINDING,
            view_formats: &[TextureFormat::Rgba8Unorm],
        }));

        #[cfg(debug_assertions)]
        println!("Blanking eternal blank texture...");
        let clear_color = Rgba([0, 0, 0, 0]);
        let mut encoder = gpu
            .render_state
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self
                        .eternal_blank
                        .as_ref()
                        .unwrap()
                        .create_view(&TextureViewDescriptor::default()),
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        gpu.render_state.queue.submit(Some(encoder.finish()));

        #[cfg(debug_assertions)]
        println!("Creating output texture...");
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

        self.recalculate_output_texture(gpu, 0);
    }

    pub fn recalculate_output_texture(&mut self, gpu: &GpuDevice, start: usize) {
        if self.output_texture.is_none() {
            self.build_output_texture(gpu);
            return;
        }
        for i in start..self.layers.len() {
            let layer_info = &self.layers[i];
            if !layer_info.visible {
                continue;
            }

            let layer_texture = &self.layer_data[i].texture;
            let mask = &self.layer_data[i].mask;

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
                                    access: StorageTextureAccess::ReadOnly,
                                    format: TextureFormat::Rgba8Unorm,
                                    view_dimension: TextureViewDimension::D2,
                                },
                                count: None,
                            },
                            BindGroupLayoutEntry {
                                binding: 2,
                                visibility: ShaderStages::COMPUTE,
                                ty: BindingType::StorageTexture {
                                    access: StorageTextureAccess::WriteOnly,
                                    format: TextureFormat::Rgba8Unorm,
                                    view_dimension: TextureViewDimension::D2,
                                },
                                count: None,
                            },
                            BindGroupLayoutEntry {
                                binding: 3,
                                visibility: ShaderStages::COMPUTE,
                                ty: BindingType::StorageTexture {
                                    access: StorageTextureAccess::ReadOnly,
                                    format: TextureFormat::R8Unorm,
                                    view_dimension: TextureViewDimension::D2,
                                },
                                count: None,
                            },
                            BindGroupLayoutEntry {
                                binding: 4,
                                visibility: ShaderStages::COMPUTE,
                                ty: BindingType::Buffer {
                                    ty: BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size:
                                        Some(
                                            std::num::NonZeroU64::new(
                                                std::mem::size_of::<f32>() as u64
                                            )
                                            .unwrap(),
                                        ),
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
                            resource: BindingResource::TextureView(&if start == 0 {
                                self.eternal_blank
                                    .as_ref()
                                    .unwrap()
                                    .create_view(&TextureViewDescriptor::default())
                            } else {
                                self.layer_data[i - 1]
                                    .running_total
                                    .create_view(&TextureViewDescriptor::default())
                            }),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: BindingResource::TextureView(
                                &self.layer_data[i]
                                    .running_total
                                    .create_view(&TextureViewDescriptor::default()),
                            ),
                        },
                        BindGroupEntry {
                            binding: 3,
                            resource: BindingResource::TextureView(
                                &mask.create_view(&TextureViewDescriptor::default()),
                            ),
                        },
                        BindGroupEntry {
                            binding: 4,
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

        let last_layer = &self.layer_data[self.layers.len() - 1];
        let mut encoder = gpu
            .render_state
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_texture(
            ImageCopyTexture {
                texture: &last_layer.running_total,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            ImageCopyTexture {
                texture: self.output_texture.as_ref().unwrap(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            Extent3d {
                width: self.size.0,
                height: self.size.1,
                depth_or_array_layers: 1,
            },
        );
        gpu.render_state.queue.submit(Some(encoder.finish()));
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
            renderer.register_native_texture(device.borrow(), &texture_view, FilterMode::Nearest);
        drop(renderer);
        id
    }
    pub fn perform_action(&mut self, origin: ActionOrigin) {
        self.selected_tool.perform_action(self, origin);
    }
}
