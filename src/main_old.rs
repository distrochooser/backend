extern crate iron;
extern crate router;
extern crate time;

#[macro_use]
extern crate hyper;
#[macro_use]
extern crate params;
#[macro_use]
extern crate mysql;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::str;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;

use router::Router;
use hyper::header::{ContentType};
use hyper::mime::{Mime};
use iron::status;
use iron::prelude::*;
use mysql::Pool;

use iron::method::Method;


static NAME:  &'static str = "Rusty Distrochooser";
static VERSION:  &'static str = "3.0.0";
static DATABASE: &'static str = "phisco_ldc4";
header! { (Server, "X-LDC") => [String] }
static mut LANG: i32 = 1;

mod structs;

fn main() {
    println!("{} {}",NAME, VERSION);
    let mut router = Router::new();
    router.get("/", index, "index"); 
    router.get("/distributions/:lang/", distributions,"distros"); 
    router.get("/distribution/:id/:lang/", distribution,"distro"); 
    router.get("/questions/:lang/", questions,"questions"); 
    router.get("/i18n/:lang/", i18n,"i18n"); 
    router.get("/newvisitor/", newvisitor,"newvisitor"); 
    router.post("/get/:lang/", get,"get"); 
    router.post("/addresult/",addresult,"addresult");
    router.get("/getstats/",getstats,"getstats");
    router.get("/getratings/:lang/", getratings,"getratings"); 
    router.post("/addrating/:lang/", addrating,"addrating"); 
    router.get("/test/:id/", gettest,"gettest"); 
    router.options("*",options,"options");
    Iron::new(router).http("127.0.0.1:8181").unwrap();
}
/**
* Helpers
*/
fn connect_database() -> Pool{
    if let Some(arg1) = env::args().nth(1) {
        let mut f = File::open(arg1).unwrap(); 
        let mut data = String::new();
        f.read_to_string(&mut data);
        let pool = Pool::new(data.as_str()).unwrap();
        let mut conn = pool.get_conn().unwrap();
        conn.prep_exec("SET NAMES UTF8;",()).unwrap();
        conn.prep_exec("SET sql_mode = '';",()).unwrap();
        return pool;
    }else{
        return Pool::new("").unwrap();
    }
}
fn middleware(request: &mut Request){
    let target: String = format!("{:?}",request.url.path()[0]).replace("\"","");
    let client = request.remote_addr.ip(); //TODO: Censor IP
    language(request);
}

fn handle_options(_request: &mut Request) -> bool{
    let method : String = _request.method.as_ref().to_string();
    return method == "OPTIONS"
}
fn options(_request: &mut Request) -> IronResult<Response> {    
    middleware(_request);
    Ok(get_response(String::from("Options :)")))
}

fn language(request: &mut Request){
    let ref lang:&str = request.extensions.get::<Router>().unwrap().find("lang").unwrap_or("/");
    unsafe{
         match lang.as_ref() {
            "de" => LANG = 1,
            _ => LANG = 2,
        }
    }
}

fn get_id(request: &mut Request) -> i32{
    return request.extensions.get::<Router>().unwrap().find("id").unwrap().parse::<i32>().unwrap();
}

fn get_distros(pool: &Pool) -> Vec<structs::Distro>{
    unsafe {
        let query: String = format!("Select d.Id as id ,d.Name as name,d.Homepage as homepage,d.Image as image, (
        Select dd.Description as description from {}.dictDistribution dd where  dd.DistributionId = d.Id and dd.LanguageId = {} limit 1
        ) as description,d.ImageSource as imagesource,d.TextSource as textsource,d.ColorCode as colorcode,d.Characteristica  as characteristica from  {}.Distribution d order by d.Name",DATABASE,LANG,DATABASE); 
        let mut distros: Vec<structs::Distro> = Vec::new();
        let mut conn = pool.get_conn().unwrap();
        let result = conn.prep_exec(query,()).unwrap();
        for row in result {
            let mut r = row.unwrap();
            let mut d = structs::Distro{
                    id:  r.take("id").unwrap(),
                    name:r.take("name").unwrap(),
                    description: r.take("description").unwrap(),
                    homepage: r.take("homepage").unwrap(),
                    image: r.take("image").unwrap(),
                    imagesource: r.take("imagesource").unwrap(),
                    textsource: r.take("textsource").unwrap(),
                    colorcode: r.take("colorcode").unwrap(),
                    tags:  Vec::new()
            };
            d.tags = d.get_tags(r.take("characteristica").unwrap());
            distros.push(d);
        }
        return distros;
    }
}

