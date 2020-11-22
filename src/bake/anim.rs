use {
    super::{
        asset::{Animation as AnimationAsset, Asset},
        get_filename_key, get_path,
        pak_log::{LogId, PakLog},
    },
    crate::{
        math::{quat, Quat, Vec3},
        pak::{Animation, AnimationId, Channel, PakBuf},
    },
    gltf::{
        animation::{
            util::{ReadOutputs, Rotations},
            Property,
        },
        import,
    },
    std::{
        collections::{hash_map::RandomState, HashSet},
        iter::FromIterator,
        path::Path,
    },
};

pub fn bake_animation<P1: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P1,
    asset_filename: P2,
    asset: &AnimationAsset,
    pak: &mut PakBuf,
    log: &mut PakLog,
) -> AnimationId {
    let dir = asset_filename.as_ref().parent().unwrap();
    let src = get_path(&dir, asset.src());

    // Early out if we've already baked this asset
    let exclude = asset
        .exclude()
        .unwrap_or_default()
        .iter()
        .map(|s| s.clone());
    let name = asset.name().map(|s| s.to_owned());
    let proto = Asset::Animation(AnimationAsset::new(&src, name, exclude));
    if let Some(LogId::Animation(id)) = log.get(&proto) {
        return id;
    }

    let key = get_filename_key(&project_dir, &asset_filename);

    info!("Processing asset: {}", key);

    let name = asset.name();
    let (doc, bufs, _) = import(src).unwrap();
    let mut anim = doc.animations().find(|anim| name == anim.name());
    if anim.is_none() && name.is_none() && doc.animations().count() > 0 {
        anim = doc.animations().next();
    }

    let anim = anim.unwrap();
    let exclude: HashSet<&str, RandomState> = HashSet::from_iter(
        asset
            .exclude()
            .unwrap_or_default()
            .iter()
            .map(|s| s.as_str()),
    );

    enum Output {
        Rotations(Vec<Quat>),
        Scales(Vec<Vec3>),
        Translations(Vec<Vec3>),
    }

    let mut channels = vec![];
    let mut channel_names = HashSet::new();

    'channel: for channel in anim.channels() {
        let name = if let Some(name) = channel.target().node().name() {
            name
        } else {
            continue;
        };

        if exclude.contains(name) {
            continue;
        }

        // Only support rotations for now
        let property = channel.target().property();
        match property {
            Property::Rotation => (),
            _ => continue,
        }

        // We require all joint names to be unique
        if channel_names.contains(&name) {
            warn!("Duplicate rotation channels or non-unique targets");
            continue;
        }

        channel_names.insert(name);

        let sampler = channel.sampler();
        let interpolation = sampler.interpolation();

        let data = channel.reader(|buf| bufs.get(buf.index()).map(|data| &*data.0));
        let inputs = data.read_inputs().unwrap().collect::<Vec<_>>();
        if inputs.is_empty() {
            continue;
        }

        // Assure increasing sort
        let mut input = inputs[0];
        for val in inputs.iter().skip(1) {
            if *val > input {
                input = *val
            } else {
                warn!("Unsorted input data");
                continue 'channel;
            }
        }

        let outputs = match data.read_outputs().unwrap() {
            ReadOutputs::Rotations(Rotations::F32(rotations)) => {
                Output::Rotations(rotations.map(|r| quat(r[0], r[1], r[2], r[3])).collect())
            }
            _ => continue,
        };
        let rotations = match outputs {
            Output::Rotations(r) => r,
            _ => continue,
        };

        channels.push(Channel::new(name, interpolation, inputs, rotations));

        // print!(
        //     " {} {:#?}",
        //     channel.target().node().name().unwrap_or("?"),
        //     channel.target().property()
        // );
        // print!(
        //     " ({:#?} {} Inputs, {} Output ",
        //     interpolation,
        //     inputs.len(),
        //     //inputs.iter().rev().take(5).collect::<Vec<_>>(),
        //     match &output {
        //         Output::Rotations(r) => r.len(),
        //         Output::Scales(s) => s.len(),
        //         Output::Translations(t) => t.len(),
        //     }
        // );

        // match &output {
        //     Output::Rotations(_) => print!("Rotations"),
        //     Output::Scales(_) => print!("Scales"),
        //     Output::Translations(_) => print!("Translations"),
        // }

        // println!(")");
    }

    // Pak and log this asset
    let anim = Animation { channels };
    let anim_id = pak.push_animation(key, anim);
    log.add(&proto, anim_id);

    anim_id
}
