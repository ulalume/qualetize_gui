use moxcms::ColorProfile;

pub fn convert_rgba_with_color_profile(
    rgba_data: &Vec<u8>,
    width: usize,
    src_profile: &ColorProfile,
    dest_profile: &ColorProfile,
) -> Vec<u8> {
    let transform = src_profile
        .create_transform_8bit(
            moxcms::Layout::Rgba,
            &dest_profile,
            moxcms::Layout::Rgba,
            moxcms::TransformOptions::default(),
        )
        .unwrap();

    let mut dst = vec![0u8; rgba_data.len()];
    for (src, dst_chunk) in rgba_data
        .chunks_exact(width * 4)
        .zip(dst.chunks_exact_mut(width * 4))
    {
        transform.transform(src, dst_chunk).unwrap();
    }
    dst
}
