#![allow(incomplete_features)]
#![allow(unused)] // TODO: remove when done
#![feature(generic_const_exprs)]
#![feature(iter_next_chunk)]
#![feature(iter_intersperse)]
#![feature(vec_into_raw_parts)]

pub mod app;
pub mod device;
pub mod filters;

use app::*;
use device::*;
use wgpu::*;
mod workspace;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
    vec,
};

use eframe::egui;
use egui::{
    load::SizedTexture, panel, Image, Layout, Pos2, Rect, Sense, TextureId, TopBottomPanel, Vec2,
};
use egui_wgpu::{RenderState, WgpuConfiguration};
use tokio::runtime::Runtime;
use workspace::{
    layer_info::LayerCreationInfo,
    tools::{
        brush::{BrushTool, BrushToolSettings},
        ActionOrigin,
    },
    Workspace,
};

fn main() -> eframe::Result {
    let args = std::env::args().collect::<Vec<String>>();
    let file_to_load = match args.len() {
        1 => None,
        2 => {
            if args[1] == "--help" {
                println!("Usage: joyful_create [file]");
                return Ok(());
            }

            let file_to_load = PathBuf::from(args[1].clone());
            if !file_to_load.exists() {
                println!("File not found: {}", file_to_load.display());
                return Ok(());
            }
            if !file_to_load.is_file() {
                println!("Not a file: {}", file_to_load.display());
                return Ok(());
            }

            Some(file_to_load)
        }
        _ => {
            println!("Too many arguments provided.");
            println!("Usage: joyful_create [file]");

            return Ok(());
        }
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .unwrap();

    let rt_arc = Arc::new(runtime);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: WgpuConfiguration {
            device_descriptor: Arc::new(|_| DeviceDescriptor {
                required_features: Features::default()
                    | Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                ..Default::default()
            }),
            ..Default::default()
        },

        ..Default::default()
    };

    eframe::run_native(
        "Joyful Create",
        options,
        Box::new(|cc| {
            let render_state = cc.wgpu_render_state.clone().unwrap();
            let gpu = rt_arc.block_on(GpuDevice::new(render_state)).unwrap();

            let workspace = match file_to_load {
                None => {
                    let mut workspace = Workspace::default();
                    workspace.create_layer(
                        LayerCreationInfo {
                            name: "Background".to_string(),
                            init_rgba: Some([255, 255, 255, 255]),
                            ..Default::default()
                        },
                        &gpu,
                        None,
                    );

                    let tool: BrushTool = BrushTool::new(
                        BrushToolSettings {
                            size: 50.0,
                            color: Some([100, 255, 0, 255]),
                            blend_mode: "normal".to_string(),
                            hardness: 0.0,
                            ..Default::default()
                        },
                        &gpu,
                    );
                    workspace.set_tool(Box::new(tool));

                    workspace
                }
                Some(path) => {
                    let extension = path.extension().unwrap().to_str().unwrap();

                    match extension {
                        "jc" => {
                            let workspace = Workspace::load(path.to_str().unwrap(), &gpu).unwrap();
                            workspace
                        }
                        "png" => {
                            let image = image::open(path).unwrap();
                            let image = image.to_rgba8();
                            let width = image.width();
                            let height = image.height();

                            let mut workspace = Workspace {
                                size: (width, height),
                                ..Default::default()
                            };

                            workspace.create_layer(
                                LayerCreationInfo {
                                    name: "Background".to_string(),
                                    init_image: Some(image),
                                    ..Default::default()
                                },
                                &gpu,
                                None,
                            );
                            workspace
                        }
                        _ => {
                            println!("Unsupported file type: {}", extension);
                            Workspace::default()
                        }
                    }
                }
            };

            let output_tex = workspace.register_output_texture(cc);
            let app = App::new(gpu, rt_arc.clone(), output_tex, workspace);

            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}
