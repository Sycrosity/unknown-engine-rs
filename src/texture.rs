use anyhow::*;
use image::GenericImageView;

pub struct Texture {
    //the gpu representation of our texture
    pub texture: wgpu::Texture,
    //describes the texture and associated metadata
    pub view: wgpu::TextureView,
    //controls how a texture is sampled - returning a colour based on a provided pixel coordinate (and some config)
    pub sampler: wgpu::Sampler,
}

impl Texture {
    //for when we create the depth stage of the render_pipeline and for creating the depth texture itself
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
    ) -> Self {
        //needs to be the same size as the screen or it won't render correctly
        let size: wgpu::Extent3d = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        let texture: wgpu::Texture = device.create_texture(
            &(wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                //mark as a depth texture
                format: Self::DEPTH_FORMAT,
                //RENDER_ATTACHMENT - we are rendering this texture so it needs this tag
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            }),
        );

        let view: wgpu::TextureView = texture.create_view(&wgpu::TextureViewDescriptor::default());
        //a sampler isn't strictly neccessary, but our Texture struct needs it and its needed if we ever want to sample it
        let sampler: wgpu::Sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            //when we want to render our depth texture, we need LessEqual - this is because of how GLSL works
            compare: Some(wgpu::CompareFunction::LessEqual),
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }

    //loads an image (from a set of bytes) into a Texture
    pub fn from_bytes(
        //since this isn't part of our main lib.rs program, we need to add references to the device and queue
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
        is_normal_map: bool,
    ) -> Result<Self> {
        //load the bytes from an image into a image::DynamicImage
        let img: image::DynamicImage = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label), is_normal_map)
    }

    //takes an image (in format image::DynamicImage) and returns a Texture
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        //labels must be Option enums, as they can being be None or have data
        label: Option<&str>,
        is_normal_map: bool,
    ) -> Result<Self> {
        //requires to_rgba8() instead of as_rgba8() as
        //convert the png into a Vector of Rgba bytes
        let rgba: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = img.to_rgba8();
        //collect the dimentions of the image (for when we create the actual texture)
        let dimensions: (u32, u32) = img.dimensions();

        //convert the image dimentions into wgpu represented texture size
        let size: wgpu::Extent3d = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            //all textures are stores as 3d objects, so we represent our 2d object by setting a depth of 1
            depth_or_array_layers: 1,
        };

        //the wgpu::Texture that will house our inputed image - here its dimentions and other descriptors are set
        let texture: wgpu::Texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            //[TODO] understand mip levels
            mip_level_count: 1,
            sample_count: 1,
            //our texture is 2 dimentional
            dimension: wgpu::TextureDimension::D2,
            format: if is_normal_map {
                //normal maps are in a different format, as it has more colour density
                wgpu::TextureFormat::Rgba8Unorm
            } else {
                //almost all textures and images are in sRGB colour format
                wgpu::TextureFormat::Rgba8UnormSrgb
            },
            //TEXTURE_BINDING tells wgpu that we want to use this texture in our shaders
            //COPY_DST means that we can copy data to this texture
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        });

        //add our image data to our texture (via the queue)
        queue.write_texture(
            //tells wgpu where to copy the pixel data to
            wgpu::ImageCopyTexture {
                texture: &texture,
                //[TODO]
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                //we are rendering our image in full
                aspect: wgpu::TextureAspect::All,
            },
            //the actual pixel data from our image that is to be written
            &rgba,
            //the layout of the texture
            wgpu::ImageDataLayout {
                offset: 0,
                //must be a multiple of 256 - multiplying our width (which is u8) by 4 ensures this (64 * 4 = 256)
                bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
                rows_per_image: std::num::NonZeroU32::new(dimensions.1),
            },
            size,
        );

        //a bit black-boxy, but we are mostly just letting wgpu configure our texture view and part of the sampler for us
        //describes the texture and associated metadata
        let view: wgpu::TextureView = texture.create_view(&wgpu::TextureViewDescriptor::default());
        //controls how a texture is sampled - returning a colour based on a provided pixel coordinate (and some config)
        let sampler: wgpu::Sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            //what to do if the sampler looks for a colour outside of our texture - ClampToEdge returns the colour of the pixel on the edge nearest to where the sampler is looking for (can be Repeat or MirroredRepeat aswell)
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            //when looking at our image from up close or far away we can often have multiple pixels overlapping a fragment or vice-versa, so we either:
            //(when the texture needs to be enlargened) attempt to blend fragments so they seem to flow together
            mag_filter: wgpu::FilterMode::Linear,
            //(when the texture needs to be minified) use the colour of the nearest pixel
            min_filter: wgpu::FilterMode::Nearest,
            //[TODO] - how to deal with mipmaps
            mipmap_filter: wgpu::FilterMode::Nearest,
            //let wgpu set the rest
            ..Default::default()
        });

        //if anything fails it will return an Err, so if we get to the end we return it with an Ok()  enum
        Ok(Self {
            texture,
            view,
            sampler,
        })
    }
}
