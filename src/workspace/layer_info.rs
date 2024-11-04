use image::{ImageBuffer, ImageFormat, ImageReader, Luma, Rgba};
use serde::{Deserialize, Serialize};
use wgpu::Texture;

#[derive(Serialize, Deserialize, Debug)]
pub struct LayerInfo {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
}

pub struct LayerCreationInfo {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub init_texture: Option<Texture>,
    pub init_image: Option<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    pub init_rgba: Option<[u8; 4]>,
    pub init_mask_texture: Option<Texture>,
    pub init_mask_image: Option<ImageBuffer<Luma<u8>, Vec<u8>>>,
}

impl Default for LayerCreationInfo {
    fn default() -> Self {
        Self {
            name: "New Layer".to_string(),
            visible: true,
            opacity: 1.0,
            blend_mode: "normal".to_string(),
            init_texture: None,
            init_image: None,
            init_rgba: None,
            init_mask_texture: None,
            init_mask_image: None,
        }
    }
}

impl From<LayerCreationInfo> for LayerInfo {
    fn from(info: LayerCreationInfo) -> Self {
        Self {
            name: info.name,
            visible: info.visible,
            opacity: info.opacity,
            blend_mode: info.blend_mode,
        }
    }
}

pub type BlendMode = String;
