use actix_web::{get, post, web, HttpResponse, Responder};

use crate::WebAppState;

use crate::messages::CommandMsg;

/// Web routes

#[get("/ready")]
pub async fn ready() -> impl Responder {
    HttpResponse::Ok().body("supers ready\n")
}

#[get("/app")]
pub async fn get_app_status(data: web::Data<WebAppState>) -> impl Responder {
    let d = data.app_state.lock().unwrap();
    let status = &d.application_status;
    let body = format!("App status is: {}\n", status);
    HttpResponse::Ok().body(body)
}

#[get("/programs")]
pub async fn get_programs(data: web::Data<WebAppState>) -> impl Responder {
    let d = data.app_state.lock().unwrap();
    let mut body = String::from("Program Statuses:\n");
    for (key, val) in &d.programs {
        let s = format!("{}: {}\n", key, val);
        body.push_str(&s);
    }

    HttpResponse::Ok().body(body)
}

#[get("/programs/{name}")]
pub async fn get_program(
    data: web::Data<WebAppState>,
    path: web::Path<(String,)>,
) -> impl Responder {
    let name = &path.0;
    let d = data.app_state.lock().unwrap();
    if !d.programs.contains_key(name) {
        let body = format!("No program with name {} found.\n", &name);
        return HttpResponse::NotFound().body(body);
    }
    let status = d.programs.get(name).unwrap();
    let body = format!("Status of program {} is: {}\n", name, &status);
    HttpResponse::Ok().body(body)
}

#[post("/programs/{name}/start")]
pub async fn start_program(
    data: web::Data<WebAppState>,
    path: web::Path<(String,)>,
) -> impl Responder {
    let name = &path.0;
    let d = data.app_state.lock().unwrap();
    // check that `name` is an existing program
    if !d.programs.contains_key(name) {
        let body = format!("No program with name {} found.\n", &name);
        return HttpResponse::NotFound().body(body);
    }

    // get the channel associated with this program and send it a start message
    let tx = data.channels.get(name).unwrap();
    if let Ok(_r) = tx.send(CommandMsg::Start) {
        let body = format!("Program {} has been instructed to start.\n", name);
        HttpResponse::Ok().body(body)
    } else {
        let body = format!("Error sending message to {} channel\n", name);
        HttpResponse::BadRequest().body(body)
    }
}

#[post("/programs/{name}/stop")]
pub async fn stop_program(
    data: web::Data<WebAppState>,
    path: web::Path<(String,)>,
) -> impl Responder {
    let name = &path.0;
    let d = data.app_state.lock().unwrap();
    // check that `name` is an existing program
    if !d.programs.contains_key(name) {
        let body = format!("No program with name {} found.\n", &name);
        return HttpResponse::NotFound().body(body);
    }

    // get the channel associated with this program and send it a stop message
    let tx = data.channels.get(name).unwrap();
    if let Ok(_r) = tx.send(CommandMsg::Stop) {
        let body = format!("Program {} has been instructed to stop.\n", name);
        HttpResponse::Ok().body(body)
    } else {
        let body = format!("Error sending message to {} channel\n", name);
        HttpResponse::BadRequest().body(body)
    }
}

#[post("/programs/{name}/restart")]
pub async fn restart_program(
    data: web::Data<WebAppState>,
    path: web::Path<(String,)>,
) -> impl Responder {
    let name = &path.0;
    let d = data.app_state.lock().unwrap();
    // check that `name` is an existing program
    if !d.programs.contains_key(name) {
        let body = format!("No program with name {} found.\n", &name);
        return HttpResponse::NotFound().body(body);
    }

    // get the channel associated with this program and send it a restart message
    let tx = data.channels.get(name).unwrap();
    if let Ok(_r) = tx.send(CommandMsg::Restart) {
        let body = format!("Program {} has been instructed to restart.\n", name);
        HttpResponse::Ok().body(body)
    } else {
        let body = format!("Error sending message to {} channel\n", name);
        HttpResponse::BadRequest().body(body)
    }
}
