extern crate iron;
extern crate router;
extern crate rustc_serialize;
extern crate time;

#[macro_use]
extern crate hyper;
#[macro_use]
extern crate params;
#[macro_use]
extern crate mysql;

use std::str;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;

use router::Router;
use hyper::header::{ContentType,HeaderFormat};
use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};
use iron::status;
use iron::prelude::*;
use mysql::Pool;
use rustc_serialize::json;

static NAME:  &'static str = "Rusty Distrochooser";
static VERSION:  &'static str = "3.0.0";
header! { (Server, "X-LDC") => [String] }
static mut LANG: i32 = 1;

mod structs;

fn main() {
    println!("Starting {} {}...",NAME, VERSION);
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
        Select dd.Description as description from phisco_ldc3.dictDistribution dd where  dd.DistributionId = d.Id and dd.LanguageId = {} limit 1
        ) as description,d.ImageSource as imagesource,d.TextSource as textsource,d.ColorCode as colorcode,d.Characteristica  as characteristica from  phisco_ldc3.Distribution d",LANG); 
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
        let query: String = format!("Select q.Id as id,q.OrderIndex, dq.Text as text,q.Single as single, dq.Help as help,if(ExclusionTags is null,'[]',ExclusionTags) as exclusiontags from phisco_ldc3.Question q INNER JOIN phisco_ldc3.dictQuestion dq
			ON LanguageId = {} and QuestionId= q.Id order by q.OrderIndex",LANG); 
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
                text: r.take("text").unwrap()
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
							Select da.Text from phisco_ldc3.dictAnswer da where da.AnswerId = a.Id and da.LanguageId = {}
						)as text,a.Tags,a.NoTags,a.IsText as istext from phisco_ldc3.Answer a where a.QuestionId = {}",LANG,id); 
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
                notags: json::decode(&notags).unwrap(),
                tags: json::decode(&tags).unwrap(),
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
        let query: String = format!("Select Text,Val, Val as Name from phisco_ldc3.dictSystem where LanguageId =  {}",LANG); 
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
    resp.headers.set(ContentType(Mime(TopLevel::Application, SubLevel::Json,
                     vec![(Attr::Charset, Value::Utf8)])));
    resp.headers.set(Server(format!("{} {}",NAME,VERSION).to_owned()));
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
    let query: String = String::from("Insert into phisco_ldc3.Visitor (Date, Referrer, Useragent, DNT,Adblocker, API) VALUES (:time,:ref,:ua,:dnt,:adblocker,'waldorf')");
    pool.prep_exec(query,(time,referer ,useragent,dnt,adblocker)).unwrap();

    //return visitor id
    let max_id: String = String::from("Select max(Id) as id from phisco_ldc3.Visitor");
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
    Ok(get_response(String::from(json::encode(&result).unwrap())))
}
fn newvisitor(_request: &mut Request) -> IronResult<Response> {    
    middleware(_request); 
    let id: i32 = new_visitor(&connect_database(),_request);
    let body: String = format!("{}",id);
    Ok(get_response(body))
}

