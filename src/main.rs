#![allow(non_snake_case)]
#![allow(unused_must_use)]
#![allow(unused_parens)]

extern crate router;
extern crate iron;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate mysql;

use router::Router;
use iron::Iron;
use iron::status;
use iron::response::Response;
use iron::Request;
use iron::IronResult;
use mysql::Pool;
use std::fs::File;
use std::io::prelude::*;
use std::env;
use std::ptr;

fn main(){
    let mut router = Router::new();
    router.get("/", get_index, "index_route"); 
    router.options("*",options,"catchall_options_route");
    router.get("/distributions/:lang/", get_distributions, "get_all_distros"); 
    router.get("/distributions/:lang/:id/", get_distribution, "get_one_distros"); 
    router.get("/questions/:lang/", get_questions, "get_all_questions"); 

    Iron::new(router).http("127.0.0.1:8181").unwrap();
}
/**
* Routes
*/
fn get_index(_request: &mut Request) -> IronResult<Response> {    
    Ok(get_response(String::from("I'm an rusty API.")))
}

/**
* /distributions/ and /distribution/:id/
*/
#[derive(Serialize, Deserialize)]
pub struct Distro{
    pub id: i32,
    pub name: String,
    pub website: String,
    pub textSource: String,
    pub imageSource: String,
    pub image: String,
    pub tags: Vec<String>,
    pub description: String
}
/**
* Query database structs
*/
fn query_distributions(pool: &Pool, lang: &String) -> Vec<Distro>{
    let query: String = format!("Select * from Distro"); 
    let mut distros: Vec<Distro> = Vec::new();
    let mut conn = pool.get_conn().unwrap();
    let result = conn.prep_exec(query,()).unwrap();
    for row in result {
        let mut r = row.unwrap();
        let mut d = Distro{
                id:  r.take("id").unwrap(),
                name: r.take("name").unwrap(),
                textSource: r.take("textSource").unwrap(),
                imageSource: r.take("imageSource").unwrap(),
                image: r.take("image").unwrap(),
                website: r.take("website").unwrap(),
                tags: Vec::new(),
                description: String::new()
        };
        let tags: String = r.take("tags").unwrap();
        d.tags = serde_json::from_str(&tags).unwrap();
        d.description = get_i18n(&pool,format!("d.{:?}.description",d.id),lang);
        distros.push(d);
    }
    return distros;
}
/**
* Get all distributions
*/
fn get_distributions(_request: &mut Request) -> IronResult<Response> {  
    let pool: Pool = connect_database();
    let lang = get_lang(&pool,_request);
    let distros: Vec<Distro> = query_distributions(&pool,&lang);
    let response: String = serde_json::to_string_pretty(&distros).unwrap();
    Ok(get_response(response))
}
/**
* Get single distribution
*/
fn get_distribution(_request: &mut Request) -> IronResult<Response> {  
    let pool: Pool = connect_database();
    let lang = get_lang(&pool,_request);
    let mut id: i32 = String::from(_request.extensions.get::<Router>().unwrap().find("id").unwrap()).parse::<i32>().unwrap();
    
    let distros: Vec<Distro> = query_distributions(&pool,&lang);
    let mut response: Response = get_not_found_response();
    for distro in distros {
        if (distro.id == id) {
            let body: String = serde_json::to_string_pretty(&distro).unwrap();
            response = get_response(body);
        }
    }
    Ok(response)
}
/**
* /questions/:lang/
*/
#[derive(Serialize, Deserialize)]
pub struct Question{
    pub id: i32,
    pub orderIndex: i32,
    pub text: String,
    pub title: String,
    pub isText: bool,
    pub isSingle: bool,
    pub excludedBy: Vec<String>,
    pub answers: Vec<Answer>
}
#[derive(Serialize, Deserialize)]
pub struct Answer{
    pub id: i32,
    pub text: String,
    pub tags: Vec<String>,
    pub excludeTags: Vec<String>
}

