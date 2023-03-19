use lazy_static::lazy_static;
use oxipng::{optimize_from_memory, Options};
use worker::{
    console_log, event, Context, Cors, Date, Env, FormEntry, Headers, Method, Request, Response,
    Result, Router,
};

mod utils;

lazy_static! {
    static ref CORS: Cors = Cors::default()
        .with_max_age(86400)
        .with_origins(vec!["*"])
        .with_methods(vec![
            Method::Get,
            Method::Head,
            Method::Post,
            Method::Options,
        ]);
}

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or_else(|| "unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    log_request(&req);

    utils::set_panic_hook();

    let router = Router::new();

    router
        .post_async("/form", |mut req, _| async move {
            return match req.form_data().await {
                Ok(form) => match form.get("image") {
                    Some(FormEntry::File(file)) => {
                        let file_bytes = file.bytes().await;

                        let mode = match form.get("mode") {
                            Some(FormEntry::Field(m)) => match m.as_str() {
                                "1" | "2" | "3" | "4" | "5" | "6" => m.parse().unwrap(),
                                _ => 6,
                            },
                            _ => 6,
                        };

                        let opts = Options::from_preset(mode);
                        let mut headers = Headers::new();
                        headers.set("content-type", "image/png")?;

                        let bytes = file_bytes?;

                        // Check file for PNG header
                        // [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
                        if *&bytes[0..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
                            return Response::error("Invalid PNG header detected", 400);
                        }

                        return match optimize_from_memory(&bytes, &opts) {
                            Ok(optimized) => Response::from_bytes(optimized),
                            Err(e) => Response::error(e.to_string(), 500),
                        };
                    }
                    Some(FormEntry::Field(_)) => Response::error("Invalid file", 400),
                    _ => Response::error("There was no 'image' key in Form-Data", 400),
                },
                Err(_) => Response::error("There was no Form-Data", 400),
            };
        })
        .options("/", |req, _ctx| {
            let headers = req.headers();
            if let (Some(_), Some(_), Some(_)) = (
                headers.get("Origin").transpose(),
                headers.get("Access-Control-Request-Method").transpose(),
                headers.get("Access-Control-Request-Headers").transpose(),
            ) {
                Response::empty().and_then(|resp| resp.with_cors(&CORS))
            } else {
                Response::empty()
            }
        })
        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .run(req, env)
        .await
}
