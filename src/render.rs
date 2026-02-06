use crate::camera::{Camera, CameraUniforms}; 
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, Device, Queue, RenderPipeline, ShaderModule, Texture,
    TextureView,
};

pub struct PathTracer {
    device: Device,
    queue: Queue,
    uniforms: Uniforms,
    uniform_buffer: Buffer,
    display_pipeline: RenderPipeline,
    display_bind_group: BindGroup,
    vertex_buffer: Buffer,
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    width: u32,
    height: u32,
    frame_count: u32,
    _pad: u32, 
    camera: CameraUniforms,
}

impl PathTracer {
    pub fn new(device: Device, queue: Queue, width: u32, height: u32) -> Self {
        device.on_uncaptured_error(Box::new(|err| {
            panic!("Unhandled error: {err}");
        }));

        let shader_mod = compile_shader_module(&device);
        let (display_pipeline, bind_group_layout) = create_display_pipeline(&device, &shader_mod);

        let uniforms = Uniforms {
            camera: CameraUniforms::zeroed(),
            width,
            height,
            frame_count: 0,
            _pad: 0,
        };

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            mapped_at_creation: false,
            size: std::mem::size_of::<Uniforms>() as u64,
            label: Some("uniform buffers"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let indices: [u32; 6] = [0, 1, 2, 3, 4, 5];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let radiance_samples = create_sample_texture(&device, width, height);
    
        let display_bind_group = create_display_bindgroup(
            &device,
            &bind_group_layout,
            &radiance_samples,
            &uniform_buffer,
        );

        Self {
            device,
            queue,
            uniforms,
            uniform_buffer,
            display_pipeline,
            display_bind_group,
            vertex_buffer,
        }
    }

    pub fn reset_samples(&mut self) {
        self.uniforms.frame_count = 0;
    }

    pub fn render_frame(&mut self, target: &TextureView, camera: &Camera) {
        self.uniforms.frame_count += 1;
        self.uniforms.camera = camera.get_uniforms(); 
        
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.uniforms),
        );

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render frame"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.display_pipeline);
            render_pass.set_bind_group(0, &self.display_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
    }
}


fn create_display_bindgroup(
    device: &Device,
    layout: &BindGroupLayout,
    texture: &Texture,
    uniform_buffer: &Buffer,
) -> BindGroup {
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bind groups"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: uniform_buffer,
                    size: None,
                    offset: 0,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
        ],
    })
}

fn create_sample_texture(device: &Device, width: u32, height: u32) -> Texture {
    let desc = wgpu::TextureDescriptor {
        label: Some("radiance samples"),
        format: wgpu::TextureFormat::Rgba32Float,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        dimension: wgpu::TextureDimension::D2,
        sample_count: 1,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        mip_level_count: 1,
        view_formats: &[],
    };
    device.create_texture(&desc)
}

fn compile_shader_module(device: &Device) -> ShaderModule {
    device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"))
}

fn create_display_pipeline(
    device: &Device,
    shader_mod: &ShaderModule,
) -> (RenderPipeline, BindGroupLayout) {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bind group"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                count: None,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                count: None,
                ty: wgpu::BindingType::StorageTexture {
                    view_dimension: wgpu::TextureViewDimension::D2,
                    access: wgpu::StorageTextureAccess::ReadWrite,
                    format: wgpu::TextureFormat::Rgba32Float,
                },
            },
        ],
    });

    let vertex_buffer_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<u32>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Uint32,
        }],
    };

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("display"),
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&bind_group_layout],
                ..Default::default()
            }),
        ),
        multiview: None,
        depth_stencil: None,
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            polygon_mode: wgpu::PolygonMode::Fill,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: shader_mod,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        vertex: wgpu::VertexState {
            module: shader_mod,
            entry_point: "vs_main", 
            buffers: &[vertex_buffer_layout],
        },
        multisample: wgpu::MultisampleState::default(),
    });
    (pipeline, bind_group_layout)
}
