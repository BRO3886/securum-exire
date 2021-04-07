use actix_web::{Responder, web, HttpResponse};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Mutex, Arc};
use std::ops::Deref;
use futures_util::stream::StreamExt as _;
use tokio::task::JoinHandle;
use crate::leak_model::LeakModel;

pub async fn check(data: actix_web::web::Data<Arc<Mutex<RefCell<HashMap<String, String>>>>>,
                   _chan: actix_web::web::Data<tokio::sync::mpsc::Sender<LeakModel>> ,
                   mut payload: web::Payload,
req: actix_web::HttpRequest)
                   -> impl Responder {
    let data = data.deref().lock().unwrap();
    let data = data.borrow();
    let keys = data.keys();


    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        if let Ok(chunk) = chunk {
            body.extend_from_slice(&chunk);
        } else {
            break;
        }
    }
    let c = &body[..];
    let req_payload = String::from_utf8(Vec::from(c)).unwrap();
    let mut f = false;

    let z = keys.map(|i| {
        let c = data.get(i).unwrap().clone();
        let d = req_payload.clone();
        let key = i.clone();
        tokio::task::spawn(async move{
            let x = c;
            let b = d.contains(&x);

            return (b, key);
        })
    }).collect::<Vec<JoinHandle<(bool, String)>>>();
    let result = futures::future::join_all(z).await;

    let mut leaks = Vec::new();
    for i in result {
        if let Ok(v) = i {

            if v.0 {
                leaks.push(v.1);
            }
            f = f || v.0;
        }
    }

    let endpoint = req.headers().get("endpoint");
    let endpoint = match endpoint {
        Some(v) => v.to_str().unwrap(),
        None => ""
    };
    if f {
        let _ = _chan.send(LeakModel{ endpoint: endpoint.to_string(), leaked_credentials: leaks}).await;
        HttpResponse::Forbidden()
    } else {
        HttpResponse::Ok()
    }
}