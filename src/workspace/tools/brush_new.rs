use wgpu::*;

use crate::{
    workspace::{BlendMode, LayerCreationInfo},
    GpuDevice,
};

use super::{ActionOrigin, Tool, Workspace};

use bytemuck::{Pod, Zeroable};

struct BrushToolNew {
    pub size: f32,
    pub color: Option<[u8; 4]>,
    pub texture: Option<TextureView>,
    pub blend_mode: BlendMode,
    pub hardness: f32,
    pub rotation: f32, // in radians
    pub opacity: f32,

    pub path: Vec<Vec2>,
    pub pipeline: Option<ComputePipeline>,
    pub group_zero_bind_group: Option<BindGroup>,
    pub path_buffer: Option<Buffer>,
    pub path_len_buffer: Option<Buffer>,
    pub cur_path_buffer_len: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vec2(f32, f32);

impl BrushToolNew {
    fn acquire_mask(&mut self, workspace: &mut Workspace, gpu: &GpuDevice) {
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

        self.texture = Some(mask_view);
    }

    fn build_pipeline(&mut self, workspace: &mut Workspace, gpu: &GpuDevice) {
        let shader = gpu.shaders.get("tools/brush_new").unwrap();

        todo!();
    }

    fn render_path(&mut self, workspace: &mut Workspace, gpu: &GpuDevice) {
        let pipeline = self.pipeline.as_ref().unwrap();
        if self.path.len() > self.cur_path_buffer_len {
            let path_buffer = gpu.render_state.device.create_buffer(&BufferDescriptor {
                label: Some("Brush Path Buffer"),
                size: (round_up_power_two(
                    self.path.len() * 2 * std::mem::size_of::<f32>() as usize,
                )) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            self.path_buffer = Some(path_buffer);
        }

        let mut path_buffer = self.path_buffer.take().unwrap();
        {
            let mut mapped_range = path_buffer
                .slice(..(self.path.len() * 2 * std::mem::size_of::<f32>()) as u64)
                .get_mapped_range_mut();

            mapped_range.copy_from_slice(bytemuck::cast_slice(self.path.as_slice()));
        }
        path_buffer.unmap();
        self.path_buffer = Some(path_buffer);

        let mut path_len_buffer = self.path_buffer.take().unwrap();
        {
            let mut mapped_range = path_len_buffer
                .slice(..std::mem::size_of::<u32>() as u64)
                .get_mapped_range_mut();

            mapped_range.copy_from_slice(bytemuck::cast_slice(&[self.path.len() as u32]));
        }
        path_len_buffer.unmap();
        self.path_len_buffer = Some(path_len_buffer);

        let group_one_layout =
            gpu.render_state
                .device
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    entries: &[
                        BindGroupLayoutEntry {
                            binding: 0,
                            visibility: ShaderStages::COMPUTE,
                            ty: BindingType::Buffer {
                                min_binding_size: None,
                                has_dynamic_offset: false,
                                ty: BufferBindingType::Storage { read_only: true },
                            },
                            count: None,
                        },
                        BindGroupLayoutEntry {
                            binding: 1,
                            visibility: ShaderStages::COMPUTE,
                            ty: BindingType::Buffer {
                                min_binding_size: None,
                                has_dynamic_offset: false,
                                ty: BufferBindingType::Uniform,
                            },
                            count: None,
                        },
                    ],
                    label: None,
                });

        let group_one_bind_group =
            gpu.render_state
                .device
                .create_bind_group(&BindGroupDescriptor {
                    layout: &group_one_layout,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: self.path_buffer.as_ref().unwrap(),
                            offset: 0,
                            size: None,
                        }),
                    }],
                    label: None,
                });

        let mut cpass = gpu
            .render_state
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        {
            let mut cpass = cpass.begin_compute_pass(&ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });

            cpass.set_pipeline(pipeline);
            cpass.set_bind_group(0, self.group_zero_bind_group.as_ref().unwrap(), &[]);
            cpass.set_bind_group(1, &group_one_bind_group, &[]);

            let work_groups = (
                (workspace.size.0 as f32 / 8.0).ceil() as u32,
                (workspace.size.1 as f32 / 8.0).ceil() as u32,
                1,
            );
            cpass.dispatch_workgroups(work_groups.0, work_groups.1, work_groups.2);
        }

        gpu.render_state
            .queue
            .submit(std::iter::once(cpass.finish()));
    }
}

#[inline]
fn round_up_power_two(x: usize) -> usize {
    let mut x = x;
    x -= 1;
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    x += 1;
    x
}

impl Default for BrushToolNew {
    fn default() -> Self {
        Self {
            size: 10.0,
            color: Some([255, 255, 255, 255]),
            texture: None,
            blend_mode: "normal".to_string(),
            hardness: 0.5,
            rotation: 0.0,
            opacity: 1.0,
            path: Vec::new(),
            pipeline: None,
            group_zero_bind_group: None,
            path_buffer: None,
            cur_path_buffer_len: 0,
            path_len_buffer: None,
        }
    }
}

impl Tool for BrushToolNew {
    fn name(&self) -> &str {
        "Brush"
    }

    fn perform_action(&mut self, workspace: &mut Workspace, gpu: &GpuDevice, origin: ActionOrigin) {
        match origin {
            ActionOrigin::MouseDown(mouse_loc) => {
                self.path.clear();
                self.path.push(Vec2(mouse_loc.0, mouse_loc.1));

                self.acquire_mask(workspace, gpu);
                self.build_pipeline(workspace, gpu);
                self.render_path(workspace, gpu);
            }
            ActionOrigin::MouseMove(mouse_loc) => {
                self.path.push(Vec2(mouse_loc.0, mouse_loc.1));

                self.render_path(workspace, gpu);
            }
            ActionOrigin::MouseUp(mouse_loc) => {}
            _ => {}
        }
    }
}
