use {
    super::{compile_shader_and_watch, create_watcher, shader::HotShader},
    log::info,
    notify::RecommendedWatcher,
    screen_13::prelude::*,
    std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug)]
pub struct HotRayTracePipeline {
    device: Arc<Device>,
    has_changes: Arc<AtomicBool>,
    instance: Arc<RayTracePipeline>,
    shader_groups: Box<[RayTraceShaderGroup]>,
    shaders: Box<[HotShader]>,
    watcher: RecommendedWatcher,
}

impl HotRayTracePipeline {
    pub fn create<S>(
        device: &Arc<Device>,
        info: impl Into<RayTracePipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
        shader_groups: impl IntoIterator<Item = RayTraceShaderGroup>,
    ) -> Result<Self, DriverError>
    where
        S: Into<HotShader>,
    {
        let shader_groups = shader_groups.into_iter().collect::<Box<_>>();
        let shaders = shaders
            .into_iter()
            .map(|shader| shader.into())
            .collect::<Box<_>>();

        let (mut watcher, has_changes) = create_watcher();
        let compiled_shaders = shaders
            .iter()
            .map(|shader| compile_shader_and_watch(shader, &mut watcher))
            .collect::<Result<Vec<_>, _>>()?;

        let instance = Arc::new(RayTracePipeline::create(
            device,
            info,
            compiled_shaders,
            shader_groups.iter().copied(),
        )?);

        let device = Arc::clone(device);

        Ok(Self {
            device,
            has_changes,
            instance,
            shader_groups,
            shaders,
            watcher,
        })
    }

    /// Returns the most recent compilation without checking for changes or re-compiling the shader
    /// source code.
    pub fn cold(&self) -> &Arc<RayTracePipeline> {
        &self.instance
    }

    /// Returns the most recent compilation after checking for changes, and if needed re-compiling
    /// the shader source code.
    pub fn hot(&mut self) -> &Arc<RayTracePipeline> {
        let has_changes = self.has_changes.swap(false, Ordering::Relaxed);

        if has_changes {
            info!("Shader change detected");

            let (mut watcher, has_changes) = create_watcher();
            if let Ok(compiled_shaders) = self
                .shaders
                .iter()
                .map(|shader| compile_shader_and_watch(shader, &mut watcher))
                .collect::<Result<Vec<_>, DriverError>>()
            {
                if let Ok(instance) = RayTracePipeline::create(
                    &self.device,
                    self.instance.info,
                    compiled_shaders,
                    self.shader_groups.iter().copied(),
                ) {
                    self.has_changes = has_changes;
                    self.watcher = watcher;
                    self.instance = Arc::new(instance);
                }
            }
        }

        self.cold()
    }
}

impl AsRef<RayTracePipeline> for HotRayTracePipeline {
    fn as_ref(&self) -> &RayTracePipeline {
        self.instance.as_ref()
    }
}