fn getstats(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    let max_id: String = String::from("SELECT 
    COUNT( Id ) as results ,
    DATE_FORMAT(DATE, '%d/%m/%Y') AS MONTH,
    (
    Select count(Id) from phisco_ldc3.Visitor where DATE_FORMAT(DATE, '%d/%m/%Y')  = MONTH
    ) as visitors
    FROM phisco_ldc3.Result
    WHERE YEAR( DATE ) = YEAR( CURDATE( ) )
    and MONTH(DATE) = MONTH(CURDATE())
    GROUP BY MONTH");
    let mut p: Pool = connect_database();
    let mut conn = p.get_conn().unwrap();
    let result = conn.prep_exec(max_id,()).unwrap();
    let mut stats: Vec<structs::Stat> = Vec::new();
    for row in result {
        let mut r = row.unwrap();
        let mut s = structs::Stat{              
            MONTH: r.take("MONTH").unwrap(),
            count: r.take("visitors").unwrap(),
            tests: r.take("results").unwrap(),
        };
        stats.push(s);        
    }
    Ok(get_response(String::from(json::encode(&stats).unwrap())))
}

fn getratings(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    unsafe {
        let query: String = format!("Select * from phisco_ldc3.Rating where Approved = 1 and Lang = {} and Test is not null order by ID desc limit 7",LANG); 
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
        Ok(get_response(String::from(json::encode(&ratings).unwrap())))
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
        let comment: String =  String::from(format!("{:?}",params["comment"]).replace('"',"").replace("\\",""));
        let test: i32 = String::from(format!("{:?}",params["test"]).replace('"',"").replace("\\","")).parse().unwrap_or(0);

        let query: String = String::from("Insert into phisco_ldc3.Rating (Rating,Date,UserAgent,Comment,Test,Lang) Values (?,CURRENT_TIMESTAMP,?,?,?,?)");
        let p: Pool = connect_database();
        p.prep_exec(query,(rating,useragent ,comment,test,LANG)).unwrap();
        Ok(get_response(format!("{}",rating)))
    }
}

fn addresult(_request: &mut Request) -> IronResult<Response> {    
    middleware(_request);

   // let tags = rustc_serialize::json::Json::from_str(&tags_json).unwrap();
   // let tagsObj = tags.as_object().unwrap();
    /*for (key, value) in obj.iter() {
        println!("{}: {}", key, value);
    }
    */
   // let answers: Vec<String> = json::decode(&answers_json).unwrap();
    //let important: Vec<String> = json::decode(&important_json).unwrap();

    let mut useragent: String = String::new();
    match  _request.headers.get::<iron::headers::UserAgent>() {
        Some(x) => useragent = format!("{}",x),
        None    => useragent = String::new(),
    }

    let params = _request.get_ref::<params::Params>().unwrap();
    let mut distro_json: String = format!("{:?}",params["distros"]);
    distro_json = String::from(distro_json.trim_matches('"').replace("\\",""));

    let mut tags_json: String = format!("{:?}",params["tags"]);
    tags_json = String::from(tags_json.trim_matches('"').replace("\\",""));

    let mut answers_json: String = format!("{:?}",params["answers"]);
    answers_json = String::from(answers_json.trim_matches('"').replace("\\",""));
    
    let mut important_json: String = format!("{:?}",params["important"]); 
    important_json = String::from(important_json.trim_matches('"').replace("\\",""));

    let p: Pool = connect_database();
    let query: String = String::from("Insert into phisco_ldc3.Result (Date,UserAgent,Tags, Answers,Important) Values(CURRENT_TIMESTAMP,:ua,:tags,:answers,:important)");
    p.prep_exec(query,(useragent,tags_json,answers_json,important_json)).unwrap();

    //return result id
    let max_id: String = String::from("Select max(Id) as id from phisco_ldc3.Result");
    let mut id: i32 = 0;
    let mut conn = p.get_conn().unwrap();
    let result = conn.prep_exec(max_id,()).unwrap();
    for row in result {
        let mut r = row.unwrap();
        id = r.take("id").unwrap();
    }

    let distros: Vec<structs::Distro> = json::decode(&distro_json).unwrap();

    for distro in distros{
        let add_result: String = String::from("Insert into phisco_ldc3.ResultDistro (DistroId,ResultId) Values(:distro,:result)");
        p.prep_exec(add_result,(distro.id,id)).unwrap();
    }

    Ok(get_response(format!("{:?}",id)))
}

fn distributions(_request: &mut Request) -> IronResult<Response> {
    middleware(_request);
    let distros: Vec<structs::Distro> = get_distros(&connect_database());
    Ok(get_response(String::from(json::encode(&distros).unwrap())))
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
        resp = get_response(String::from(json::encode(&distro).unwrap()));
    }
    return Ok(resp);
}
fn questions(_request: &mut Request) -> IronResult<Response>{
    middleware(_request);
    let questions: Vec<structs::Question> = get_questions(&connect_database());
    Ok(get_response(String::from(json::encode(&questions).unwrap())))
}
fn i18n(_request: &mut Request) -> IronResult<Response>{
    middleware(_request);
    let translation: HashMap<String,structs::i18nValue> = get_i18n(&connect_database());
    Ok(get_response(String::from(json::encode(&translation).unwrap())))
}

