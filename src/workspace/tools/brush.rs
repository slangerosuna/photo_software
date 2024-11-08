use wgpu::{BindGroup, BindingResource, BufferBinding, Texture};

use crate::{
    workspace::{BlendMode, LayerCreationInfo},
    GpuDevice,
};

use super::{ActionOrigin, Tool, Workspace};
use wgpu::util::DeviceExt;

pub struct BrushTool {
    pub size: f32,
    pub color: Option<[u8; 4]>,
    pub texture: Option<Texture>,
    pub blend_mode: BlendMode,
    pub hardness: f32,
    pub rotation: f32, // in radians
    pub opacity: f32,
    pub group_one_binding: Option<BindGroup>,
    pub group_zero_binding: Option<BindGroup>,
    pub pipeline: Option<wgpu::ComputePipeline>,
}

pub struct BrushToolSettings {
    pub size: f32,
    pub color: Option<[u8; 4]>,
    pub texture: Option<Texture>,
    pub blend_mode: BlendMode,
    pub hardness: f32,
    pub rotation: f32, // in radians
    pub opacity: f32,
}

impl Default for BrushToolSettings {
    fn default() -> Self {
        Self {
            size: 10.0,
            color: None,
            texture: None,
            blend_mode: "normal".into(),
            hardness: 1.0,
            rotation: 0.0,
            opacity: 1.0,
        }
    }
}

impl BrushTool {
    pub fn new(settings: BrushToolSettings, gpu: &GpuDevice) -> Self {
        let mut this = Self {
            size: settings.size,
            color: settings.color,
            texture: settings.texture,
            blend_mode: settings.blend_mode,
            hardness: settings.hardness,
            rotation: settings.rotation,
            opacity: settings.opacity,
            group_one_binding: None,
            group_zero_binding: None,
            pipeline: None,
        };

        this.gen_group_one_binding(gpu);
        this.create_pipeline(gpu);

        this
    }
    
    fn create_pipeline(&mut self, gpu: &GpuDevice) {
        let device = &gpu.render_state.device;
        let shader = gpu.shaders.get("tools/brush").unwrap();

        let zero_layout = gpu.render_state
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture  {
                    access: wgpu::StorageTextureAccess::ReadWrite,
                    format: wgpu::TextureFormat::R8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        });

        let one_layout =  gpu.render_state.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let two_layout = gpu.render_state.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&zero_layout, &one_layout, &two_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions {
                ..Default::default()
            },
            cache: None,
        });

        self.pipeline = Some(pipeline);
    }

    fn gen_group_one_binding(&mut self, gpu: &GpuDevice) {
        let device = &gpu.render_state.device;
        let brush_texture_view = self
            .texture
            .as_ref()
            .unwrap_or(
                &gpu.render_state.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[wgpu::TextureFormat::R8Unorm],
                }),
            )
            .create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let opacity_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[self.opacity]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let opacity_buffer: BindingResource<'_> = opacity_buffer.as_entire_binding();

        let brush_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[self.size]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let brush_size_buffer: BindingResource<'_> = brush_size_buffer.as_entire_binding();

        let brush_hardness_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[self.hardness]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let brush_hardness_buffer: BindingResource<'_> = brush_hardness_buffer.as_entire_binding();

        let brush_rotation_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[self.rotation]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let brush_rotation_buffer: BindingResource<'_> = brush_rotation_buffer.as_entire_binding();

        let use_texture_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[if self.texture.is_some() { 1 } else { 0 }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let use_texture_buffer: BindingResource<'_> = use_texture_buffer.as_entire_binding();

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: opacity_buffer,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: brush_size_buffer,
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: brush_hardness_buffer,
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: brush_rotation_buffer,
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&brush_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: use_texture_buffer,
                },
            ],
            label: None,
        });

        self.group_one_binding = Some(bind_group);
    }

    fn create_brush_layer(&mut self, workspace: &mut Workspace, gpu: &GpuDevice) {
        let index = workspace.selected_layer.map(|x| x + 1);
        workspace.create_layer(
            LayerCreationInfo {
                name: "Brush Layer".to_string(),
                blend_mode: self.blend_mode.clone(),
                init_mask_luma: Some(0),
                init_rgba: self.color,
                is_tool_layer: true,
                ..Default::default()
            },
            gpu,
            index,
        );

        let index = index.unwrap_or(workspace.layers.len() - 1);
        let layer_mask = &workspace.layer_data[index].mask;
        let mask_view = layer_mask.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group_layout =
            gpu.render_state
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture  {
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::R8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    }],
                });

        let bind_group = gpu
            .render_state
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&mask_view),
                }],
                label: None,
            });

        self.group_zero_binding = Some(bind_group);
    }
    fn brush(&self, mouse_loc: (f32, f32), gpu: &GpuDevice) {
        "
        @group(2) @binding(0)
        var<uniform> brush_center : vec2<f32>;
        ";

        let device = &gpu.render_state.device;
        let queue = &gpu.render_state.queue;

        let brush_center_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[mouse_loc.0, mouse_loc.1]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: brush_center_buffer.as_entire_binding(),
            }],
            label: None,
        });

        let mut cpass = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: None,
        });

        {
            let mut cpass = cpass.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            cpass.set_pipeline(&self.pipeline.as_ref().unwrap());
            cpass.set_bind_group(0, self.group_zero_binding.as_ref().unwrap(), &[]);
            cpass.set_bind_group(1, self.group_one_binding.as_ref().unwrap(), &[]);
            cpass.set_bind_group(2, &bind_group, &[]);

            let work_groups =  (self.size + 1.0) / 4.0;
            let work_groups = work_groups.ceil() as u32;
            println!("Work groups: {}", work_groups);
            cpass.dispatch_workgroups(work_groups, work_groups, 1);
        }

        queue.submit(std::iter::once(cpass.finish()));
    }
    fn apply(&self, workspace: &mut Workspace, gpu: &GpuDevice) {
        // merge the tool layer with the layer below it
        todo!()
    }
}

impl Tool for BrushTool {
    fn name(&self) -> &str {
        "Brush"
    }
    fn perform_action(&mut self, workspace: &mut Workspace, gpu: &GpuDevice, origin: ActionOrigin) {
        match origin {
            ActionOrigin::MouseDown(mouse_loc) => {
                self.create_brush_layer(workspace, gpu);
                self.brush(mouse_loc, gpu);
                workspace.recalculate_output_texture(gpu, workspace.selected_layer.unwrap_or(0));
            }
            ActionOrigin::MouseMove(mouse_loc) => {
                self.brush(mouse_loc, gpu);
                workspace.recalculate_output_texture(gpu, workspace.selected_layer.unwrap_or(0));
            }
            ActionOrigin::MouseUp(_mouse_loc) => {
                self.apply(workspace, gpu);
            }
            _ => (),
        }
    }
}