fn get_questions(pool: &Pool) -> Vec<structs::Question>{
    unsafe {
        let query: String = format!("Select q.Id as id,q.OrderIndex, dq.Text as text,q.Single as single, dq.Help as help,if(ExclusionTags is null,'[]',ExclusionTags) as exclusiontags from {}.Question q INNER JOIN {}.dictQuestion dq
			ON LanguageId = {} and QuestionId= q.Id order by q.OrderIndex",DATABASE,DATABASE,LANG); 
        let mut questions: Vec<structs::Question> = Vec::new();
        let mut conn = pool.get_conn().unwrap();
        let result = conn.prep_exec(query,()).unwrap();
        let mut i:i32 = 1;
        for row in result {
           let mut r = row.unwrap();
           let mut id: i32 = r.take("id").unwrap();
           let mut q = structs::Question{             
                buttontext: String::new(),
                help: r.take("help").unwrap(),
                id: id, 
                answers: get_answers(&pool,id),
                exclusiontags: Vec::new(),
                important: false,
                number: i,
                singleanswer: r.take("single").unwrap(),
                text: r.take("text").unwrap(),
                answered: false
           };
           q.exclusiontags =  q.get_exclusiontags(r.take("exclusiontags").unwrap());
           questions.push(q);
           i +=1;
        }
        return questions;
    }
}

fn get_answers(pool: &Pool,id: i32) -> Vec<structs::Answer>{
    unsafe {
        let query: String = format!("Select a.Id as id,(
							Select da.Text from {}.dictAnswer da where da.AnswerId = a.Id and da.LanguageId = {}
						)as text,a.Tags,a.NoTags,a.IsText as istext from {}.Answer a where a.QuestionId = {}",DATABASE,LANG,DATABASE,id); 
        let mut answers: Vec<structs::Answer> = Vec::new();
        let mut conn = pool.get_conn().unwrap();
        let result = conn.prep_exec(query,()).unwrap();
        for row in result {
           let mut r = row.unwrap();
           let tags: String = r.take("Tags").unwrap();
           let notags: String= r.take("NoTags").unwrap();
           let mut a = structs::Answer{              
                id: r.take("id").unwrap(),
                text: r.take("text").unwrap(),
                notags:serde_json::from_str(&notags).unwrap(),
                tags: serde_json::from_str(&tags).unwrap(),
                image: String::new(),
                istext: true,//r.take("istext").unwrap(),
                selected: false
           };
           answers.push(a);
        }
        return answers;
    }
}

fn get_i18n(pool: &Pool) -> HashMap<String,structs::i18nValue>{
    unsafe {
        let query: String = format!("Select Text,Val, Val as Name from {}.dictSystem where LanguageId =  {}",DATABASE,LANG); 
        let mut values = HashMap::new();
        let mut conn = pool.get_conn().unwrap();
        let result = conn.prep_exec(query,()).unwrap();
        for row in result {
           let mut r = row.unwrap();
           let text: String = r.take("Text").unwrap();
           let val: String = r.take("Val").unwrap();
           let name: String = r.take("Name").unwrap();
           values.insert(name,structs::i18nValue::new(text,val));
        }
        return values;
    }
}

fn get_distro(pool: &Pool, id: i32) -> structs::APIResult{
    let distros: Vec<structs::Distro> = get_distros(pool);
    for distro in distros{
        if distro.id == id{
            return Ok(distro)
        }
    }
    return Err(structs::APIError::DistroNotFound)
}
fn get_response(body: String) -> Response{
    let mut resp = Response::with((status::Ok, body.to_owned()));
    let mut server: String =  format!("{} {}",NAME,VERSION);
    resp.headers.set_raw("content-type", vec![b"application/json;charset=utf-8".to_vec()]);
    resp.headers.set_raw("server", vec![server.into_bytes()]);
    resp.headers.set_raw("Access-Control-Allow-Origin",vec![b"*".to_vec()]);
    resp.headers.set_raw("Access-Control-Allow-Method",vec![b"GET, OPTIONS, POST".to_vec()]);
    resp.headers.set_raw("Access-Control-Allow-Headers",vec![b"Content-Type".to_vec()]);
    return resp;
}


fn new_visitor(pool: &Pool,request: &mut Request) -> i32{
    let mut useragent: String = String::new();
    let mut referer: String = String::new();
    match  request.headers.get::<iron::headers::UserAgent>() {
        Some(x) => useragent = format!("{}",x),
        None    => useragent = String::new(),
    }
    match  request.headers.get::<iron::headers::Referer>() {
        Some(x) => referer = format!("{}",x),
        None    => referer = String::new(),
    }

    let params = request.get_ref::<params::Params>().unwrap();
    use params::{Params, Value};
    let mut adblocker: i32 = 0;
    let mut dnt: i32 = 0;
    match params.find(&["adblocker"]) {
        Some(&Value::String(ref name)) if name != "" => {
            if name == "true"{
                adblocker = 1;
            }else{
                adblocker = 0;
            }
        },
        _ => adblocker = 0
    }

    match params.find(&["dnt"]) {
        Some(&Value::String(ref name)) if name != "" => {
            if name == "true"{
                dnt = 1;
            }else{
                dnt = 0;
            }
        },
        _ => dnt = 0
    }

    let tm = time::now();
    let time = format!("{}",tm.strftime("%Y-%m-%d %H:%M:%S").unwrap());
    let query: String = format!("Insert into {}.Visitor (Date, Referrer, Useragent, DNT,Adblocker, API) VALUES (:time,:ref,:ua,:dnt,:adblocker,'waldorf')",DATABASE);
    pool.prep_exec(query,(time,referer ,useragent,dnt,adblocker)).unwrap();

    //return visitor id
    let max_id: String = format!("Select max(Id) as id from {}.Visitor",DATABASE);
    let mut id: i32 = 0;
    let mut conn = pool.get_conn().unwrap();
    let result = conn.prep_exec(max_id,()).unwrap();
    for row in result {
        let mut r = row.unwrap();
        id = r.take("id").unwrap();
    }
    return id;
}
/**
* Routes
*/
fn index(_request: &mut Request) -> IronResult<Response> {    
    middleware(_request);
    Ok(get_response(String::from("I'm an rusty API.")))
}
fn get(_request: &mut Request) -> IronResult<Response>{
    middleware(_request); 
    let mut p: Pool = connect_database();
    let result: structs::Get = structs::Get{
        questions: get_questions(&p),
        distros: get_distros(&p),
        i18n: get_i18n(&p),
        visitor: new_visitor(&p,_request)
    };
    let response: String = serde_json::to_string(&result).unwrap();    
    Ok(get_response(response))
}
fn newvisitor(_request: &mut Request) -> IronResult<Response> {    
    middleware(_request); 
    let id: i32 = new_visitor(&connect_database(),_request);
    let body: String = format!("{}",id);
    Ok(get_response(body))
}

fn getstats(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    let max_id: String = format!("SELECT 
        COUNT( Id ) as count ,
        DATE_FORMAT(DATE, '%d/%m') AS MONTH,
        DATE_FORMAT(DATE, '%d/%m/%Y') AS FullDate,
        (
        Select count(Id) from {}.Visitor where DATE_FORMAT(DATE, '%d/%m/%Y')  = FullDate
        ) as hits
        FROM {}.Result
        WHERE YEAR( DATE ) = YEAR( CURDATE( ) )
        and MONTH(DATE) = MONTH(CURDATE())
        GROUP BY FullDate",DATABASE,DATABASE);
    let mut p: Pool = connect_database();
    let mut conn = p.get_conn().unwrap();
    let result = conn.prep_exec(max_id,()).unwrap();
    let mut stats: Vec<structs::Stat> = Vec::new();
    for row in result {
        let mut r = row.unwrap();
        let mut s = structs::Stat{              
            MONTH: r.take("MONTH").unwrap(),
            hits: r.take("hits").unwrap(),
            count: r.take("count").unwrap(),
            FullDate: r.take("FullDate").unwrap()
        };
        stats.push(s);        
    }
    stats.reverse();
    let response: String = serde_json::to_string(&stats).unwrap();
    Ok(get_response(response))
}

fn getratings(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    unsafe {
        let query: String = format!("Select * from {}.Rating where Approved = 1 and Lang = {} and Test is not null order by ID desc limit 7",DATABASE,LANG); 
        let mut ratings: Vec<structs::Rating> = Vec::new();
        let pool: Pool = connect_database();
        let mut conn = pool.get_conn().unwrap();
        let result = conn.prep_exec(query,()).unwrap();
        for row in result {
            let mut r = row.unwrap();
            let comment = structs::Rating{
                    ID:  r.take("ID").unwrap(),
                    Rating: r.take("Rating").unwrap(),
                    UserAgent: r.take("UserAgent").unwrap(),
                    Comment: r.take("Comment").unwrap(),
                    Test: r.take("Test").unwrap()
            };
            ratings.push(comment);
        }
        let response: String = serde_json::to_string(&ratings).unwrap();
        Ok(get_response(response))
    }
}

fn gettest(_request: &mut Request) -> IronResult<Response>{
    middleware(_request);
    unsafe {
        let ref id:i32 = get_id(_request);
        let query: String = format!("Select Answers as answers, Important as important from {}.Result where Id = {}",DATABASE,id); 
        let mut ratings: Vec<structs::Rating> = Vec::new();
        let pool: Pool = connect_database();
        let mut conn = pool.get_conn().unwrap();
        let result = conn.prep_exec(query,()).unwrap();
        let mut test= structs::Test{
                    answers:  Vec::new(),
                    important: Vec::new()
        };
        for row in result {
            let mut r = row.unwrap();
            test.answers = test.get_tags(r.take("answers").unwrap());
            test.important = test.get_tags(r.take("important").unwrap());
        }
        let response: String = serde_json::to_string(&test).unwrap();
        Ok(get_response(response))
    }
}
fn addrating(_request: &mut Request) -> IronResult<Response>{
    middleware(_request);
    unsafe {
        let mut useragent: String = String::new();
        match  _request.headers.get::<iron::headers::UserAgent>() {
            Some(x) => useragent = format!("{}",x),
            None    => useragent = String::new(),
        }
        let params = _request.get_ref::<params::Params>().unwrap();
        
        let rating: i32 = String::from(format!("{:?}",params["rating"]).replace('"',"").replace("\\","")).parse().unwrap_or(0);       
 
        use params::{Params, Value};
        let mut comment: String = String::new();
        match params.find(&["comment"]) {
            Some(&Value::String(ref val)) => {
                comment = val.to_owned()
            },
            _ => (),
        }
        let test: i32 = String::from(format!("{:?}",params["test"]).replace('"',"").replace("\\","")).parse().unwrap_or(0);        

        let mut email: String = String::new();
        match params.find(&["email"]) {
            Some(&Value::String(ref val)) => {
                email = val.to_owned()
            },
            _ => (),
        }

        let query: String = format!("Insert into {}.Rating (Rating,Date,UserAgent,Comment,Test,Lang,Email) Values (?,CURRENT_TIMESTAMP,?,?,?,?,?)",DATABASE);
        let p: Pool = connect_database();
        p.prep_exec(query,(rating,useragent ,comment,test,LANG,email)).unwrap();
        Ok(get_response(format!("{}",rating)))
    }
}

fn addresult(_request: &mut Request) -> IronResult<Response> {    
    middleware(_request);

    let mut useragent: String = String::new();
    match  _request.headers.get::<iron::headers::UserAgent>() {
        Some(x) => useragent = format!("{}",x),
        None    => useragent = String::new(),
    }

    let params = _request.get_ref::<params::Params>().unwrap();

    use params::{Params, Value};
    let mut distro_json: String = String::new();
    match params.find(&["distros"]) {
        Some(&Value::String(ref val)) => {
            distro_json = val.to_owned()
        },
        _ => (),
    }
    let mut tags_json: String = String::new();
    match params.find(&["tags"]) {
        Some(&Value::String(ref val)) => {
            tags_json = val.to_owned()
        },
        _ => (),
    }
    let mut answers_json: String = String::new();
    match params.find(&["answers"]) {
        Some(&Value::String(ref val)) => {
            answers_json = val.to_owned()
        },
        _ => (),
    }
    let mut important_json: String = String::new();
    match params.find(&["important"]) {
        Some(&Value::String(ref val)) => {
            important_json = val.to_owned()
        },
        _ => (),
    }

    let p: Pool = connect_database();
    let query: String = format!("Insert into {}.Result (Date,UserAgent,Tags, Answers,Important) Values(CURRENT_TIMESTAMP,:ua,:tags,:answers,:important)",DATABASE);
    p.prep_exec(query,(useragent,tags_json,answers_json,important_json)).unwrap();

    //return result id
    let max_id: String = format!("Select max(Id) as id from {}.Result",DATABASE);
    let mut id: i32 = 0;
    let mut conn = p.get_conn().unwrap();
    let result = conn.prep_exec(max_id,()).unwrap();
    for row in result {
        let mut r = row.unwrap();
        id = r.take("id").unwrap();
    }

    let distros: Vec<i32> = serde_json::from_str(&distro_json).unwrap();

    for distro in distros{
        let add_result: String = format!("Insert into {}.ResultDistro (DistroId,ResultId) Values(:distro,:result)",DATABASE);
        p.prep_exec(add_result,(distro,id)).unwrap();
    }

    Ok(get_response(format!("{:?}",id)))
}

fn distributions(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    let distros: Vec<structs::Distro> = get_distros(&connect_database());
    
    let response: String = serde_json::to_string(&distros).unwrap();
    Ok(get_response(response))
}
fn distribution(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    let id: i32 = get_id(_request);
    let raw = get_distro(&connect_database(),id);
    let mut distro: Option<structs::Distro> = None;
    match raw{
        Ok(n) => distro = Some(n),
        Err(_) => distro = None
    };
    let resp;
    if distro.is_none(){
        resp = Response::with((status::NotFound,"Not found"));
    }else{
        resp = get_response(serde_json::to_string(&distro).unwrap());
        
    }
    return Ok(resp);
}
fn questions(_request: &mut Request) -> IronResult<Response>{
    middleware(_request);
    let questions: Vec<structs::Question> = get_questions(&connect_database());
    let response: String = serde_json::to_string(&questions).unwrap();
    Ok(get_response(response))
}
fn i18n(_request: &mut Request) -> IronResult<Response>{
    middleware(_request);
    let translation: HashMap<String,structs::i18nValue> = get_i18n(&connect_database());
    let response: String = serde_json::to_string(&translation).unwrap();
    Ok(get_response(response))
}

