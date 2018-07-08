#![feature(plugin)]
#![plugin(rocket_codegen)]

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

extern crate chashmap;
extern crate chrono;
extern crate failure;
extern crate iota_lib_rs;
extern crate rocket;
extern crate rocket_contrib;
extern crate uuid;

use chashmap::CHashMap;
use chrono::prelude::*;
use failure::Error;
use iota_lib_rs::crypto::PearlDiver;
use iota_lib_rs::iri_api::responses::AttachToTangleResponse;
use iota_lib_rs::model::Transaction;
use iota_lib_rs::utils::converter;
use rocket::State;
use rocket_contrib::Json;
use uuid::Uuid;

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;

lazy_static! {
    static ref RESULT_MAP: CHashMap<String, AttachToTangleResponse> = CHashMap::new();
    static ref MAX_TIMESTAMP_VALUE: i64 = (3_i64.pow(27) - 1) / 2;
}

#[derive(Default, Serialize, Deserialize)]
struct PowRequest {
    #[serde(default)]
    id: String,
    #[serde(rename = "trunkTransaction")]
    trunk_transaction: String,
    #[serde(rename = "branchTransaction")]
    branch_transaction: String,
    #[serde(rename = "minWeightMagnitude")]
    min_weight_magnitude: usize,
    trytes: Vec<String>,
}

struct SendState {
    tx: Mutex<Sender<String>>,
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/<id>")]
fn check_status(id: String) -> Result<Json, Error> {
    let check = RESULT_MAP.get(&id);
    let result = match check {
        Some(res) => Json(serde_json::to_value(&*res)?),
        None => Json(json!("Not ready yet")),
    };
    Ok(result)
}

#[post("/", data = "<task>")]
fn pow(
    mut task: Json<PowRequest>,
    sender: State<SendState>,
) -> Result<Json<AttachToTangleResponse>, Error> {
    let uuid = Uuid::new_v4().simple().to_string();
    println!("id: {}", uuid);
    task.id = uuid.clone();
    sender
        .tx
        .lock()
        .unwrap()
        .send(serde_json::to_string(&task.into_inner())?)?;
    let result = Json(AttachToTangleResponse::new(0, Some(uuid), None, None, None));
    Ok(result)
}

fn main() -> Result<(), Error> {
    let (pow_tx, pow_rx): (Sender<String>, Receiver<String>) = mpsc::channel();

    thread::spawn(move || {
        let mut pearl_diver = PearlDiver::default();
        for work in pow_rx.iter() {
            let request: PowRequest = serde_json::from_str(&work).unwrap_or_default();
            let id = request.id.clone();
            let result = process_request(&mut pearl_diver, request);
            match result {
                Ok(res) => RESULT_MAP.insert(id, res),
                Err(e) => RESULT_MAP.insert(id.clone(), AttachToTangleResponse::new(0, Some(id), Some(e.to_string()), None, None))
            };
        }
    });

    rocket::ignite()
        .manage(SendState {
            tx: Mutex::new(pow_tx),
        })
        .mount("/", routes![index, pow, check_status])
        .launch();
    Ok(())
}

fn process_request(pearl_diver: &mut PearlDiver, request: PowRequest) -> Result<AttachToTangleResponse, Error> {
    let trytes = request.trytes;
    let trunk_transaction = request.trunk_transaction;
    let branch_transaction = request.branch_transaction;
    let min_weight_magnitude = request.min_weight_magnitude;

    let mut result_trytes: Vec<String> = Vec::with_capacity(trytes.len());
    let mut previous_transaction: Option<String> = None;
    for i in 0..trytes.len() {
        let mut tx: Transaction = trytes[i].parse()?;
        let new_trunk_tx = if let Some(previous_transaction) = &previous_transaction {
            previous_transaction.to_string()
        } else {
            trunk_transaction.to_string()
        };
        tx.set_trunk_transaction(new_trunk_tx);

        let new_branch_tx = if previous_transaction.is_some() {
            trunk_transaction.to_string()
        } else {
            branch_transaction.to_string()
        };
        tx.set_branch_transaction(new_branch_tx);

        let tag = tx.tag().unwrap_or_default();
        if tag.is_empty() || tag == "9".repeat(27) {
            *tx.tag_mut() = tx.obsolete_tag();
        }
        tx.set_attachment_timestamp(Utc::now().timestamp_millis());
        tx.set_attachment_timestamp_lower_bound(0);
        tx.set_attachment_timestamp_upper_bound(*MAX_TIMESTAMP_VALUE);
        let mut tx_trits = converter::trits_from_string(&tx.to_trytes());

        pearl_diver.search(&mut tx_trits, min_weight_magnitude)?;
        result_trytes.push(converter::trits_to_string(&tx_trits)?);
        previous_transaction = result_trytes[i].parse::<Transaction>()?.hash();
    }
    result_trytes.reverse();
    Ok(AttachToTangleResponse::new(0, None, None, None, Some(result_trytes)))
}
