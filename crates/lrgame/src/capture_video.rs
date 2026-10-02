//! Opt-in local encoding of actual framebuffer samples, never the user's screen.
use std::{io::Write,path::{Path,PathBuf},process::{Child,ChildStdin,Command,Stdio}};
use macroquad::prelude::Image;
pub struct Video {child:Child,input:Option<ChildStdin>,path:PathBuf,width:u16,height:u16,frames:u32}
impl Video {
    pub fn start(path:&Path,width:u16,height:u16)->Result<Self,String> {
        if width==0||height==0||width%2!=0||height%2!=0 {return Err("clip dimensions must be positive even numbers".into());}
        let mut child=Command::new("ffmpeg").args(["-hide_banner","-loglevel","warning","-n","-f","rawvideo","-pixel_format","rgba","-video_size",&format!("{width}x{height}"),"-framerate","30","-i","pipe:0","-vf","vflip","-c:v","libx264","-preset","fast","-crf","20","-pix_fmt","yuv420p","-an","-movflags","+faststart"]).arg(path).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::from(std::fs::File::create(path.with_extension("ffmpeg.log")).map_err(|e|e.to_string())?)).spawn().map_err(|e|format!("local video encoder: {e}"))?;
        let input=child.stdin.take().ok_or("missing video input pipe")?;Ok(Self {child,input:Some(input),path:path.into(),width,height,frames:0})
    }
    pub fn frame(&mut self,image:&Image)->Result<(),String> {
        if (image.width,image.height)!=(self.width,self.height)||image.bytes.len()!=usize::from(self.width)*usize::from(self.height)*4 {return Err("clip framebuffer dimensions changed".into());}
        self.input.as_mut().ok_or("video encoder already closed")?.write_all(&image.bytes).map_err(|e|format!("framebuffer encoder write: {e}"))?;self.frames+=1;Ok(())
    }
    pub fn finish(mut self)->Result<u32,String> {
        drop(self.input.take());let status=self.child.wait().map_err(|e|e.to_string())?;
        if !status.success()||self.frames==0 {return Err(format!("local clip encoder failed ({status}), {} samples preserved at {}",self.frames,self.path.display()));}Ok(self.frames)
    }
}
impl Drop for Video {fn drop(&mut self) {drop(self.input.take());if self.child.try_wait().ok().flatten().is_none() {let _=self.child.kill();let _=self.child.wait();}}}
