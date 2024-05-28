use actix_web::http::StatusCode;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use log::{debug, error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

mod config;

#[derive(Clone)]
struct ClientData {
    web_client: Client,
    cfg: config::Config,
}

#[derive(Debug, Deserialize)]
struct QSParams {
    token: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum OtsdbValue {
    String(String),
    Integer(i64),
    Float(f64),
}

#[derive(Debug, Deserialize, Serialize)]
struct OtsdbData {
    metric: String,
    value: OtsdbValue,
    timestamp: i64,
    tags: HashMap<String, OtsdbValue>,
}

const CONFIG_FILE: &str = "config.yaml";

#[actix_web::post("/put")]
async fn put_post(
    shared: web::Data<ClientData>,
    qs: web::Query<QSParams>,
    body: web::Json<OtsdbData>,
) -> impl Responder {
    let authenticated_client = config::try_authenticate_client(&shared.cfg.clients, &qs.token);

    if authenticated_client.is_none() {
        let emsg = format!(
            "Unauthorized. Unknown token: {}. Please specify a valid tokne.",
            qs.token
        );
        error!("{}", emsg);
        return HttpResponse::Unauthorized().body(emsg);
    }

    let client = authenticated_client.unwrap();

    if !client.can_write(&body.metric) {
        let emsg = format!(
            "Not allowed to write metric `{}`. Allowed metrics: {}",
            body.metric,
            client.metrics.join(", ")
        );
        error!("{}", emsg);
        return HttpResponse::Forbidden().body(emsg.to_string());
    }

    let post_url = format!("{}put", shared.cfg.config.opentsdb.url);
    let otsdb_body = serde_json::to_string(&body).unwrap();

    info!(
        "{} sent metric {}={:?}",
        client.name, body.metric, body.value
    );
    debug!("POST {} with body: {}", post_url, otsdb_body);

    let response = shared
        .web_client
        .post(post_url)
        .body(otsdb_body)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();

            let body = resp.text().await.unwrap_or_else(|_| "".to_string());
            debug!("OpenTSDB response {}: {}", status, body);
            let sstatus =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            HttpResponse::Ok().status(sstatus).body(body)
        }
        Err(err) => {
            error!("OpenTSDB error: {}", err);
            HttpResponse::InternalServerError().body(format!("Proxy error: {}", err))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cfg_file = env::var("CONFIG_FILE").unwrap_or(CONFIG_FILE.to_string());
    let cfg = config::load_config_file(&cfg_file);

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    println!("Loaded config: {:#?}", cfg);
    let server_port = cfg.config.server.port.clone();

    let web_client = Client::new();

    let shared = ClientData { web_client, cfg };
    let client_data = web::Data::new(shared);

    HttpServer::new(move || {
        App::new()
            .app_data(client_data.clone())
            .app_data(web::JsonConfig::default().content_type_required(false))
            .wrap(Logger::new("%r %s")) // k8s already logs timestamp
            .service(put_post)
    })
    .bind(format!("[::]:{}", server_port))?
    .run()
    .await
}
