use std::collections::{HashMap, VecDeque};
use std::future::{ready, Ready};

use actix_web::web::{Data, Json};
use actix_web::{
    error, get, App, FromRequest, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;


// === "Domain" types ===

#[derive(Serialize, PartialEq)]
enum TestStatus {
    #[serde(rename = "ongoing")]
    Ongoing,
    #[serde(rename = "done")]
    Done,
}

struct AppState {
    build_map: Mutex<HashMap<String, Build>>,
}

struct Build {
    instance_map: HashMap<String, Instance>,
    created_on: chrono::DateTime<Utc>,
    test_spec_list: VecDeque<String>,
}

#[derive(Serialize)]
struct Instance {
    test_list: Vec<String>,
    test_status: TestStatus,
    is_master: bool,
}

// === Extractors ===

struct RequestMeta {
    build_id: String,
    instance_id: String,
    token: String,
}

impl FromRequest for RequestMeta {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        if let Some(meta) = get_meta(req) {
            let config = req.app_data::<Data<Settings>>().unwrap();
            if meta.token == config.token {
                return ready(Ok(meta));
            }
        }
        ready(Err(error::ErrorBadRequest(
            "Expected meta info like build id, instance id and token",
        )))
    }
}

fn get_meta(req: &HttpRequest) -> Option<RequestMeta> {
    let headers = req.headers();
    Some(RequestMeta {
        build_id: headers.get("CI-BUILD-ID")?.to_str().ok()?.to_string(),
        instance_id: headers.get("CI-INSTANCE-ID")?.to_str().ok()?.to_string(),
        token: headers.get("REPO-TOKEN")?.to_str().ok()?.to_string(),
    })
}

// === Handlers ===

#[get("/")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(json!({ "status": "Online" }))
}

#[derive(Deserialize)]
struct RegisterInstanceData {
    test_spec_list: VecDeque<String>,
}

#[get("/register-instance")]
async fn register_instance(
    state: Data<AppState>,
    specs: Json<RegisterInstanceData>,
    meta: RequestMeta,
) -> Result<impl Responder> {
    let mut build_map = state.build_map.lock().await;
    clear_old_data(&mut build_map).await;

    let build = build_map.entry(meta.build_id).or_insert(Build {
        created_on: Utc::now(),
        instance_map: HashMap::default(),
        test_spec_list: specs.test_spec_list.clone(),
    });

    build.instance_map.insert(
        meta.instance_id.clone(),
        Instance {
            test_list: vec![],
            test_status: TestStatus::Ongoing,
            // First one becomes "master"
            is_master: build.instance_map.is_empty(),
        },
    );

    return Ok(HttpResponse::Ok().json(build.instance_map.get(&meta.instance_id).unwrap()));
}

#[get("/get-next-test-spec")]
async fn next_spec(meta: RequestMeta, state: Data<AppState>) -> Result<impl Responder> {
    let mut build_map = state.build_map.lock().await;

    let next_test = build_map
        .get_mut(&meta.build_id)
        .ok_or_else(|| error::ErrorBadRequest("Build not found"))?
        .test_spec_list
        .pop_front()
        .unwrap_or_else(|| "".to_string());

    build_map
        .get_mut(&meta.build_id)
        .ok_or_else(|| error::ErrorBadRequest("Build not found"))?
        .instance_map
        .get_mut(&meta.instance_id)
        .ok_or_else(|| error::ErrorBadRequest("Instance not found"))?
        .test_list
        .push(next_test.clone());

    Ok(HttpResponse::Ok().json(json!({
        "status": if next_test.is_empty() { TestStatus::Done } else { TestStatus::Ongoing },
        "next_test": next_test
    })))
}

#[get("/test-completed")]
async fn test_completed(meta: RequestMeta, state: Data<AppState>) -> Result<impl Responder> {
    let mut build_map = state.build_map.lock().await;

    build_map
        .get_mut(&meta.build_id)
        .ok_or_else(|| error::ErrorBadRequest("Build not found"))?
        .instance_map
        .get_mut(&meta.instance_id)
        .ok_or_else(|| error::ErrorBadRequest("Instance not found"))?
        .test_status = TestStatus::Done;

    Ok(HttpResponse::Ok().json(json!({})))
}

#[get("/reset")]
async fn reset_data(meta: RequestMeta, state: Data<AppState>) -> Result<impl Responder> {
    let mut build_map = state.build_map.lock().await;
    build_map.remove(&meta.build_id);
    Ok(HttpResponse::Ok())
}

async fn clear_old_data(build_map: &mut HashMap<String, Build>) {
    let threshold = Utc::now() - chrono::Duration::hours(2);

    let expired_builds: Vec<String> = build_map
        .iter()
        .filter(|(_, build)| build.created_on < threshold)
        .map(|(id, _)| id.clone())
        .collect();

    expired_builds.iter().for_each(|id| {
        let _ = &build_map.remove(id);
    });
}

// === Configuration ===

#[derive(Deserialize, Clone)]
struct Settings {
    port: u16,
    token: String,
}

fn get_configuration() -> Settings {
    config::Config::builder()
        .add_source(config::File::with_name("config"))
        .build()
        .unwrap()
        .try_deserialize()
        .unwrap()
}

// === Startup ===

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let state = Data::new(AppState {
        build_map: Mutex::new(HashMap::new()),
    });

    let config = get_configuration();
    let port = config.port;

    HttpServer::new(move || {
        App::new()
            .service(health_check)
            .service(register_instance)
            .service(next_spec)
            .service(test_completed)
            .service(reset_data)
            .app_data(state.clone())
            .app_data(Data::new(config.clone()))
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}
