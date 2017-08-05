#![allow(non_snake_case)]
#![allow(unused_must_use)]
#![allow(unused_parens)]

extern crate router;
extern crate iron;
extern crate params;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate mysql;
extern crate time;
extern crate bodyparser;

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
use std::str::FromStr;
use iron::Plugin;
use std::collections::HashMap;



fn main(){
    let mut router = Router::new();
    router.get("/", get_index, "index_route"); 
    router.options("*",options,"catchall_options_route");
    router.get("/distributions/:lang/", get_distributions, "get_all_distros"); 
    router.get("/distributions/:lang/:id/", get_distribution, "get_one_distros"); 
    router.get("/questions/:lang/", get_questions, "get_all_questions"); 
    router.get("/get/:lang/:adblocker/:dnt/",new_visitor, "get_new_visitor");
    router.post("/addresult/:lang/:rating/:visitor/",add_result,"add_new_result");
    router.get("/getresult/:id/",get_result, "get_old_result");
    Iron::new(router).http("127.0.0.1:8181").unwrap();
}
/**
* Routes
*/
fn get_index(_request: &mut Request) -> IronResult<Response> {    
    Ok(get_response(String::from("I'm an rusty API.")))
}

#[derive(Debug, Clone, Deserialize,Serialize)]
struct Result {
    pub answers: Vec<i32>,
    pub tags: Vec<Tag>
}
#[derive(Debug, Clone, Deserialize,Serialize)]
struct Tag {
    pub name: String,
    pub weight: i32,
    pub amount: i32,
    pub negative: bool
}

fn add_result(_request: &mut Request) -> IronResult<Response> {  
    let pool: Pool = connect_database();
    //get meta info
    let lang: String = get_lang(&pool,_request);
    let ratingRaw: String = String::from(_request.extensions.get::<Router>().unwrap().find("rating").unwrap_or("0"));
    let visitorRaw: String = String::from(_request.extensions.get::<Router>().unwrap().find("visitor").unwrap_or("0"));
   
    //let data =   _request.get_ref::<params::Params>().unwrap();
    let rawResult = _request.get_ref::<bodyparser::Struct<Result>>();
    let result: Result = rawResult.unwrap().to_owned().unwrap();
    //create rating
    let query: String = format!("Insert into Result (rating, visitorId, lang) VALUES (:r,:v, :l)"); 
    let mut conn = pool.get_conn().unwrap();
    conn.prep_exec(query,params!{
            "r" => ratingRaw.to_owned(),
            "v" => visitorRaw.to_owned(),
            "l" => lang
    }).unwrap();

    let max_id: String = format!("Select max(id) as id from Result");
    let mut resultId: i32 = 0;
    let mut conn = pool.get_conn().unwrap();
    let maxIdResult = conn.prep_exec(max_id,()).unwrap();
    for row in maxIdResult {
        let mut r = row.unwrap();
        resultId = r.take("id").unwrap();
    }
    //insert answers
    let answers: Vec<i32> = result.answers;
    for answer in answers.to_owned() {
        let query: String = format!("Insert into ResultAnswers (resultId, answer) VALUES (:r,:a)");
        let mut conn = pool.get_conn().unwrap();
        conn.prep_exec(query,params!{
                "r" => resultId.to_owned(),
                "a" => answer
        }).unwrap();
    }
    //insert tags into database
    let tags: Vec<Tag> = result.tags;
    for tag in tags.to_owned() {
        let query: String = format!("Insert into ResultTags (resultId, tag,weight,isNegative,amount) VALUES (:r,:t,:w,:i,:a)"); 
        let mut conn = pool.get_conn().unwrap();
        conn.prep_exec(query,params!{
                "r" => resultId.to_owned(),
                "t" => tag.name,
                "w" => tag.weight,
                "i" => tag.negative,
                "a" => tag.amount
        }).unwrap();
    }
    Ok(get_response(format!("{:?}",resultId)))
}

fn get_result(_request: &mut Request) -> IronResult<Response> {  
    let pool: Pool = connect_database();

    let resultId: String = String::from(_request.extensions.get::<Router>().unwrap().find("id").unwrap_or("0"));
    let mut result: Result = Result{
        answers: Vec::new(),
        tags: Vec::new()
    };

    let mut conn = pool.get_conn().unwrap();
    let answerResults = conn.prep_exec("Select answer as id from ResultAnswers where resultId = :r",params!{
                "r" => resultId.to_owned(),
    }).unwrap();
    for row in answerResults {
        let mut r = row.unwrap();
        let answer: i32 = r.take("id").unwrap();
        result.answers.push(answer)
    }
    result.tags = get_tags_of_result(resultId,&pool);
    
    let response: String = serde_json::to_string_pretty(&result).unwrap();
    Ok(get_response(response))
}

