use async_recursion::async_recursion;
use clap::Parser;
use futures::StreamExt;
use rand::Rng;
use reqwest::multipart::Part;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::sync::Semaphore;

// This struct can be cloned to share it!
#[derive(Clone)]
struct HttpClient {
    client: reqwest::Client,
    semaphore: Arc<tokio::sync::Semaphore>,
}
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Arguments {
    #[clap(short, long)]
    token: String,
    #[clap(short, long)]
    guild_id: i64,
    #[clap(parse(from_os_str))]
    path_dir: PathBuf,
    #[clap(short, long, default_value = "0")]
    offset: i32,
    #[clap(short, long)]
    channel_id: Option<i64>,

    #[clap(short = 'f', long)]
    get_messages: bool,
    #[clap(short, long, default_value = "poggers")]
    name: String,
}

fn get_all_file_paths(paths: &mut Vec<String>, path: &Path) {
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
    permission_overwrites: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Attachment {
    id: String,
    filename: String,
    url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MessagePayload {
    content: String,
    #[serde(rename = "type")]
    _type: i32,
    sticker_ids: Vec<String>,
    attachments: Vec<Attachment>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Channel {
    id: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct Message {
    id: String,
    attachments: Vec<Attachment>,
}
#[derive(Serialize, Deserialize, Debug)]
struct RateLimitError {
    code: i32,
    global: bool,
    message: String,
    retry_after: f32,
}

async fn upload_image(
    client: &Client,
    ch: i64,
    filep: &Path,
    api_ver: Option<i8>,
    auth: &str,
) -> Result<Message, Option<Response>> {
    let api = format!(
        "https://discordapp.com/api/v{}/channels/{}/messages",
        api_ver.unwrap_or(9),
        ch
    );
    let http = client;
    println!("uploading : {:?} @ {}", filep.file_name().unwrap(),api);
    let part = Part::bytes(fs::read(filep).unwrap()).file_name("poggers.png");
    let multi_part = reqwest::multipart::Form::new().part("files[0]", part);
    let dat = http
        .post(api.to_string())
        .multipart(multi_part)
        .header("authorization", auth)
        .send()
        .await
        .or(Err(None))?;
    if dat.status().is_success() {
        let dat = dat.json::<Message>().await.unwrap();
        Ok(dat)
    } else {
        Err(Some(dat))
    }
}
#[async_recursion]
async fn attempt_upload(
    client: &Client,
    ch: i64,
    filep: String,
    api_ver: i8,
    auth: &str,
    attempts: Option<i32>,
) -> Result<Message, ()> {
    let msg = upload_image(client, ch, Path::new(&filep), Some(api_ver), auth).await;
match msg {
        Ok(msg) => {
            println!("got ok msg");
            return Ok(msg);
        }
        Err(e) => {
            println!("got err msg");
            if attempts.unwrap_or(0) > 5 {
                println!("too many attempts");
                return Err(());
            }

            match e {
                Some(e) => { // request was made
                    match e.status().as_u16() {
                        429 => {
                            // rate limit
                            let text = e.text().await.unwrap_or("{}".to_string());
                            let rate_limit = serde_json::from_str::<RateLimitError>(&text);
                            match rate_limit {
                                Ok(rl) => {
                                    println!("got rate limit {} | {}", rl.retry_after, text);
                                    std::thread::sleep(std::time::Duration::from_secs(
                                        (rl.retry_after as u64 + 10u64).min(20u64),
                                    ));
                                    return attempt_upload(
                                        client,
                                        ch,
                                        filep,
                                        api_ver,
                                        auth,
                                        Some(attempts.unwrap_or(0) + 1),
                                    )
                                    .await;
                                }
                                Err(er) => {
                                    println!("{:?}, {:?}", er, text)
                                }
                            }
                        }
                        413 => {
                            // too big
                            println!("File too big!");
                            return Err(());
                        }
                        _ => {
                            // other error
                            println!("{:?}", e);
                        }
                    }
                }
                None => { // request was not made
                    println!("unable to make request??? retrying. {}",filep);
                    return attempt_upload(
                        client,
                        ch,
                        filep,
                        api_ver,
                        auth,
                        Some(attempts.unwrap_or(0) + 1),
                    )
                    .await;
                }
            }

        }
    }
    Err(())
}

#[tokio::main]
async fn main() {
    let args: Arguments = Arguments::parse();
    let mut paths: Vec<String> = vec![];
    get_all_file_paths(&mut paths, &args.path_dir);
    let http = reqwest::Client::new();
    let mut messages: Vec<Message> = vec![];
    let mut api: u32 = 0;
    let discord_api = "https://discord.com/api/v9/";

    let upload_id: String = if let Some(id) = args.channel_id {
        id.to_string()
    } else {
        let time_since_epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let ch_payload = CreateChannelPayload {
            name: format!("{}{}", &args.name, time_since_epoch.as_secs()).to_string(),
            _type: 0,
            permission_overwrites: vec![],
        };
        let data = http
            .post(format!("{}guilds/{}/channels", discord_api, args.guild_id).as_str())
            .json(&ch_payload)
            .header("authorization", args.token.as_str())
            .send()
            .await
            .unwrap();

        if data.status().is_success() {
            let channel = data.json::<Channel>().await.unwrap();
            channel.id
        } else {
            println!("{:?}", data.text().await.unwrap());
            panic!("Failed to create channel");
            String::from("0")
        }
    };

    if args.get_messages {
        loop {
            let before = if messages.len() == 0 {
                "".to_string()
            } else {
                format!("before={}", messages.last().unwrap().id)
            };

            match http
                .get(
                    format!(
                        "{}channels/{}/messages?limit=100&{}",
                        discord_api, upload_id, before
                    )
                    .as_str(),
                )
                .header("authorization", args.token.as_str())
                .send()
                .await {
                    Ok(data) => {
                        match data.status().is_success() {
                            true => {
                                let messages_ = data.json::<Vec<Message>>().await.unwrap();
                                if messages_.len() == 0 {
                                    break;
                                }
                                println!("{:?} | {}", messages_.len(), before);
                                messages.extend(messages_);
            
                            }
                            false => {
                                match data.status().as_u16() {
                                    429 => {
                                        // rate limit
                                        let text = data.text().await.unwrap_or("{}".to_string());
                                        let rate_limit = serde_json::from_str::<RateLimitError>(&text);
                                        match rate_limit {
                                            Ok(rl) => {
                                                println!("got rate limit {} | {}", rl.retry_after, text);
                                                std::thread::sleep(std::time::Duration::from_secs(
                                                    (rl.retry_after as u64 + 10u64).min(20u64),
                                                ));
                                            }
                                            Err(er) => {
                                                println!("{:?}, {:?}", er, text)
                                            }
                                        }
                                    }
                                    _ => {
                                        // other error
                                        println!("another error {:?}", data.status().as_str());
                                    }
                                }
                            }
                        }

                    }
                    _=>{
                        println!("failed to get messages");
                    }
                }

            let mut rng = rand::thread_rng();
            
            let ms = rng.gen_range(0.3..1.0);

            std::thread::sleep(std::time::Duration::from_millis((ms*1000f64).floor() as u64));
        }

        let f = File::create(format!("{}out.json",args.name)).unwrap();
        serde_json::to_writer_pretty(f, &messages).unwrap();
        return;
    }
    let mut file_offset = args.offset;
    let mut safe_api = Arc::new(Mutex::new(api));
    let mut safe_messages = messages;
    let mut safe_upload_id = upload_id;
    let mut safe_token = args.token.clone();
    // let mut safe_http = Arc::new(Mutex::new(http));
    loop {
        let files = paths
            .clone()
            .into_iter()
            .skip(file_offset as usize)
            .take(4)
            .collect::<Vec<String>>();
        file_offset += 4;
        if files.len() == 0 {
            break;
        }
        // let reqs: Vec<_> = files.into_iter().map(|path| {
        //     let api = Arc::clone(&safe_api);
        //     let uid = Arc::clone(&safe_upload_id);
        //     let token = Arc::clone(&safe_token);
        //     tokio::spawn(async move {
        //         let mut api_ver = api.lock().unwrap();
        //         *api_ver += 1;
        //         attempt_upload((*uid.lock().unwrap()).parse::<i64>().unwrap(), path.clone(), ((*api_ver % 4) + 6) as i8, token.lock().unwrap().as_str(), None).await
        //     })
        // }).collect();
        // let http_client = HttpClient{
        //     client: http.clone(),
        //     semaphore: Arc::new(Semaphore::new(4))
        // };
        let reqs = futures::future::join_all(files.into_iter().map(|pathx| {
            println!("{:?} upload batch", &pathx);
            let mut api_ver = Arc::clone(&safe_api);
            let mut api_ver = api_ver.lock().unwrap();
            *api_ver += 1;
            let api_to_use = (((api_ver.clone() % 4) + 6) as i8).clone();
            let uid = &safe_upload_id;
            let token = &safe_token;
            let poggers = &http;
            async move {
                attempt_upload(
                    poggers,
                    (*uid).parse::<i64>().unwrap(),
                    pathx.clone(),
                    api_to_use.clone(),
                    token.as_str(),
                    None,
                )
                .await
            }
        }))
        .await;
        for r in reqs {
            println!("{:?}", r);
        }
        // for req in reqs {
        //     println!("{:?}",req.await);
        // }
        println!("new offset = {}", file_offset);
        std::thread::sleep(std::time::Duration::from_secs(7));
    }
    // let mut futures = vec![];
    // for (idx,path) in paths.iter().enumerate() {
    //     if idx < args.offset as usize {
    //         println!("skipping {} cuz offset was set.", path);
    //         continue;
    //     }
    //     let m = attempt_upload(upload_id.parse::<i64>().unwrap(), path.clone(), ((api % 4) + 6) as i8, args.token.as_str(), None);
    //     futures.push(m.boxed());
    //     // if let Ok(msg) = attempt_upload(upload_id.parse::<i64>().unwrap(), path.clone(), ((api % 4) + 6) as i8, args.token.as_str(), None).await {
    //     //     messages.push(msg);
    //     // } else {
    //     //     println!("failed to upload {}", path);
    //     // }
    //     println!("{}/{}", idx, paths.len());
    //     api += 1;
    //     if (api % 4) == 0 {
    //         for mut f in futures::future::join_all(&futures).await {
    //             if let Ok(msg) = f.await {
    //                 messages.push(msg);
    //             } else {
    //                 println!("failed to upload batch {:?}", &paths);
    //             }
    //         }
    //         futures.clear();
    //         std::thread::sleep(std::time::Duration::from_secs(2));
    //     }
    // }

    let f = File::create("out.json").unwrap();
    serde_json::to_writer_pretty(f, &*safe_messages).unwrap();
}
