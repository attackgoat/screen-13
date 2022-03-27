use {screen_13::prelude_arc::*, std::env::current_exe};

// A .pak file "ModelBuf" has the following basic structure:
//   Model -> Mesh[] -> Primitive[] -> LevelOfDetail[] -> Meshlet[]
//
// Where:
//   "->": Specifies "Owns a"
//   "[]": Specifies "Array of"
//   Model: The file, a container of meshes
//   Mesh: A named collection of primitives
//   Primitive: A list of triangles with a material
//   Level of Detail: LOD0..N where each level is half the vertices
//   Meshlet: The actual index/vertex data for rendering!
//
// ModelBufs may be:
// - Just a mesh, index/vertex buffers
// - Mesh + Shadow mesh (same as above but shadow mesh is just positions)
// - Mesh + Shadow + LODs (mesh and shadow mesh each have separate LODs)
// - Mesh + Shadow + LODs all as meshlets (small localized groups of triangles)
//
// ...and more! See the getting started docs at:
// https://github.com/attackgoat/screen-13/blob/master/examples/getting-started.md

fn main() -> anyhow::Result<()> {
    // The models are inside the .pak file which is located in the same directory as this example
    let mut pak_path = current_exe()?;
    pak_path.set_file_name("models.pak");

    // Opening the .pak reads a small header only
    let mut pak = PakBuf::open(pak_path)?;

    // Reads the "default.toml" model which physically reads 155 K of index/vertex data
    let default_model = pak.read_model("model/lantern/default")?;

    // Also read "meshlets.toml" which is the same model but baked into meshopt "meshlets" (172 K)
    let meshlets_model = pak.read_model("model/lantern/meshlets")?;

    // Each model contains multiple artist-named meshes, here we are only looking at one mesh
    // from each model file. Notice how this file bakes each detail level into a single meshlet
    println!(
        "Regular model w/ baked shadow mesh:\n{:#?}\n",
        default_model.meshes[0]
    );

    // Notice how this file has a bunch of meshlets for the geometry
    println!(
        "Meshlet model also w/ shadows:\n{:#?}\n",
        meshlets_model.meshes[0]
    );

    Ok(())
}
