extern crate iron;
extern crate router;
#[macro_use]
extern crate hyper;
extern crate params;
extern crate rustc_serialize;

#[macro_use]
extern crate mysql;

use iron::prelude::*;
use router::Router;
use hyper::header::{ContentType};
use iron::status;
use rustc_serialize::json;
use mysql::Pool;
use std::str;

static NAME:  &'static str = "Rusty Distrochooser";
static VERSION:  &'static str = "3.0.0";
header! { (Server, "Server") => [String] }
static DEBUG: bool = true;
static mut LANG: i32 = 1;
fn main() {
    println!("Starting {} {}...",NAME, VERSION);
    let mut router = Router::new();
    router.get("/", index, "index"); 
    router.get("/distributions/:lang/", distributions,"distros"); 
    router.get("/distribution/:id/:lang/", distribution,"distro"); 
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
    if DEBUG{
        println!("Serving.. /{} for {:?}",target,client);
    }
    language(request);
}
fn language(request: &mut Request){
    let ref lang:&str = request.extensions.get::<Router>().unwrap().find("lang").unwrap_or("/");
    unsafe{
         match lang.as_ref() {
            "de" => LANG = 1,
            _ => LANG = 2,
        }
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
    unsafe {
        let query: String = format!("Select d.Id as id ,d.Name as name,d.Homepage as homepage,d.Image as image, (
        Select dd.Description as description from phisco_ldc2.dictDistribution dd where  dd.DistributionId = d.Id and dd.LanguageId = {} limit 1
        ) as description,d.ImageSource as imagesource,d.TextSource as textsource,d.ColorCode as colorcode,d.Characteristica  as characteristica from  phisco_ldc2.Distribution d",LANG); 
       let mut distros: Vec<Distro> = Vec::new();
       pool.prep_exec(query,()).map(|mut result| {
            let row = result.next().unwrap().unwrap();
            let (id, name, homepage, image, description, imagesource, textsource, colorcode,characteristica) = mysql::from_row(row);
            let mut d = Distro{
                    id: id,
                    name:name,
                    description: description,
                    homepage: homepage,
                    image: image,
                    imagesource: imagesource,
                    textsource: textsource,
                    colorcode: colorcode,
                    characteristica: characteristica,
                    tags:  Vec::new()
            };
            d.tags = d.get_tags();
            distros.push(d);
       }).unwrap();
       return distros;
    }
}


fn get_distro(pool: Pool, id: i32) -> APIResult{
    let distros: Vec<Distro> = get_distros(pool);
    for distro in distros{
        if distro.id == id{
            return Ok(distro)
        }
    }
    return Err(APIError::DistroNotFound)
}
/**
* Routes
*/
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
fn distribution(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    let id: i32 = _request.extensions.get::<Router>().unwrap().find("id").unwrap().parse::<i32>().unwrap();
    let raw = get_distro(connect_database(),id);
    let mut distro: Option<Distro> = None;
    match raw{
        Ok(n) => distro = Some(n),
        Err(_) => distro = None
    };
    let mut resp;
    if distro.is_none(){
        let encoded = json::encode(&distro).unwrap();
        if DEBUG{
            println!("Distro {:?} not found!",id);
        }
        resp = Response::with((status::NotFound,"Not found"));
    }else{
        let encoded = json::encode(&distro).unwrap();
        resp = Response::with((status::Ok, encoded));
        resp.headers.set(ContentType::json());
        resp.headers.set(Server(format!("{} {}",NAME,VERSION).to_owned()));
    }
    return Ok(resp);
}
/**
* Structs
*/
#[derive(RustcDecodable, RustcEncodable)]
pub struct Distro {
    id: i32,
    name: String,
    description: String,
    homepage: String,
    image: String,
    imagesource: String,
    textsource: String,
    colorcode: String,
    characteristica: String,
    tags: Vec<String>
}

impl Distro{
    fn get_tags(&self) -> Vec<String> {
        let v: Vec<String> = json::decode(&self.characteristica).unwrap();
        return v;
    }
}

/**
* Enums
*/
#[derive(RustcDecodable, RustcEncodable)]
pub enum APIError {
    DistroNotFound
}
/**
* Types
*/
pub type APIResult = Result<Distro, APIError>;