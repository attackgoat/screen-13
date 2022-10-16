use {
    super::KeyBuf,
    std::{fmt::Debug, time::Instant},
    winit::event::VirtualKeyCode,
};

/// A binding between key and axis activation values.
#[derive(Clone, Debug)]
pub struct Binding<A> {
    axis: A,
    key: VirtualKeyCode,
    multiplier: f32,
    activation: f32,
    activation_time: f32,
}

impl<A> Binding<A> {
    pub const DEFAULT_ACTIVATION_TIME: f32 = 0.15;

    pub fn new(key: VirtualKeyCode, axis: A, multiplier: f32) -> Self {
        Self {
            axis,
            key,
            multiplier,
            activation: 0.0,
            activation_time: Self::DEFAULT_ACTIVATION_TIME,
        }
    }

    pub fn with_activation_time(mut self, activation_time: f32) -> Self {
        assert!(activation_time >= 0.0);

        self.activation_time = activation_time;
        self
    }
}

/// A basic key input mapping.
#[derive(Clone, Debug)]
pub struct KeyMap<A> {
    axis: Vec<(A, f32)>,
    bindings: Vec<Binding<A>>,
    last_update: Instant,
}

impl<A> KeyMap<A>
where
    A: Copy + Debug + Ord,
{
    /// Gets the value of this axis, which is between -1.0 and 1.0 inclusive.
    pub fn axis_value(&self, axis: &A) -> f32 {
        let res = self
            .axis
            .binary_search_by(|(a, _)| a.cmp(axis))
            .ok()
            .map(|idx| self.axis[idx].1);

        #[cfg(debug_assertions)]
        if res.is_none() {
            log::warn!("Unrecognized axis: {:#?}", axis);
        }

        res.unwrap_or_default()
    }

    /// Binds a key to an axis.
    pub fn bind(self, key: VirtualKeyCode, axis: A, multiplier: f32) -> Self {
        self.binding(Binding::new(key, axis, multiplier))
    }

    /// Binds a key.
    pub fn binding(mut self, binding: Binding<A>) -> Self {
        if let Err(idx) = self.axis.binary_search_by(|(a, _)| a.cmp(&binding.axis)) {
            self.axis.insert(idx, (binding.axis, 0.0));
        };
        self.bindings.push(binding);
        self
    }

    /// Updates the key axis values.
    pub fn update(&mut self, keyboard: &KeyBuf) {
        let now = Instant::now();
        let dt = (now - self.last_update).as_secs_f32().max(1.0 / 60.0);
        self.last_update = now;

        for binding in &mut self.bindings {
            if binding.activation_time > 1e-10 {
                let change = if keyboard.is_pressed(&binding.key) {
                    dt
                } else {
                    -dt
                };
                binding.activation =
                    (binding.activation + change / binding.activation_time).clamp(0.0, 1.0);
            } else if keyboard.is_pressed(&binding.key) {
                binding.activation = 1.0;
            } else {
                binding.activation = 0.0;
            }

            let axis_idx = self
                .axis
                .binary_search_by(|(a, _)| a.cmp(&binding.axis))
                .unwrap();
            let (_, value) = &mut self.axis[axis_idx];
            *value = (*value + binding.activation * binding.multiplier).clamp(-1.0, 1.0);
        }
    }
}

impl<A> Default for KeyMap<A> {
    fn default() -> Self {
        Self {
            axis: Default::default(),
            bindings: Default::default(),
            last_update: Instant::now(),
        }
    }
}
