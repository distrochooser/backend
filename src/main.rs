extern crate iron;
extern crate router;
extern crate hyper;
extern crate rustc_serialize;

use iron::prelude::*;
use router::Router;
use hyper::header::{ContentType};
use iron::status;
use rustc_serialize::json;

static NAME:  &'static str = "Rusty Distrochooser";
static VERSION:  &'static str = "3.0.0";
static DEBUG: bool = true;

fn main() {
    println!("Starting {} {}...",NAME, VERSION);
    let mut router = Router::new();
    router.get("/", index, "index"); 
    router.get("/distributions", distributions,"distros"); 
    Iron::new(router).http("localhost:3000").unwrap();
}
fn middleware(request: &mut Request){
    let target: String = format!("{:?}",request.url.path()[0]).replace("\"","");
    let client = request.remote_addr.ip(); //TODO: Censor IP
    if (DEBUG){
        println!("Serving.. /{} for {:?}",target,client);
    }
}
/**
* Routes
*/
//"_" suppresses unused warning
fn index(_request: &mut Request) -> IronResult<Response> {    
    middleware(_request);
    Ok(Response::with((status::Ok, "fsaf!")))
}
fn distributions(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    let ubuntu = Distro{
        Name: String::from("Ubuntu")
    };
    let arch = Distro{
        Name: String::from("Arch Linux")
    };
    let mut distros= [ubuntu,arch];
    let encoded = json::encode(&distros).unwrap();
    let mut resp = Response::with((status::Ok, encoded));
    resp.headers.set(ContentType::json());
    Ok(resp)
}
/**
* Structs
*/
#[derive(RustcDecodable, RustcEncodable)]
struct Distro {
    Name: String
}