fn get_tags_of_result(id: String, pool: &Pool) -> Vec<Tag>{
    let mut tags: Vec<Tag> = Vec::new();
    let mut conn = pool.get_conn().unwrap();
    let tagResults = conn.prep_exec("Select * from ResultTags where resultId = :r",params!{
                "r" => id.to_owned(),
    }).unwrap();
    for row in tagResults {
        let mut r = row.unwrap();
        let tag: Tag = Tag{
            name: r.take("tag").unwrap(),
            weight: r.take("weight").unwrap(),
            amount: r.take("amount").unwrap(),
            negative: r.take("isNegative").unwrap(),
        };
        tags.push(tag);
    }
    return tags;
}

#[derive(Serialize, Deserialize)]
pub struct Visitor{
    pub id: i32,
    pub userAgent: String,
    pub hasDNT: bool,
    pub hasAdblocker: bool,
    pub visitDate: String,
    pub referrer: String,
    pub questions: Vec<Question>,
    pub distros: Vec<Distro>,
    pub i18n:  HashMap<String,i18n>
}
fn new_visitor(_request: &mut Request) -> IronResult<Response> { 
    let pool: Pool = connect_database();
    let lang: String = get_lang(&pool,_request);
    let hasAdblockerRaw: String = String::from(_request.extensions.get::<Router>().unwrap().find("adblocker").unwrap_or(""));
    let hasDNTRaw: String = String::from(_request.extensions.get::<Router>().unwrap().find("dnt").unwrap_or(""));
    let hasAdblocker = if (hasAdblockerRaw == "1" ) { true } else { false };
    let hasDNT = if (hasDNTRaw == "1" ) { true } else { false };
    let mut userAgent: String = String::new();
    if( _request.headers.has::<iron::headers::UserAgent>() ) {
        userAgent = String::from_str(_request.headers.get::<iron::headers::UserAgent>().unwrap().as_str()).unwrap();
    }
    let mut referrer: String = String::new();
    if( _request.headers.has::<iron::headers::Referer>() ) {
        referrer = String::from_str(_request.headers.get::<iron::headers::Referer>().unwrap().as_str()).unwrap();
    }
    let tm = time::now();
    let visitDate = format!("{}",tm.strftime("%Y-%m-%d %H:%M:%S").unwrap());
    
    let query: String = format!("Insert into Visitor (visitDate, userAgent,hasDNT, hasAdblocker, referrer, lang) VALUES (:date,:ua,:dnt,:adblocker,:ref,:l)"); 
    let mut conn = pool.get_conn().unwrap();
    conn.prep_exec(query,params!{
            "date" => visitDate.to_owned(),
            "ua" => userAgent.to_owned(),
            "dnt" => hasDNT,
            "adblocker" => hasAdblocker,
            "ref" => referrer.to_owned(),
            "l" => lang.to_owned()
    }).unwrap();

    let max_id: String = format!("Select max(id) as id from Visitor");
    let mut id: i32 = 0;
    let mut conn = pool.get_conn().unwrap();
    let result = conn.prep_exec(max_id,()).unwrap();
    for row in result {
        let mut r = row.unwrap();
        id = r.take("id").unwrap();
    }


    let v = Visitor{
        id: id,
        userAgent: userAgent,
        hasDNT: hasDNT,
        hasAdblocker: hasAdblocker,
        visitDate: visitDate,
        referrer: referrer,
        questions: query_questions(&pool, &lang),
        distros: query_distributions(&pool, &lang),
        i18n: get_all_translations(&pool, &lang)
    };
    let response: String = serde_json::to_string_pretty(&v).unwrap();
    Ok(get_response(response))
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
    let id: i32 = String::from(_request.extensions.get::<Router>().unwrap().find("id").unwrap()).parse::<i32>().unwrap();
    
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
    pub answers: Vec<Answer>,
    pub answered: bool
}
#[derive(Serialize, Deserialize)]
pub struct Answer{
    pub id: i32,
    pub text: String,
    pub tags: Vec<String>,
    pub excludeTags: Vec<String>,
    pub selected: bool
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
                text: String::new(),
                answered: false
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
/**
* get answers for a single question
*/
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
                excludeTags: Vec::new(),
                selected: false
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
/**
* get all Questions
*/
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

#[derive(Serialize, Deserialize)]
pub struct i18n{
    pub val: String,
    pub translation: String
}

fn get_all_translations(pool: &Pool, lang: &String) -> HashMap<String,i18n>{
    let mut results: HashMap<String,i18n>=  HashMap::new();
    let mut conn = pool.get_conn().unwrap();
    let result = conn.prep_exec("Select * from i18n where langCode = :code",
        params!{
            "code" => lang.to_owned()
        }
    ).unwrap();
    for row in result {
        let mut r = row.unwrap();
        let key: String = r.take("val").unwrap();
        let element = i18n{
            val: key.to_owned(),
            translation: r.take("translation").unwrap()
        };
        results.insert(key, element);
    }
    return results;
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