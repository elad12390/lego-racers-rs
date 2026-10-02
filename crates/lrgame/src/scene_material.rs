//! Native one-sided, depth-tested scene geometry using original mesh winding.
use macroquad::prelude::*;
use macroquad::miniquad::{CullFace,FrontFaceOrder,Comparison,BlendState,BlendFactor,BlendValue,Equation,PipelineParams};
use std::{cell::RefCell,collections::HashMap};
thread_local! {static PIPELINES:RefCell<HashMap<Option<[u8;2]>,Material>>=RefCell::new(HashMap::new());}
pub fn load()->Result<Material,String> {
    load_blend(None)
}
pub fn load_blend(blend:Option<[u8;2]>)->Result<Material,String> {
    PIPELINES.with(|cache| {
        if let Some(material)=cache.borrow().get(&blend) {return Ok(material.clone());}
        let material=create(blend)?;cache.borrow_mut().insert(blend,material.clone());Ok(material)
    })
}
fn create(blend:Option<[u8;2]>)->Result<Material,String> {
    let factor=|v|match v {0=>BlendFactor::Zero,1=>BlendFactor::One,2=>BlendFactor::Value(BlendValue::SourceColor),3=>BlendFactor::Value(BlendValue::DestinationColor),4=>BlendFactor::OneMinusValue(BlendValue::SourceColor),5=>BlendFactor::OneMinusValue(BlendValue::DestinationColor),6=>BlendFactor::Value(BlendValue::SourceAlpha),7=>BlendFactor::Value(BlendValue::DestinationAlpha),8=>BlendFactor::OneMinusValue(BlendValue::SourceAlpha),9=>BlendFactor::OneMinusValue(BlendValue::DestinationAlpha),_=>BlendFactor::SourceAlphaSaturate};
    let [source,destination]=blend.unwrap_or([6,8]);
    load_material(ShaderSource::Glsl {vertex:VERTEX,fragment:FRAGMENT},MaterialParams {pipeline_params:PipelineParams {
        cull_face:CullFace::Back,front_face_order:FrontFaceOrder::CounterClockwise,depth_test:Comparison::LessOrEqual,depth_write:blend.is_none(),
        color_blend:Some(BlendState::new(Equation::Add,factor(source),factor(destination))),..Default::default()
    },..Default::default()}).map_err(|e|e.to_string())
}
const VERTEX:&str=r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main(){gl_Position=Projection*Model*vec4(position,1.0);color=color0/255.0;uv=texcoord;}
"#;
const FRAGMENT:&str=r#"#version 100
varying lowp vec4 color;
varying lowp vec2 uv;
uniform sampler2D Texture;
void main(){gl_FragColor=color*texture2D(Texture,uv);if(gl_FragColor.a<0.001)discard;}
"#;
