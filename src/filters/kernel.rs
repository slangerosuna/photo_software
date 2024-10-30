use crate::device::{pad_to_multiple_of_256, GpuDevice};

use image::{ImageBuffer, Rgba};
use wgpu::*;

impl GpuDevice {
    pub async fn compile_kernel_shader(&self) -> std::io::Result<ShaderModule> {
        let kernel_shader = self
            .render_state
            .device
            .create_shader_module(ShaderModuleDescriptor {
                label: None,
                source: ShaderSource::Wgsl(std::fs::read_to_string("filters/kernel")?.into()),
            });

        Ok(kernel_shader)
    }
}

pub struct Kernel(Texture);

impl Kernel {
    pub fn new(data: &[f32], i: u32, j: u32, gpu: &GpuDevice) -> Self {
        #[cfg(debug_assertions)]
        assert!(data.len() as u32 == i * j * 4);

        #[cfg(debug_assertions)]
        print!("Creating kernel with size {}x{}...\n", i, j);

        let texture = gpu.render_state.device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: i,
                height: j,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC
                | TextureUsages::STORAGE_BINDING
                | TextureUsages::COPY_DST,
            view_formats: &[TextureFormat::Rgba32Float],
        });

        gpu.render_state.queue.write_texture(
            ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&data),
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * 4 * i),
                rows_per_image: Some(j),
            },
            Extent3d {
                width: i,
                height: j,
                depth_or_array_layers: 1,
            },
        );

        Self(texture)
    }

    pub async fn apply_to_image(
        &self,
        image: ImageBuffer<Rgba<u8>, Vec<u8>>,
        gpu: &GpuDevice,
    ) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let width = image.width();
        let height = image.height();

        #[cfg(debug_assertions)]
        print!("Creating input texture...\n");
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
        print!("Writing input texture...\n");
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

        #[cfg(debug_assertions)]
        print!("Creating output texture...\n");
        let output_texture = gpu.render_state.device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: pad_to_multiple_of_256(width),
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

        self.apply(&input_texture, &output_texture, width, height, gpu)
            .await;

        #[cfg(debug_assertions)]
        print!("Reading output texture...\n");
        gpu.texture_to_image(&output_texture, width).await
    }

    pub async fn apply(
        &self,
        input_texture: &Texture,
        output_texture: &Texture,
        width: u32,
        height: u32,
        gpu: &GpuDevice,
    ) {
        #[cfg(debug_assertions)]
        print!("Applying kernel...\n");
        let bind_group_layout =
            gpu.render_state
                .device
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
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
                                format: TextureFormat::Rgba32Float,
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
                    ],
                });

        let pipeline_layout =
            gpu.render_state
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        #[cfg(debug_assertions)]
        assert!(gpu.shaders.contains_key("filters/kernel"));

        let kernel_shader = gpu.shaders.get("filters/kernel").unwrap();

        let pipeline =
            gpu.render_state
                .device
                .create_compute_pipeline(&ComputePipelineDescriptor {
                    label: None,
                    layout: Some(&pipeline_layout),
                    module: kernel_shader,
                    entry_point: "main",
                    compilation_options: Default::default(),
                    cache: None,
                });

        let bind_group = gpu
            .render_state
            .device
            .create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(
                            &input_texture.create_view(&TextureViewDescriptor::default()),
                        ),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(
                            &self.0.create_view(&TextureViewDescriptor::default()),
                        ),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(
                            &output_texture.create_view(&TextureViewDescriptor::default()),
                        ),
                    },
                ],
            });

        let mut encoder = gpu
            .render_state
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        #[cfg(debug_assertions)]
        print!("Dispatching workgroups...\n");
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                timestamp_writes: Default::default(),
                label: None,
            });

            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(
                f32::ceil(width as f32 / 16.) as u32,
                f32::ceil(height as f32 / 16.) as u32,
                1,
            );
        }

        #[cfg(debug_assertions)]
        print!("Submitting work...\n");
        gpu.render_state
            .queue
            .submit(std::iter::once(encoder.finish()));
    }

    pub fn gaussian_kernel<const I: usize, const J: usize>(gpu: &GpuDevice) -> Self
    where
        [(); I * J * 4]:,
    {
        #[cfg(debug_assertions)]
        assert!(I * J < 10000, "When creating a kernel larger than 100x100, such as the {}x{} one you are making, use `Kernel::big_gaussian_kernel` to avoid stack overflow", I, J);

        let mut kernel = [0.0; I * J * 4];
        let sigma = I as f32 / 3.0;
        let mut sum = 0.0;

        let center = (I / 2, J / 2);
        for x in 0..I {
            for y in 0..J {
                let (rx, ry) = (x as i32 - center.0 as i32, y as i32 - center.1 as i32);
                let value = (-(rx as f32 * rx as f32 + ry as f32 * ry as f32)
                    / (2.0 * sigma * sigma))
                    .exp();
                kernel[(x * I + y) * 4] = value;
                kernel[(x * I + y) * 4 + 1] = value;
                kernel[(x * I + y) * 4 + 2] = value;
                kernel[(x * I + y) * 4 + 3] = 1.0;
                sum += value;
            }
        }
        for i in 0..I * J * 4 {
            kernel[i] /= sum;
        }

        Self::new(&kernel, I as u32, J as u32, gpu)
    }

    pub fn big_gaussian_kernel(gpu: &GpuDevice, i: usize, j: usize) -> Self {
        let mut kernel = Vec::with_capacity(i * j * 4);
        let sigma = i as f32 / 3.0;
        let mut sum = 0.0;

        let center = ((i / 2) as i32, (j / 2) as i32);
        for x in 0..i {
            for y in 0..j {
                let (rx, ry) = (x as i32 - center.0, y as i32 - center.1);
                let value = (-(rx as f32 * rx as f32 + ry as f32 * ry as f32)
                    / (2.0 * sigma * sigma))
                    .exp();

                kernel.push(value);
                kernel.push(value);
                kernel.push(value);
                kernel.push(1.0);

                sum += value;
            }
        }
        for i in 0..i * j * 4 {
            kernel[i] /= sum;
        }

        Self::new(&kernel, i as u32, j as u32, gpu)
    }
}
