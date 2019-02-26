use crate::encoding::{renderable::PipelineEncodingSystem, PipelineListResolver};
use gfx_hal::{
    pass::Subpass,
    pso::{BakedStates, GraphicsPipelineDesc, GraphicsShaderSet},
    Backend,
};
use rendy::{
    command::{QueueId, RenderPassEncoder},
    factory::Factory,
    graph::{
        render::{PrepareResult, RenderGroup, RenderGroupDesc},
        BufferAccess, ImageAccess, NodeBuffer, NodeImage,
    },
};
use shred::{DispatcherBuilder, Resources, RunNow};

#[derive(Debug)]
pub struct DataDrivenRenderGroup<B, T>
where
    B: Backend,
    T: PipelineListResolver,
{
    resolver: T,
    systems: Vec<PipelineEncodingSystem<B>>,
    pso_desc_builder: PsoDescBuilder,
}

impl<B: Backend, T: PipelineListResolver> RenderGroup<B, Resources>
    for DataDrivenRenderGroup<B, T>
{
    fn prepare(
        &mut self,
        factory: &Factory<B>,
        queue: QueueId,
        index: usize,
        _subpass: gfx_hal::pass::Subpass<'_, B>,
        res: &Resources,
    ) -> PrepareResult {
        // TODO: don't do that every frame, obviously
        let new_systems = self
            .resolver
            .resolve(res)
            .into_iter()
            .map(|pipeline| PipelineEncodingSystem::new(pipeline))
            .collect();

        self.systems = new_systems;
        PrepareResult::DrawRecord
    }

    fn draw_inline(
        &mut self,
        mut encoder: RenderPassEncoder<'_, B>,
        _index: usize,
        subpass: gfx_hal::pass::Subpass<'_, B>,
        res: &Resources,
    ) {
        // TODO: don't build the dispatcher every frame
        {
            let mut builder = DispatcherBuilder::new();
            for system in self.systems.iter_mut() {
                builder.add(system, "", &[]);
            }
            builder.build().run_now(res);
        }

        for system in self.systems.iter_mut() {
            system
                .pipeline_mut()
                .draw_inline(&mut encoder, &self.pso_desc_builder, subpass);
        }

        unimplemented!()
    }

    fn dispose(self: Box<Self>, factory: &mut Factory<B>, res: &mut Resources) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct PsoDescBuilder {
    baked_states: BakedStates,
}

impl PsoDescBuilder {
    pub fn new(framebuffer_width: u32, framebuffer_height: u32) -> Self {
        let rect = gfx_hal::pso::Rect {
            x: 0,
            y: 0,
            w: framebuffer_width as i16,
            h: framebuffer_height as i16,
        };

        Self {
            baked_states: gfx_hal::pso::BakedStates {
                viewport: Some(gfx_hal::pso::Viewport {
                    rect,
                    depth: 0.0..1.0,
                }),
                scissor: Some(rect),
                blend_color: None,
                depth_bounds: None,
            },
        }
    }

    pub fn build<'a, B: Backend>(
        &self,
        shader_set: GraphicsShaderSet<'a, B>,
        pipeline_layout: &'a B::PipelineLayout,
        subpass: Subpass<'a, B>,
    ) -> GraphicsPipelineDesc<'a, B> {
        GraphicsPipelineDesc {
            shaders: shader_set,
            rasterizer: gfx_hal::pso::Rasterizer::FILL,
            vertex_buffers: Vec::new(), // TODO
            attributes: Vec::new(),     // TODO
            input_assembler: gfx_hal::pso::InputAssemblerDesc {
                primitive: gfx_hal::Primitive::TriangleList,
                primitive_restart: gfx_hal::pso::PrimitiveRestart::Disabled,
            },
            blender: gfx_hal::pso::BlendDesc {
                logic_op: None,
                // TODO: make blend targets configurable (probably on Renderable)
                targets: vec![gfx_hal::pso::ColorBlendDesc(
                    gfx_hal::pso::ColorMask::ALL,
                    gfx_hal::pso::BlendState::ALPHA,
                )],
            },
            // TODO: make depth_stencil configurable (probably on Renderable)
            depth_stencil: gfx_hal::pso::DepthStencilDesc {
                depth: gfx_hal::pso::DepthTest::On {
                    fun: gfx_hal::pso::Comparison::Less,
                    write: true,
                },
                depth_bounds: false,
                stencil: gfx_hal::pso::StencilTest::Off,
            },
            multisampling: None,
            baked_states: self.baked_states.clone(),
            layout: &pipeline_layout,
            subpass,
            flags: gfx_hal::pso::PipelineCreationFlags::empty(),
            parent: gfx_hal::pso::BasePipeline::None,
        }
    }
}

#[derive(Debug)]
pub struct PipelineListResolverDesc<T>
where
    T: PipelineListResolver,
{
    resolver: T,
    colors: usize,
    depth: bool,
}

impl<T> PipelineListResolverDesc<T>
where
    T: PipelineListResolver,
{
    pub fn with_colors(mut self, colors: usize) -> Self {
        self.colors = colors;
        self
    }

    pub fn with_depth(mut self, depth: bool) -> Self {
        self.depth = depth;
        self
    }
}

impl<B, T> RenderGroupDesc<B, Resources> for PipelineListResolverDesc<T>
where
    B: Backend,
    T: PipelineListResolver + 'static,
{
    /// Get buffers used by the group
    fn buffers(&self) -> Vec<BufferAccess> {
        Vec::new()
    }

    fn images(&self) -> Vec<ImageAccess> {
        Vec::new()
    }

    fn colors(&self) -> usize {
        self.colors
    }

    fn depth(&self) -> bool {
        self.depth
    }

    fn build<'a>(
        self,
        _factory: &mut Factory<B>,
        _queue: QueueId,
        _aux: &mut Resources,
        framebuffer_width: u32,
        framebuffer_height: u32,
        _subpass: gfx_hal::pass::Subpass<'_, B>,
        _buffers: Vec<NodeBuffer<'a, B>>,
        _images: Vec<NodeImage<'a, B>>,
    ) -> Result<Box<dyn RenderGroup<B, Resources>>, failure::Error> {
        Ok(Box::new(DataDrivenRenderGroup {
            resolver: self.resolver,
            systems: Vec::new(),
            pso_desc_builder: PsoDescBuilder::new(framebuffer_width, framebuffer_height),
        }))
    }
}
