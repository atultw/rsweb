#[macro_use]
extern crate mongodb;
#[macro_use]
extern crate rsweb_macros;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_json;

mod application;
mod data_mongo;
mod frontend_http;

#[cfg(test)]
mod tests {
    use std::borrow::Borrow;
    use std::collections::HashMap;
    use std::convert::Infallible;
    use std::error::Error;
    use std::net::SocketAddr;
    use std::pin::Pin;
    use std::sync::Arc;

    use async_trait::async_trait;
    use futures::future::BoxFuture;
    use hyper::{Body, Method, Request, Response, Server};
    use mongodb::{bson, Client};
    use mongodb::options::ClientOptions;
    use routerify::{RouterBuilder, RouterService};
    use serde::de;
    use serde_json::Value;
    use serde_json::value::Value::Number;

    use crate::application::{Field, Fields, to_map};
    use crate::data_mongo::DbMongo;
    use crate::frontend_http::{Application, CollectionRoute, Context, DataResource, FrontendExtended, launch, Protected, SingleRoute};

    // example app using the framework
    #[tokio::test]
    async fn server_test() {
        println!("entrypoint");

        #[derive(Serialize, Deserialize, Fields)]
        struct User {
            pub id: u32,
            pub username: String,
        }
        impl DataResource for User {
            fn get_collection_name() -> String {
                String::from("user")
            }
        }

        struct ExampleContext {
            pub signed_in: User,
            pub request: Request<Body>,
        }

        #[async_trait]
        impl Context for ExampleContext {
            async fn generate(req: Request<Body>) -> Self {
                let ctx = ExampleContext {
                    signed_in: User { id: 3, username: "john.doe".to_string() },
                    request: req,
                };
                return ctx;
            }
        }

        #[derive(Serialize, Deserialize, Fields)]
        struct Movie {
            pub id: u32,
            pub year: u32,
            pub title: String,
            pub user_id: u32,
        }
        impl DataResource for Movie {
            fn get_collection_name() -> String {
                "movies".to_string()
            }
        }

        let mut client_options =
            ClientOptions::parse("mongodb://localhost:27017")
                .await.expect("Failed to connect to data layer");
        // Manually set an option
        client_options.app_name = Some("Demo".to_string());
        // Get a handle to the cluster
        let client = Client::with_options(client_options).unwrap();

        let mut app = Application {
            router_builder: RouterBuilder::new(),
            data_source: Arc::new(DbMongo {
                client
            }),
        };

        let r: SingleRoute<Movie, ExampleContext> = SingleRoute {
            path: "/movies/:id".to_string(),
            methods: vec![Method::GET],
            check_to_view: Box::new(|_: &Movie, _: &ExampleContext| Box::pin(async { true })),
            filter_view_data: |ctx, data| {
                let mut map = to_map(&data).unwrap();
                map.insert(String::from("years_since"), json!(2020-data.year));
                if ctx.signed_in.id != data.user_id {
                    map.remove("user_id");
                }
                map
            },
        };
        app.add_route(Arc::new(r));


        let r: SingleRoute<User, ExampleContext> = SingleRoute {
            path: "/users/:id".to_string(),
            methods: vec![Method::GET],
            check_to_view: Box::new(|_, _| Box::pin(async { true })),
            filter_view_data: |ctx, data| {
                let serialized = serde_json::ser::to_string(&data).unwrap();
                let mut map: HashMap<String, Value> = serde_json::de::from_str(&serialized).unwrap();
                if ctx.signed_in.id != data.id {
                    map.remove("id");
                }
                map
            },
        };
        app.add_route(Arc::new(r));
        app.add_route(Arc::new(
            CollectionRoute::<Movie, ExampleContext> {
                path: "/movies".to_string(),
                methods: vec![Method::GET],
                check_to_view: |_| Box::pin(async { true }),
                filter_one: |_, data| { to_map(&data).unwrap() },
            }
        ));

        let service = RouterService::new(app.router_builder.build().unwrap()).unwrap();

        // The address on which the server will be listening.
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

        // Create a server by passing the created service to `.serve` method.
        let server = Server::bind(&addr).serve(service);

        println!("App is running on: {}", addr);

        if let Err(e) = server.await {
            println!("uh oh")
        }
    }
    //
    // pub async fn db_get<T: de::DeserializeOwned>(app: &Application) -> Result<T, hyper::Error> {
    //     // Look up one document:
    //     let movie = app.data_source
    //         .database("app")
    //         .collection("movies")
    //         .find_one(Some(doc! { "_id": "617330461403e1d395959d92" }), None)
    //         .await
    //         .unwrap()
    //         .expect("Document not found");
    //
    //     // Deserialize the document into a Movie instance
    //     let loaded_movie_struct: T = bson::from_bson(bson::Bson::Document(movie)).unwrap();
    //     return Ok(loaded_movie_struct);
    // }
    //
    // #[derive(Serialize, Deserialize)]
    // pub struct Movie {
    //     #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    //     id: Option<bson::oid::ObjectId>,
    //     title: String,
    //     year: i32,
    // }
}
