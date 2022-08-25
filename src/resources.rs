//for loading and serving assets on wasm

use std::io::{BufReader, Cursor};

use cfg_if::cfg_if;
use wgpu::util::DeviceExt;

use crate::{model, texture};

//on wasm only
#[cfg(target_arch = "wasm32")]
//get the url and search for the res directory
fn format_url(file_name: &str) -> reqwest::Url {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let base = reqwest::Url::parse(&format!(
        "{}/{}/",
        location.origin().unwrap(),
        option_env!("RES_PATH").unwrap_or("res"),
    ))
    .unwrap();
    base.join(file_name).unwrap()
}

//get the text data from a file location (res/* )
pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let txt = reqwest::get(url)
                .await?
                .text()
                .await?;
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("res")
                .join(&file_name);
            println!("str: {:?}", path);
            let txt = std::fs::read_to_string(path)?;
        }
    }

    Ok(txt)
}

//get the byte data from a file location (res/* )
pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let data = reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec();
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("res")
                .join(&file_name);
                println!("bin: {:?}", path);
            let data = std::fs::read(path)?;
        }
    }

    Ok(data)
}

//load a texture into a specified queue on a device from a filename (res/* ) - a replacement for include_bytes!() macro, as it requires us to know the filename when compiling
pub async fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    let data: Vec<u8> = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}

pub async fn load_obj_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    let obj_text: String = load_string(file_name).await?;
    let obj_cursor: Cursor<String> = Cursor::new(obj_text);
    //sets up an in memory file buffer (so we don't have to read from the file and instead can just from the buffer)
    let mut obj_reader: BufReader<Cursor<String>> = BufReader::new(obj_cursor);

    //models: a list of the models that will be imported from our .obj file
    //obj_materials: a list of the textures that will be imported from the references in the .mtl file
    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            //turns points and lines into zero area triangles
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            let mat_text: String = load_string(&p).await.unwrap();
            //loads the texture material data from the models .mtl file
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials: Vec<model::Material> = Vec::new();
    //consatruct the actual texture materials from the file and index references in the .mtl file
    for mat in obj_materials? {
        let diffuse_texture: texture::Texture =
            load_texture(&mat.diffuse_texture, device, queue).await?;
        let bind_group: wgpu::BindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            //the inputed BindGroupLayout
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
        });

        materials.push(model::Material {
            label: mat.name,
            diffuse_texture,
            bind_group,
        })
    }

    let meshes: Vec<model::Mesh> = models
        .into_iter()
        .map(|mat| {
            println!("{}", mat.mesh.texcoords.len()/2);
            println!("{}", mat.mesh.positions.len()/ 3);

            //divide the mesh positions from the .obj file into groups of 3 f32 for the ModelVertex struct (as they are flattened and must be re-grouped into their 3d space positions)            
            let vertices: Vec<model::ModelVertex> = (0..mat.mesh.positions.len() / 3)
                .map(|i| model::ModelVertex {
                    position: [
                        //as they are in groups of 3, the i * 3 is needed to ensure we are skipping properly over positions
                        mat.mesh.positions[i * 3],
                        mat.mesh.positions[i * 3 + 1],
                        mat.mesh.positions[i * 3 + 2],
                    ],
                    //same as position but only i * 2 as textures are 2d
                    tex_coords: [mat.mesh.texcoords[i * 2], mat.mesh.texcoords[i * 2 + 1]],
                    //the normal texture mappings are 3d, as they are how the entire object is lit
                    normal: [
                        mat.mesh.normals[i * 3],
                        mat.mesh.normals[i * 3 + 1],
                        mat.mesh.normals[i * 3 + 2],
                    ],
                })
                .collect::<Vec<_>>();

            //a buffer to store the vertex data we want to draw (so we don't have to expensively recomplie the shader on every update)
            let vertex_buffer: wgpu::Buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} (Vertex Buffer)", file_name)),
                    //cast to &[u8] as that is how gpu buffers typically expect buffer data
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

            //means that we don't have duplicate vertices, and instead just have a list of their positions that we then render (which saves memory)
            let index_buffer: wgpu::Buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} (Index Buffer)", file_name)),
                    contents: bytemuck::cast_slice(&mat.mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            model::Mesh {
                label: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: mat.mesh.indices.len() as u32,
                material: mat.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok(model::Model { meshes, materials })
}