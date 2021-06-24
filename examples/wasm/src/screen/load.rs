use {super::menu::Menu, screen_13::prelude_rc::*};

pub struct Load;

impl Screen<RcK> for Load {
    fn render(&self, gpu: &Gpu, dims: Extent) -> Render {
        let mut frame = gpu.render(dims);
        frame.clear().record();

        frame
    }

    fn update(self: Box<Self>, gpu: &Gpu, _: &Input) -> DynScreen {
        let mut pak = Pak::open("wasm.pak")
            .expect("ERROR: You must first pack the runtime content - See README.md");
        let font_h1 = gpu.read_bitmap_font(&mut pak, "font/permanent-marker");

        Box::new(Menu { font_h1 })
    }
}
