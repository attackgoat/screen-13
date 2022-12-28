use {
    super::{compile_shader_and_watch, create_watcher, shader::HotShader},
    notify::INotifyWatcher,
    screen_13::prelude::*,
    std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug)]
pub struct HotGraphicPipeline {
    device: Arc<Device>,
    has_changes: Arc<AtomicBool>,
    instance: Arc<GraphicPipeline>,
    shaders: Box<[HotShader]>,
    watcher: INotifyWatcher,
}

impl HotGraphicPipeline {
    pub fn create<S>(
        device: &Arc<Device>,
        info: impl Into<GraphicPipelineInfo>,
        shaders: impl IntoIterator<Item = S>,
    ) -> Result<Self, DriverError>
    where
        S: Into<HotShader>,
    {
        let shaders = shaders
            .into_iter()
            .map(|shader| shader.into())
            .collect::<Box<_>>();

        let (mut watcher, has_changes) = create_watcher();
        let compiled_shaders = shaders
            .iter()
            .map(|shader| compile_shader_and_watch(shader, &mut watcher))
            .collect::<Result<Vec<_>, _>>()?;

        let instance = Arc::new(GraphicPipeline::create(device, info, compiled_shaders)?);

        let device = Arc::clone(device);

        Ok(Self {
            device,
            has_changes,
            instance,
            shaders,
            watcher,
        })
    }

    /// Returns the most recent compilation without checking for changes or re-compiling the shader
    /// source code.
    pub fn cold(&self) -> &Arc<GraphicPipeline> {
        &self.instance
    }

    /// Returns the most recent compilation after checking for changes, and if needed re-compiling
    /// the shader source code.
    pub fn hot(&mut self) -> &Arc<GraphicPipeline> {
        let has_changes = self.has_changes.swap(false, Ordering::Relaxed);

        if has_changes {
            let (mut watcher, has_changes) = create_watcher();
            if let Ok(compiled_shaders) = self
                .shaders
                .iter()
                .map(|shader| compile_shader_and_watch(shader, &mut watcher))
                .collect::<Result<Vec<_>, DriverError>>()
            {
                if let Ok(instance) = GraphicPipeline::create(
                    &self.device,
                    self.instance.info.clone(),
                    compiled_shaders,
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
