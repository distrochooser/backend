extern crate iron;
extern crate router;
extern crate hyper;
extern crate params;
extern crate rustc_serialize;

#[macro_use]
extern crate mysql;

use iron::prelude::*;
use router::Router;
use hyper::header::{ContentType};
use iron::status;
use iron::TypeMap;
use rustc_serialize::json;
use params::{Params, Value};
use mysql::Pool;

static NAME:  &'static str = "Rusty Distrochooser";
static VERSION:  &'static str = "3.0.0";
static DEBUG: bool = true;
static mut LANG: i32 = 1;
fn main() {
    println!("Starting {} {}...",NAME, VERSION);
    let mut router = Router::new();
    router.get("/", index, "index"); 
    router.get("/distributions/:lang/", distributions,"distros"); 
    Iron::new(router).http("localhost:3000").unwrap();
}
/**
* Helpers
*/
fn connect_database() -> Pool{
   let pool = Pool::new("mysql://root:foobarbarz@localhost").unwrap();
   return pool;
}
fn middleware(request: &mut Request){
    let target: String = format!("{:?}",request.url.path()[0]).replace("\"","");
    let client = request.remote_addr.ip(); //TODO: Censor IP
    if (DEBUG){
        println!("Serving.. /{} for {:?}",target,client);
    }
    language(request);
}
fn language(request: &mut Request){
    let ref lang:&str = request.extensions.get::<Router>().unwrap().find("lang").unwrap_or("/");
    let mut langCode: i32 = 1;
    match lang.as_ref() {
        "de" => langCode = 1,
        _ => langCode = 2,
    }
    unsafe{
        LANG = langCode;
        println!("Choosing language key {:?} for this request.",LANG);
    }
    /*
    let get =  request.get_ref::<Params>();
    for item in get {
        println!("{:?}",item); //get
    }
    */
}
fn get_distros(pool: Pool) -> Vec<Distro>{
    let distros: Vec<Distro> =
   // pool.prep_exec("SELECT Id as id,Name as name  from phisco_ldc2.Distribution where Id = :id", params!{"id" => 2})
   pool.prep_exec("SELECT Id as id,Name as name  from phisco_ldc2.Distribution",())
    .map(|result| { 
        result.map(|x| x.unwrap()).map(|row| {
            let (id, name) = mysql::from_row(row);
            Distro {
                id: id,
                name:name
            }
        }).collect()
    }).unwrap();
    return distros;
}
/**
* Routes
*/
//"_" suppresses unused warning
fn index(_request: &mut Request) -> IronResult<Response> {    
    middleware(_request);
    Ok(Response::with((status::Ok, String::from("This is my old 'n rusty API."))))
}
fn distributions(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    let distros: Vec<Distro> = get_distros(connect_database());
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
    id: i32,
    name: String
}