use {
    super::{Pass, RenderGraph},
    archery::SharedPointerKind,
};

pub fn assert_valid(graph: &RenderGraph<impl SharedPointerKind>) {
    for pass in &graph.passes {
        assert_valid_pass(pass);
    }
}

fn assert_valid_pass(pass: &Pass<impl SharedPointerKind>) {
    //let mut first_end = 0;

    // let mut start = 0;
    // while start < pass.ops.len() {
    //     let end = pass.ops[start..]
    //         .iter()
    //         .enumerate()
    //         .take_while(|(idx, op)| !op.is_execute())
    //         .last()
    //         .map(|(idx, _)| idx)
    //         .unwrap_or_default()
    //         + start
    //         + 1;
    //     if end == start {
    //         // No further executions found
    //         break;
    //     } else if first_end == 0 {
    //         // Keep track of the first group; it has to fully bind the descriptors!
    //         first_end = end;
    //     }

    //     assert_valid_pass_execution(pass, &pass.ops[start..end]);
    // }

    // TODO: Some of this is now duplicated with runtime checks....

    // let mut clear_color = BTreeSet::new();
    // let mut clear_depth = false;
    // let mut load_color = BTreeSet::new();
    // let mut load_depth = false;
    // let mut input_color = BTreeSet::new();
    // let mut input_depth = false;
    // let mut resolve_color = BTreeSet::new();
    // let mut resolve_depth = false;
    // let mut store_color = BTreeSet::new();
    // let mut store_depth = false;

    // let raster_ops = pass
    //     .ops
    //     .iter()
    //     .map(|op| op.as_raster())
    //     .flatten()
    //     .collect::<Vec<_>>();
    // for op in &raster_ops {
    //     match op {
    //         RasterOp::ClearColor(attachment, _) => assert!(!clear_color.insert(*attachment)),
    //         RasterOp::ClearDepthStencil(_) => assert!(!replace(&mut clear_depth, true)),
    //         RasterOp::LoadColor(attachment, ..) => assert!(!load_color.insert(*attachment)),
    //         RasterOp::LoadDepthStencil(..) => assert!(!replace(&mut load_depth, true)),
    //         RasterOp::InputColor(attachment, ..) => assert!(!input_color.insert(*attachment)),
    //         RasterOp::InputDepthStencil(..) => assert!(!replace(&mut input_depth, true)),
    //         RasterOp::ResolveColor(attachment, ..) => assert!(!resolve_color.insert(*attachment)),
    //         RasterOp::ResolveDepthStencil(..) => assert!(!replace(&mut resolve_depth, true)),
    //         RasterOp::StoreColor(attachment, ..) => assert!(!store_color.insert(*attachment)),
    //         RasterOp::StoreDepthStencil(..) => assert!(!replace(&mut store_depth, true)),
    //     }
    // }

    // TODO: Check for cleared but not stored or resolved!

    // for attachment in &input_color {
    //     assert!(
    //         clear_color.contains(attachment) || load_color.contains(attachment),
    //         "Color attachment {attachment} used as input without specifying a clear value or loading an image"
    //     );
    // }

    // if input_depth {
    //     assert!(
    //         clear_depth || load_depth,
    //         "Depth/stencil attachment used as input without specifying a clear value or loading an image"
    //     );
    // }

    for _attachment in pass.store_attachments.attached.iter() {
        // assert!(
        //     pass.ops.contains(attachment) || load_color.contains(attachment),
        //     "Color attachment {attachment} stored without specifying a clear value or loading an image"
        // );
    }

    // for attachment in &store_color {
    //     assert!(
    //         clear_color.contains(attachment) || load_color.contains(attachment),
    //         "Color attachment {attachment} stored without specifying a clear value or loading an image"
    //     );
    // }

    // if store_depth {
    //     assert!(
    //         clear_depth || load_depth,
    //         "Depth/stencil attachment stored without specifying a clear value or loading an image"
    //     );
    // }

    // if let Some(pipeline) = pass.pipelines.get(0) {
    //     let descriptor_pool_sizes = pass.descriptor_pools_sizes().next().unwrap();
    //     let descriptor_sets_info = pass.descriptor_sets().next().unwrap();

    // // Don't allow binding to any attachments (use input for that sort of thing)
    // for (node_idx, desc_binding) in pass
    //     .ops
    //     .iter()
    //     .filter(|op| op.is_bind())
    //     .map(|op| op.unwrap_bind())
    // {
    //     assert!(!load_colors.contains_key(&node_idx));
    //     assert!(!load_depth
    //         .map(|(idx, _)| idx)
    //         .filter(|idx| *idx == node_idx)
    //         .is_some());
    //     assert!(!resolve_colors.contains_key(&node_idx));
    //     assert!(!resolve_depth
    //         .map(|(idx, _)| idx)
    //         .filter(|idx| *idx == node_idx)
    //         .is_some());
    //     assert!(!store_colors.contains_key(&node_idx));
    //     assert!(!store_depth
    //         .map(|(idx, _)| idx)
    //         .filter(|idx| *idx == node_idx)
    //         .is_some());
    // }

    // Start looking at descriptors now...
    // Figure out what was bound just prior to the first execution
    //let mut bound = BTreeMap::new();
    // for (node_idx, desc_binding) in pass.ops[0..first_end]
    //     .iter()
    //     .filter(|op| op.is_bind())
    //     .map(|op| op.unwrap_bind())
    // {
    //     let (descriptor_set_idx, binding_idx, binding_array_idx) = desc_binding.into_tuple();
    //     let descriptor_set = bound
    //         .entry(descriptor_set_idx)
    //         .or_insert_with(BTreeMap::new);

    //     let binding: &mut Vec<_> = descriptor_set.entry(binding_idx).or_default();
    //     while binding.len() <= binding_idx as usize {
    //         binding.push(None);
    //     }

    //     binding[binding_array_idx as usize] = Some(node_idx);
    // }

    // // Check the results
    // for (descriptor_set_idx, bindings) in bound.iter() {
    //     // Must fully bind arrays
    //     for (binding_idx, array_items) in bindings {
    //         assert!(
    //             array_items.iter().all(|item| item.is_some()),
    //             "Descriptor set {descriptor_set_idx} binding {binding_idx} array item not bound"
    //         );
    //     }
    // }

    // Must fully bind descriptors before the first execution
    //     for (descriptor_binding, (name, descriptor_ty)) in descriptor_sets_info.iter() {
    //         let bound_descriptor_set = bound
    //             .get(&descriptor_binding.set())
    //             .unwrap_or_else(|| panic!("Descriptor set {} not bound", descriptor_binding.set()));

    //         let bound_binding_count = bound_descriptor_set
    //             .get(&descriptor_binding.bind())
    //             .unwrap_or_else(|| {
    //                 panic!(
    //                     "Descriptor set {} binding {} not bound",
    //                     descriptor_binding.set(),
    //                     descriptor_binding.bind()
    //                 )
    //             })
    //             .len();
    //         let expected_binding_count = descriptor_ty.nbind();

    //         assert_eq!(bound_binding_count, expected_binding_count as usize, "Descriptor set {} binding {} specified {expected_binding_count} bindings but found {bound_binding_count}", descriptor_binding.set(), descriptor_binding.bind());
    //     }
    // }
}

// This looks at a single group of state ops (Access, Bind, or Raster - no Execute/NextPipeline)
// fn assert_valid_pass_execution<P>(pass: &Pass<P>, ops: &[Op<P>])
// where
//     P: SharedPointerKind,
// {
// }
