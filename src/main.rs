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
struct QPutParams {
    token: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct OtsdbPutData {
    metric: String,
    value: StringIntFloat,
    timestamp: i64,
    tags: HashMap<String, StringIntFloat>,
}

#[derive(Debug, Deserialize)]
struct QQueryParams {
    #[serde(default)]
    token: String,

    #[serde(flatten)]
    q: OpentsdbQuery,
}

#[derive(Debug, Deserialize, Serialize)]
struct OpentsdbQuery {
    start: StringInt,
    end: Option<StringInt>,
    m: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum StringIntFloat {
    String(String),
    Integer(i64),
    Float(f64),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum StringInt {
    String(String),
    Integer(i64),
}

const CONFIG_FILE: &str = "config.yaml";

fn get_metric(m: &String) -> String {
    let mut metric = m.clone();
    let pts: Vec<&str> = metric.split(":").collect();
    pts[1].to_string()
}

#[actix_web::post("/put")]
async fn put_post(
    shared: web::Data<ClientData>,
    qs: web::Query<QPutParams>,
    body: web::Json<OtsdbPutData>,
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
            "Not allowed to write metric `{}`. Allowed metrics: {} and {}",
            body.metric,
            client.metrics.join(", "),
            client.write_metrics.join(", ") // XXX make it nicer
        );
        error!("{}", emsg);
        return HttpResponse::Forbidden().body(emsg);
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

#[actix_web::get("/query")]
async fn query_get(shared: web::Data<ClientData>, qs: web::Query<QQueryParams>) -> impl Responder {
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

    println!("Query get: {:?}", qs);
    let metric = get_metric(&qs.q.m);

    if !client.can_read(&metric) {
        let emsg = format!("Not allowed to read metric `{}`", metric);
        error!("{}", emsg);
        return HttpResponse::Forbidden().body(emsg);
    }

    let get_url = format!("{}query", shared.cfg.config.opentsdb.url);
    //let otsdb_body = serde_json::to_string(&body).unwrap();
    //let query_string

    info!("{} get metric {}", client.name, metric);
    debug!("GET {} with qs: {:?}", get_url, qs.q);

    let response = shared.web_client.get(get_url).query(&qs.q).send().await;

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
            .service(query_get)
    })
    .bind(format!("[::]:{}", server_port))?
    .run()
    .await
}