/**
* Query database structs
*/
fn query_questions(pool: &Pool, lang: &String) -> Vec<Question>{
    let query: String = format!("Select * from Question"); 
    let mut questions: Vec<Question> = Vec::new();
    let mut conn = pool.get_conn().unwrap();
    let result = conn.prep_exec(query,()).unwrap();
    for row in result {
        let mut r = row.unwrap();
        let mut q = Question{
                id:  r.take("id").unwrap(),
                orderIndex:  r.take("orderIndex").unwrap(),
                isSingle : r.take("isSingle").unwrap(),
                isText : r.take("isText").unwrap(),
                excludedBy: Vec::new(),
                answers: Vec::new(),
                title: String::new(),
                text: String::new()
        };
        // get derived properties
        q.text = get_i18n(&pool,format!("q.{:?}.text",q.id),lang);
        q.title = get_i18n(&pool,format!("q.{:?}.title",q.id),lang);
        let tags: String = r.take("excludedBy").unwrap();
        q.excludedBy = serde_json::from_str(&tags).unwrap();
        q.answers = query_answers(&pool,lang,q.id);
        questions.push(q);
    }
    return questions;
}
fn query_answers(pool: &Pool, lang: &String, question: i32) -> Vec<Answer>{
    let query: String = format!("Select * from Answer where questionId = :id"); 
    let mut answers: Vec<Answer> = Vec::new();
    let mut conn = pool.get_conn().unwrap();
    let result = conn.prep_exec(query,params!{
            "id" => question
    }).unwrap();
    for row in result {
        let mut r = row.unwrap();
        let mut a = Answer{
                id:  r.take("id").unwrap(),
                text: String::new(),
                tags: Vec::new(),
                excludeTags: Vec::new()
        };
        a.text = get_i18n(&pool,format!("a.{:?}.text",a.id),lang);
        let  mut tags: String = r.take("tags").unwrap();
        a.tags = serde_json::from_str(&tags).unwrap();
        tags = r.take("excludeTags").unwrap();
        a.excludeTags = serde_json::from_str(&tags).unwrap();
        answers.push(a);
    }
    return answers;
}
fn get_questions(_request: &mut Request) -> IronResult<Response> {
    let pool: Pool = connect_database();
    let lang = get_lang(&pool,_request);
    let questions: Vec<Question> = query_questions(&pool,&lang);
    let response: String = serde_json::to_string_pretty(&questions).unwrap();
    Ok(get_response(response))
}

/**
* Helper functions
*/
fn connect_database() -> Pool{
    if let Some(arg1) = env::args().nth(1) {
        let mut f = File::open(arg1).unwrap(); 
        let mut data = String::new();
        f.read_to_string(&mut data);
        print!("{:?}",data);
        let pool = Pool::new(data.as_str()).unwrap();
        let mut conn = pool.get_conn().unwrap();
        conn.prep_exec("SET NAMES UTF8;",()).unwrap();
        conn.prep_exec("SET sql_mode = '';",()).unwrap();
        return pool;
    }else{
        return Pool::new("").unwrap();
    }
}

fn is_lang_present(pool: &Pool, lang: String) -> bool{
    let mut conn = pool.get_conn().unwrap();
    let result = conn.prep_exec("Select langCode from i18n group by langCode",()).unwrap();
    for row in result {
        let mut r = row.unwrap();
        let code: String = r.take("langCode").unwrap();
        if code == lang {
            return true;
        }
    }
    return false;
}

fn get_lang(pool: &Pool, _request: &mut Request) -> String{
    let mut lang: String = String::from(_request.extensions.get::<Router>().unwrap().find("lang").unwrap_or("de"));
    if (!is_lang_present(&pool,lang.to_owned())) {
        lang = String::from("en");
    }
    return lang;
}

fn get_i18n(pool: &Pool, val: String, lang: &String) -> String{
    let mut conn = pool.get_conn().unwrap();
    let result = conn.prep_exec("Select * from i18n where langCode = :code and val = :value limit 1",
        params!{
            "code" => lang.to_owned(),
            "value" => val.to_owned()
        }
    ).unwrap();
    for row in result {
        let mut r = row.unwrap();
        let translation: String = r.take("translation").unwrap();
        return translation;
    }
    return val.to_owned();
}

fn get_response(body: String) -> Response{
    let mut resp = Response::with((status::Ok, body.to_owned()));
    set_cors(&mut resp);
    return resp;
}
fn get_not_found_response() -> Response{
    let mut resp = Response::with((status::NotFound));
    set_cors(&mut resp);
    return resp;
}
fn options(_request: &mut Request) -> IronResult<Response> {    
    Ok(get_response(String::from("Options :)")))
}
fn set_cors(resp: &mut Response) {   
    let server: String =  String::from("Distrochooser 4");
    resp.headers.set_raw("content-type", vec![b"application/json;charset=utf-8".to_vec()]);
    resp.headers.set_raw("server", vec![server.into_bytes()]);
    resp.headers.set_raw("Access-Control-Allow-Origin",vec![b"*".to_vec()]);
    resp.headers.set_raw("Access-Control-Allow-Method",vec![b"GET, OPTIONS, POST".to_vec()]);
    resp.headers.set_raw("Access-Control-Allow-Headers",vec![b"Content-Type".to_vec()]);
}