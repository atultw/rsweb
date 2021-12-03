use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::{Debug, Formatter};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::TryStreamExt;
use hyper::{Body, Error, Method, Request, Response, Server, StatusCode};
use hyper::server::conn::AddrIncoming;
use mongodb::bson;
use mongodb::bson::Bson::DateTime;
use mongodb::bson::doc;
use routerify::{RouterBuilder, RouterService};
use routerify::ext::RequestExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::application::{Database, Fields, Filter};
use crate::frontend_http::MapOrStruct::{Map, Struct};

#[async_trait]
pub trait Route<R, S> where R: Serialize + Send + Sync, S: Context + Send + Sync {
    async fn generate_context(&self, request: Request<Body>) -> S;
    async fn handler_get<DB: Database + Send + Sync>(&self, data_layer: Arc<DB>, req: Request<Body>) -> Result<Response<Body>, Infallible>;
    async fn handler_post<DB: Database + Send + Sync>(&self, data_layer: Arc<DB>, req: Request<Body>) -> Result<Response<Body>, Infallible>;
    fn methods(&self) -> &Vec<Method>;
    fn path(&self) -> &String;
}

pub enum MapOrStruct<T: Serialize + Send + Sync> {
    Map(HashMap<String, Value>), Struct(T)
}

pub struct SingleRoute<R, S> where R: Serialize + Send + Sync, S: Context + Send + Sync {
    pub path: String,
    pub methods: Vec<Method>,
    pub check_to_view: fn(&R, &S) -> BoxFuture<'static, bool>,
    pub filter_view_data: fn(&S, R) -> MapOrStruct<R>,
    // pub filters_get: Vec<fn (&R, &mut S, &HashMap<String, serde_json::value::Value>)>
}

pub struct CollectionRoute<R, S> where R: Serialize, S: Context {
    pub path: String,
    pub methods: Vec<Method>,
    pub check_to_view: fn(&S) -> BoxFuture<'static, bool>,
    pub filter_one: fn(&S, R) -> HashMap<String, Value>,
}

#[async_trait]
pub trait FrontendExtended<R> where R: Serialize + DeserializeOwned {
    // take a value from data layer struct and populate calculated fields
    async fn new_with_base(base: R);
}

#[async_trait]
impl<R, S> Route<R, S> for SingleRoute<R, S> where R: DataResource + Fields + Send + Sync , S: Context + Send + Sync{
    async fn generate_context(&self, request: Request<Body>) -> S {
        S::generate(request).await
    }
    async fn handler_get<DB: Database + Send + Sync>(&self, data_layer: Arc<DB>, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut filter = DB::Filter::default();

        if let Some(id) = req.param("id") {
            let parsed: u32 = id.parse().expect("[webf] error parsing id query param");
            filter.insert("_id", parsed);
        }

        let ctx = &self.generate_context(req).await;
        let data = data_layer.retrieve_one(R::get_collection_name(), filter).await;

        if let Ok(Some(data)) = data
        {
            if !(&self.check_to_view)(&data, &ctx).await {
                return Ok(Response::new(Body::from("access denied")))
            }
            let filtered = (&self.filter_view_data)(&ctx, data);
            match filtered {
                Map(map) => { Ok(Response::new(Body::from(serde_json::ser::to_string(&map).unwrap()))) }
                Struct(f) => { Ok(Response::new(Body::from(serde_json::ser::to_string(&f).unwrap()))) }
            }
        } else {
            Ok(Response::builder().status(404).body(Body::from("")).unwrap())
        }
    }

    async fn handler_post<DB: Database + Send + Sync>(&self, data_layer: Arc<DB>, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        Ok(Response::builder().status(405).body(Body::from("")).unwrap())
    }

    fn methods(&self) -> &Vec<Method> {
        &self.methods
    }

    fn path(&self) -> &String {
        &self.path
    }
}


