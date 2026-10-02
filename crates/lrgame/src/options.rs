use std::path::PathBuf;

#[derive(Clone)]
pub struct Options {
    pub play:bool,
    pub car:String,
    pub driver:String,
    pub race_name:Option<String>,
    pub time_trial:bool,
    pub profile:Option<PathBuf>,
    pub menu_smoke:bool,
    pub menu_views_only:bool,
    pub award_smoke:bool,
    pub reward_smoke:Option<String>,
    pub versus:bool,
    pub versus_smoke:bool,
    pub capture_clip:bool,
    pub menu_preview_smoke:bool,
    pub music:bool,
    pub sound:bool,
    pub mirrored:bool,
    pub smoke_race:Option<String>,
    pub builder_smoke:bool,
    pub driver_smoke:bool,
    pub custom_build:Option<lrsim::brick_build::Build>,
    pub custom_driver:Option<crate::custom_driver::Build>,
    pub drive: bool,
    pub jam: PathBuf,
    pub table: String,
    pub capture: Option<PathBuf>,
    pub frames: Option<u32>,
    pub scripted_camera: bool,
    pub scripted_drive: bool,
    pub scripted_route: bool,
    pub finish_tail_frames:u32,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let first=args.next();
        let play=first.is_none()||first.as_deref()==Some("--play");
        let drive = match first.as_deref() {
            None|Some("--play")=>true,
            Some("--preview") => false,
            Some("--drive") => true,
            _ => return Err("usage: lrracers <--preview|--drive> <LEGO.JAM> <RACE_TABLE> [--capture DIR --frames N --scripted-camera|--scripted-drive]".into()),
        };
        let jam = if first.is_none() {PathBuf::from("extracted/Program_Files_Group/LEGO.JAM")} else {args.next().ok_or("missing LEGO.JAM path")?.into()};
        let table = if play {"RACEC0R0".into()} else {args.next().ok_or("missing race table")?};
        let mut options = Self {play,car:"BK".into(),driver:"BK".into(),race_name:None,time_trial:false,profile:None,menu_smoke:false,menu_views_only:false,award_smoke:false,reward_smoke:None,versus:false,versus_smoke:false,capture_clip:false,menu_preview_smoke:false,music:true,sound:true,mirrored:false,smoke_race:None,builder_smoke:false,driver_smoke:false,custom_build:None,custom_driver:None,jam, table, drive, capture: None, frames: None, scripted_camera: false, scripted_drive: false, scripted_route: false,finish_tail_frames:0 };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--capture" => options.capture = Some(args.next().ok_or("missing capture directory")?.into()),
                "--profile"=>options.profile=Some(args.next().ok_or("missing profile path")?.into()),
                "--menu-smoke"=>options.menu_smoke=true,
                "--menu-views-only"=>options.menu_views_only=true,
                "--award-smoke"=>options.award_smoke=true,
                "--reward-smoke"=>options.reward_smoke=Some(args.next().ok_or("missing reward champion")?),
                "--versus-smoke"=>{options.versus_smoke=true;options.versus=true;},
                "--capture-clip"=>options.capture_clip=true,
                "--menu-preview-smoke"=>options.menu_preview_smoke=true,
                "--smoke-race"=>options.smoke_race=Some(args.next().ok_or("missing smoke race name")?),
                "--builder-smoke"=>options.builder_smoke=true,
                "--driver-smoke"=>options.driver_smoke=true,
                "--time-trial"=>options.time_trial=true,
                "--car"=>options.car=args.next().ok_or("missing car ID")?,
                "--driver"=>options.driver=args.next().ok_or("missing driver ID")?,
                "--frames" => {
                    let count = args.next().ok_or("missing frame count")?.parse::<u32>()
                        .map_err(|_| "frame count must be a positive integer")?;
                    if count == 0 || count > 100_000 {
                        return Err("frame count must be between1 and100000".into());
                    }
                    options.frames = Some(count);
                }
                "--scripted-camera" => options.scripted_camera = true,
                "--scripted-drive" => options.scripted_drive = true,
                "--scripted-route" => options.scripted_route = true,
                "--finish-tail-frames"=> {
                    options.finish_tail_frames=args.next().ok_or("missing finish tail frame count")?.parse().map_err(|_|"finish tail frames must be an integer")?;
                    if options.finish_tail_frames>600 {return Err("finish tail frames must be between0and600".into());}
                }
                _ => return Err(format!("unknown option: {arg}")),
            }
        }
        if options.capture.is_some() && options.frames.is_none() {
            return Err("bounded capture requires --frames".into());
        }
        if options.scripted_camera && options.frames.is_none() {
            return Err("scripted camera requires a bounded --frames run".into());
        }
        if options.scripted_drive && (!options.drive || options.frames.is_none()) {
            return Err("scripted drive requires --drive and a bounded --frames run".into());
        }
        if options.scripted_route && (!options.drive || options.frames.is_none()) {
            return Err("scripted route requires --drive and a bounded --frames run".into());
        }
        if options.finish_tail_frames>0 && !options.scripted_route {return Err("finish tail capture requires --scripted-route".into());}
        if options.menu_smoke&&(!options.play||options.profile.is_none()||options.capture.is_none()) {return Err("menu smoke requires --play, --profile and --capture isolation".into());}
        if options.menu_preview_smoke&&(!options.play||options.profile.is_none()||options.capture.is_none()||options.menu_smoke) {return Err("menu preview smoke requires isolated --play, --profile, --capture and no race smoke".into());}
        if options.builder_smoke&&!options.menu_smoke {return Err("builder smoke requires isolated menu smoke".into());}
        if options.driver_smoke&&!options.menu_smoke {return Err("driver smoke requires isolated menu smoke".into());}
        if options.menu_views_only&&!options.menu_smoke {return Err("menu views require isolated menu smoke".into());}
        if options.award_smoke&&(!options.play||options.profile.is_none()||options.capture.is_none()||options.menu_smoke) {return Err("award smoke requires isolated --play, --profile, --capture and no race smoke".into());}
        if options.versus_smoke&&(!options.play||options.profile.is_none()||options.capture.is_none()||options.menu_smoke||options.award_smoke||options.menu_preview_smoke) {return Err("versus smoke requires isolated --play, --profile, --capture and no other smoke mode".into());}
        if options.capture_clip&&!options.versus_smoke {return Err("progress clip capture requires isolated --versus-smoke".into());}
        if options.reward_smoke.is_some()&&(!options.play||options.profile.is_none()||options.capture.is_none()||options.menu_smoke||options.award_smoke||options.versus_smoke||options.menu_preview_smoke) {return Err("single reward smoke requires isolated --play, --profile, --capture and no other smoke mode".into());}
        Ok(options)
    }
}
