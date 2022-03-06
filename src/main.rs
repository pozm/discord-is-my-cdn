use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use clap::Parser;
use reqwest::multipart::Part;
use reqwest::{Response, StatusCode};
use serde::{Serialize, Deserialize};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Arguments {
    #[clap(short, long)]
    token : String,
    #[clap(short, long)]
    guild_id : i64,
    #[clap(parse(from_os_str))]
    path_dir : PathBuf,
    #[clap(short, long,default_value = "0")]
    offset : i32,
    #[clap(short, long)]
    channel_id : Option<i64>,

    #[clap(short='f', long)]
    get_messages : bool,
    // #[clap(short, long, default_value = "poggers")]
    // channel_name : String,
}

fn get_all_file_paths(paths: &mut Vec<String>, path:&Path) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            get_all_file_paths(paths, &path);
        } else if path.is_file() {
            paths.push(path.display().to_string());
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
struct CreateChannelPayload {
    name: String,
    #[serde(rename = "type")]
    _type: i32,
    permission_overwrites: Vec<String>
}

#[derive(Serialize, Deserialize, Debug)]
struct Attachment {
    id: String,
    filename:String,
    url: Option<String>
}

#[derive(Serialize, Deserialize, Debug)]
struct MessagePayload {
    content: String,
    #[serde(rename = "type")]
    _type: i32,
    sticker_ids: Vec<String>,
    attachments: Vec<Attachment>
}


#[derive(Serialize, Deserialize, Debug)]
struct Channel {
    id: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct Message {
    id: String,
    attachments: Vec<Attachment>
}
#[derive(Serialize, Deserialize, Debug)]
struct RateLimitError {
    code: i32,
    global: bool,
    message: String,
    retry_after: f32
}

async fn upload_image(ch:i64, filep:&Path, api_ver:Option<i8>, auth:&str) -> Result<Message,Response> {
    let api = format!("https://discordapp.com/api/v{}/channels/{}/messages",api_ver.unwrap_or(9),ch);
    let http = reqwest::Client::new();
    let part = Part::bytes(fs::read(filep).unwrap()).file_name("poggers.png");
    println!("uploading : {:?}", &part);
    let multi_part = reqwest::multipart::Form::new()
        .part("files[0]",part);
    let dat = http.post(api.to_string()).multipart(multi_part).header("authorization",auth).send().await.unwrap();
    if dat.status().is_success() {
        let dat = dat.json::<Message>().await.unwrap();
        Ok(dat)
    } else {
        Err(dat)
    }
}


#[tokio::main]
async fn main() {
    let args : Arguments = Arguments::parse();
    let mut paths:Vec<String> = vec![];
    get_all_file_paths(&mut paths, &args.path_dir);
    let http = reqwest::Client::new();
    let mut messages: Vec<Message> = vec![];
    let mut api:u32 = 0;
    let discord_api = "https://discord.com/api/v9/";

    let upload_id : String = if let Some(id) = args.channel_id{
        id.to_string()
    } else {
        let time_since_epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        let ch_payload = CreateChannelPayload {
            name: format!("poggers{}",time_since_epoch.as_secs()).to_string(),
            _type: 0,
            permission_overwrites: vec![]
        };
        let data = http.post(format!("{}guilds/{}/channels", discord_api, args.guild_id).as_str()).json(&ch_payload).header("authorization",args.token.as_str()).send().await.unwrap();
        let channel = data.json::<Channel>().await.unwrap();
        channel.id
    };

    if args.get_messages {
        let mut offset = 0;
        loop {
            let before = if messages.len() == 0 {"".to_string()} else {format!("before={}",messages.last().unwrap().id)};
            let data = http.get(format!("{}channels/{}/messages?limit=100&{}", discord_api, upload_id, before ).as_str()).header("authorization",args.token.as_str()).send().await.unwrap();
            let messages_ = data.json::<Vec<Message>>().await.unwrap();
            if messages_.len() == 0 {
                break;
            }
            println!("{:?} | {}",messages_.len(),before);
            messages.extend(messages_);
            offset += 100;
            std::thread::sleep(std::time::Duration::from_secs(3));
        }

        let f= File::create("out.json").unwrap();
        serde_json::to_writer_pretty(f, &messages).unwrap();
        return;
    }


    for (idx,path) in paths.iter().enumerate() {
        if idx < args.offset as usize {
            println!("skipping {} cuz offset", path);
            continue;
        }
        println!("{}/{}", idx, paths.len());
        let msg = upload_image(upload_id.parse::<i64>().unwrap(), Path::new(&path), Some(((api % 4) + 6) as i8), args.token.as_str()).await;
        match msg {
            Ok (msg) => {
                println!("got ok msg");
                messages.push(msg);
            }
            Err(e) => {
                println!("got err msg");
                if e.status().as_u16() == 429 {
                    let text = e.text().await.unwrap_or("{}".to_string());
                    let rate_limit = serde_json::from_str::<RateLimitError>(&text);
                    match rate_limit {
                        Ok (rl) => {                            
                            println!("got rate limit {} | {}", rl.retry_after,text);
                            std::thread::sleep(std::time::Duration::from_secs(rl.retry_after as u64 + 10u64));
                            let msg = upload_image(upload_id.parse::<i64>().unwrap(), Path::new(&path), Some(((api % 4) + 6) as i8), args.token.as_str()).await;
                            if let Ok(pog) = msg {
                                println!("got ok msg");
                                messages.push(pog);
                            } else {
                                println!("got err msg");
                                println!("{:?}", msg);
                            }
                        },
                        Err(er) => {
                            println!("{:?}, {:?}",er,text)
                        }
                    }
                } else {
                    println!("{:?}", e);
                }
            }
        }
        api += 1;
        if (api % 4) == 0 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    let f= File::create("out.json").unwrap();
    serde_json::to_writer_pretty(f, &messages).unwrap();

}
