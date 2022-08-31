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
            // println!("str: {:?}", path);
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
                // println!("bin: {:?}", path);
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
    is_normal_map: bool,
) -> anyhow::Result<texture::Texture> {
    let data: Vec<u8> = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name, is_normal_map)
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
            load_texture(&mat.diffuse_texture, device, queue, true).await?;

        let normal_texture: texture::Texture =
            load_texture(&mat.normal_texture, device, queue, true).await?;

        materials.push(model::Material::new(
            device,
            &mat.name,
            diffuse_texture,
            normal_texture,
            layout,
        ));
    }

    let meshes: Vec<model::Mesh> = models
        .into_iter()
        .map(|mat| {
            // println!("{}", mat.mesh.texcoords.len() / 2);
            // println!("{}", mat.mesh.positions.len() / 3);

            //divide the mesh positions from the .obj file into groups of 3 f32 for the ModelVertex struct (as they are flattened and must be re-grouped into their 3d space positions)
            let mut vertices: Vec<model::ModelVertex> = (0..mat.mesh.positions.len() / 3)
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
                    // We'll calculate these later
                    tangent: [0.0; 3],
                    bitangent: [0.0; 3],
                })
                .collect::<Vec<_>>();

            let indices: &Vec<u32> = &mat.mesh.indices;
            let mut triangles_included: Vec<i32> = vec![0; vertices.len()];

            //calculate tangents and bitangets - we're going to use the triangles, so we need to loop through the indices in chunks of 3
            for c in indices.chunks(3) {
                let v0: model::ModelVertex = vertices[c[0] as usize];
                let v1: model::ModelVertex = vertices[c[1] as usize];
                let v2: model::ModelVertex = vertices[c[2] as usize];

                let pos0: cgmath::Vector3<_> = v0.position.into();
                let pos1: cgmath::Vector3<_> = v1.position.into();
                let pos2: cgmath::Vector3<_> = v2.position.into();

                let uv0: cgmath::Vector2<_> = v0.tex_coords.into();
                let uv1: cgmath::Vector2<_> = v1.tex_coords.into();
                let uv2: cgmath::Vector2<_> = v2.tex_coords.into();

                // Calculate the edges of the triangle
                let delta_pos1: cgmath::Vector3<f32> = pos1 - pos0;
                let delta_pos2: cgmath::Vector3<f32> = pos2 - pos0;

                // This will give us a direction to calculate the
                // tangent and bitangent
                let delta_uv1: cgmath::Vector2<f32> = uv1 - uv0;
                let delta_uv2: cgmath::Vector2<f32> = uv2 - uv0;

                //black box of complicated maths

                //solving the following system of equations will give us the tangent and bitangent.
                //    delta_pos1 = delta_uv1.x * T + delta_u.y * B
                //    delta_pos2 = delta_uv2.x * T + delta_uv2.y * B
                let r: f32 = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tangent: cgmath::Vector3<f32> =
                    (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
                // We flip the bitangent to enable right-handed normal
                // maps with wgpu texture coordinate system
                let bitangent: cgmath::Vector3<f32> =
                    (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * -r;

                //we'll use the same tangent/bitangent for each vertex in the triangle
                vertices[c[0] as usize].tangent =
                    (tangent + cgmath::Vector3::from(vertices[c[0] as usize].tangent)).into();
                vertices[c[1] as usize].tangent =
                    (tangent + cgmath::Vector3::from(vertices[c[1] as usize].tangent)).into();
                vertices[c[2] as usize].tangent =
                    (tangent + cgmath::Vector3::from(vertices[c[2] as usize].tangent)).into();
                vertices[c[0] as usize].bitangent =
                    (bitangent + cgmath::Vector3::from(vertices[c[0] as usize].bitangent)).into();
                vertices[c[1] as usize].bitangent =
                    (bitangent + cgmath::Vector3::from(vertices[c[1] as usize].bitangent)).into();
                vertices[c[2] as usize].bitangent =
                    (bitangent + cgmath::Vector3::from(vertices[c[2] as usize].bitangent)).into();

                // Used to average the tangents/bitangents
                triangles_included[c[0] as usize] += 1;
                triangles_included[c[1] as usize] += 1;
                triangles_included[c[2] as usize] += 1;
            }

            //average the tangents/bitangents
            for (i, n) in triangles_included.into_iter().enumerate() {
                let denom: f32 = 1.0 / n as f32;
                let mut v: &mut model::ModelVertex = &mut vertices[i];
                v.tangent = (cgmath::Vector3::from(v.tangent) * denom).into();
                v.bitangent = (cgmath::Vector3::from(v.bitangent) * denom).into();
            }

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
