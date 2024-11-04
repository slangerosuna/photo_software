use crate::device::GpuDevice;
use crate::workspace::{tools::ActionOrigin, Workspace};
use egui::{Image, Pos2, Rect, Sense, TextureId, Vec2};
use std::sync::Arc;
use tokio::runtime::Runtime;
use wgpu::*;

pub struct App<'a> {
    gpu: GpuDevice,
    runtime: Arc<Runtime>,
    output_tex: TextureId,
    workspace: Workspace<'a>,
    prev_mouse_pos: Pos2,
    sec_mouse_down: bool,
    prim_mouse_down: bool,
    central_panel_center: Pos2,
}

impl<'a> App<'a> {
    pub fn new(
        gpu: GpuDevice,
        runtime: Arc<Runtime>,
        output_tex: TextureId,
        workspace: Workspace<'a>,
    ) -> App<'a> {
        Self {
            gpu,
            runtime,
            output_tex,
            workspace,
            prev_mouse_pos: Pos2::new(0.0, 0.0),
            sec_mouse_down: false,
            prim_mouse_down: false,
            central_panel_center: Pos2::new(0.0, 0.0),
        }
    }
}

impl eframe::App for App<'_> {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Joyful Create");
                ui.label("v0.1");
            });
        });

        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.heading("Left Panel");
            ui.label("This is a simple egui app.");

            let max_rect = ui.max_rect();

            egui::TopBottomPanel::bottom("bottom_panel")
                .min_height(max_rect.height() / 2.)
                .show_inside(ui, |ui| {
                    ui.heading("Layers");
                });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.input(|reader| {
                for event in reader.events.iter() {
                    match event {
                        egui::Event::PointerMoved(pos) => {
                            if self.sec_mouse_down {
                                let delta = *pos - self.prev_mouse_pos;
                                self.workspace.pixel_at_center = (
                                    self.workspace.pixel_at_center.0
                                        - (delta.x / self.workspace.zoom),
                                    self.workspace.pixel_at_center.1
                                        - (delta.y / self.workspace.zoom),
                                );
                            }
                            self.prev_mouse_pos = *pos;

                            if self.prim_mouse_down {
                                self.workspace.perform_action(ActionOrigin::MouseMove);
                            }
                        }
                        egui::Event::MouseWheel {
                            unit,
                            delta,
                            modifiers,
                        } => {
                            let mouse = self.prev_mouse_pos;
                            let mouse = mouse - self.central_panel_center;
                            let zoom = self.workspace.zoom;
                            let pixel_over_delta_center = (mouse.x / zoom, mouse.y / zoom);

                            match delta {
                                egui::Vec2 { x, y } => {
                                    let zoom_factor = 1.1;
                                    if *y > 0.0 {
                                        self.workspace.zoom = zoom * zoom_factor;
                                        self.workspace.pixel_at_center = (
                                            self.workspace.pixel_at_center.0
                                                - pixel_over_delta_center.0 * (1. - zoom_factor),
                                            self.workspace.pixel_at_center.1
                                                - pixel_over_delta_center.1 * (1. - zoom_factor),
                                        );
                                    } else {
                                        self.workspace.zoom = zoom / zoom_factor;
                                        self.workspace.pixel_at_center = (
                                            self.workspace.pixel_at_center.0
                                                + pixel_over_delta_center.0 * (1. - zoom_factor),
                                            self.workspace.pixel_at_center.1
                                                + pixel_over_delta_center.1 * (1. - zoom_factor),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                        egui::Event::PointerButton {
                            pos,
                            button,
                            pressed,
                            modifiers,
                        } => match button {
                            egui::PointerButton::Primary => {
                                self.prim_mouse_down = *pressed;
                            }
                            egui::PointerButton::Secondary => {
                                self.sec_mouse_down = *pressed;
                            }
                            _ => {}
                        },
                        egui::Event::Key {
                            key,
                            physical_key,
                            pressed,
                            repeat,
                            modifiers,
                        } => match key {
                            egui::Key::F5 => {
                                if !*pressed {
                                    continue;
                                }
                                self.runtime
                                    .block_on(self.workspace.save("saved.jc", &self.gpu));
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            });
            let size: (u32, u32) = self.workspace.size;
            let zoom: f32 = self.workspace.zoom;
            let size: Vec2 = Vec2::new(size.0 as f32 * zoom, size.1 as f32 * zoom);

            let pixel_at_center: (f32, f32) = self.workspace.pixel_at_center;
            let pixel_at_center: Vec2 =
                Vec2::new(pixel_at_center.0 as f32, pixel_at_center.1 as f32);
            let center: Vec2 = pixel_at_center * zoom;

            let image = Image::new((self.output_tex, size));

            let panel_rect = ui.min_rect();
            let panel_center = panel_rect.center();
            self.central_panel_center = panel_center;

            let top_left = panel_center - center;
            let bottom_right = top_left + size;

            let image_rect = Rect::from_min_max(top_left, bottom_right);

            let image = image.sense(Sense::click());
            ui.put(image_rect, image);
        });
    }
}