#[async_trait]
impl<R, S> Route<R, S> for CollectionRoute<R, S> where R: DataResource + Fields + Send + Sync , S: Context + Send + Sync {
    async fn generate_context(&self, request: Request<Body>) -> S {
        S::generate(request).await
    }
    async fn handler_get<DB: Database + Send + Sync>(&self, data_layer: Arc<DB>, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        println!("got request");

        let params: HashMap<String, String> = req
            .uri()
            .query()
            .map(|v| {
                url::form_urlencoded::parse(v.as_bytes())
                    .into_owned()
                    .collect()
            })
            .unwrap_or_else(HashMap::new);

        println!("{:?}", params.iter());

        let mut filter = DB::Filter::default();

        for key in R::fields() {
            let name = &*key.name;
            if params.contains_key(name) {
                println!("{}", name);

                if key.is_num {
                    let parsed: u32 = params.get(name).unwrap().parse().expect("[webf] error parsing int query param");
                    filter.insert(name, parsed);
                } else if key.is_bool {
                    if key.name == "true" {
                        filter.insert(key.name, true);
                    } else if key.name == "false" {
                        filter.insert(key.name, false);
                    }
                } else {
                    filter.insert(name, params.get(name).unwrap());
                }
            }
        }

        let ctx = &self.generate_context(req).await;
        if !(&self.check_to_view)(&ctx).await {
            return Ok(Response::new(Body::from("access denied")))
        }

        let mut res = data_layer.retrieve_many(R::get_collection_name(), filter).await;
        match res {
            Ok(res) => {
                let mut maps = vec![];
                for item in res {
                    maps.push((self.filter_one)(&ctx, item));
                }
                match serde_json::to_string(&maps) {
                    Ok(serialized) => {
                        Ok(Response::new(Body::from(serialized)))
                    }
                    Err(err) => {
                        Ok(Response::builder().status(500).body(Body::from("error deserializing")).unwrap())
                    }
                }
            }
            Err(_) => {
                Ok(Response::builder().status(500).body(Body::from("error in data layer")).unwrap())
            }
        }
    }

    async fn handler_post<DB: Database + Send + Sync>(&self, data_layer: Arc<DB>, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        if let Ok(body) = hyper::body::to_bytes(req.into_body()).await {
            let res : serde_json::Result<R> = serde_json::de::from_slice(&*body.to_vec());
            if let Ok(mut deser) = res {
                deser.set_id(Some(std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32));
                let res = data_layer.insert_one(R::get_collection_name(), deser).await;
                match res {
                    Ok(id) => {
                        Ok(Response::new(Body::from(id.to_string())))
                    }
                    Err(_) => {
                        Ok(Response::builder().status(500).body(Body::from("Error inserting")).unwrap())
                    }
                }
            } else {
                Ok(Response::new(Body::from("error deserializing")))
            }
        } else {
            Ok(Response::new(Body::from("internal server error")))
        }
    }

    fn methods(&self) -> &Vec<Method> {
        &self.methods
    }

    fn path(&self) -> &String {
        &self.path
    }
}


#[async_trait]
pub trait Context {
    async fn generate(req: Request<Body>) -> Self;
}

pub trait DataResource: Serialize + DeserializeOwned {
    fn get_collection_name() -> String;
    fn get_id(&self) -> Option<u32>;
    fn set_id(&mut self, id: Option<u32>);
}

#[async_trait]
pub trait Protected {
    fn check_to_view<C: Context>(&self, ctx: &C) -> bool {
        false
    }

    fn check_to_edit<C: Context>(&self, ctx: C) -> bool {
        false
    }

    fn check_to_delete<C: Context>(&self, ctx: C) -> bool {
        false
    }

    fn sanitize_edit_data<C: Context>(&self, ctx: C) {}

    fn filter_view_data<C: Context>(&self, ctx: C, response: &mut HashMap<String, serde_json::value::Value>) {}
}

pub async fn launch<T: Database>(app: Application<T>) -> Server<AddrIncoming, RouterService<Body, Infallible>> {
    // Create a Service from the router above to handle incoming requests.
    let service = RouterService::new(app.router_builder.build().unwrap()).unwrap();

    // The address on which the server will be listening.
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    // Create a server by passing the created service to `.serve` method.
    let server = Server::bind(&addr).serve(service);

    println!("App is running on: {}", addr);
    return server;
}

pub struct Application<T: Database> {
    // pub router_builder: Box<&'static RouterBuilder<Body, Infallible>>,
    pub router_builder: RouterBuilder<Body, Infallible>,
    pub data_source: Arc<T>,
}

impl<T> Application<T> where T: Database + 'static + Send + Sync {
    pub fn add_route<R: 'static + DataResource + Send + Sync, S: 'static +  Context + Send + Sync, RT: 'static + Route<R, S> + Send + Sync>
    (&mut self, route: Arc<RT>) {
        for method in route.methods() {
            match method {
                &Method::GET => {
                    println!("[webf] added GET route {}", route.path());
                    let handler = {
                        let ds = self.data_source.clone();
                        let route = route.clone();
                        move |req| {
                            let ds = ds.clone();
                            let route = route.clone();
                            async move {
                                route.handler_get(ds, req).await
                            }
                        }
                    };
                    let mut refer = std::mem::take(&mut self.router_builder);
                    self.router_builder = refer.get(route.path(), handler);
                }
                &Method::POST => {
                    println!("[webf] added POST route {}", route.path());
                    let handler = {
                        let ds = self.data_source.clone();
                        let route = route.clone();
                        move |req| {
                            let ds = ds.clone();
                            let route = route.clone();
                            async move {
                                route.handler_post(ds, req).await
                            }
                        }
                    };
                    let mut refer = std::mem::take(&mut self.router_builder);
                    self.router_builder = refer.post(route.path(), handler);
                }
                _ => {
                    return
                }
            }
        }
    }
}
