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
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::Arc;

    use async_trait::async_trait;
    use hyper::{Body, Method, Request, Response, Server};
    use mongodb::{bson, Client};
    use mongodb::options::ClientOptions;
    use routerify::{RouterBuilder, RouterService};
    use serde_json::Value;

    use crate::application::{Field, Fields, to_map};
    use crate::data_mongo::DbMongo;
    use crate::frontend_http::{Application, CollectionRoute, Context, DataResource, FrontendExtended, launch, Protected, SingleRoute};
    use crate::frontend_http::MapOrStruct::Map;

    // example app using the framework
    #[tokio::test]
    async fn server_test() {
        #[derive(Serialize, Deserialize, Fields)]
        struct User {
            #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
            pub id: Option<u32>,
            pub username: String,
        }
        impl DataResource for User {
            fn get_collection_name() -> String {
                String::from("user")
            }

            fn get_id(&self) -> Option<u32> {
                self.id
            }

            fn set_id(&mut self, id: Option<u32>) {
                self.id = id;
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
                    signed_in: User { id: Some(3), username: "john.doe".to_string() },
                    request: req,
                };
                return ctx;
            }
        }

        #[derive(Serialize, Deserialize, Fields)]
        struct Movie {
            #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
            pub id: Option<u32>,
            pub year: u32,
            pub title: String,
            pub user_id: u32,
        }
        impl DataResource for Movie {
            fn get_collection_name() -> String {
                "movies".to_string()
            }

            fn get_id(&self) -> Option<u32> {
                self.id
            }

            fn set_id(&mut self, id: Option<u32>) {
                self.id = id;
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

        app.add_route(
            SingleRoute {
                path: "/movies/:id".to_string(),
                methods: vec![Method::GET],
                check_to_view: |_: &Movie, _: &ExampleContext| Box::pin(async { true }),
                filter_view_data: |ctx, data| {
                    let mut map = to_map(&data).unwrap();
                    map.insert(String::from("years_since"), json!(2030-data.year));
                    if ctx.signed_in.id != Some(data.user_id) {
                        map.remove("user_id");
                    }
                    Map(map)
                },
            }
        );

        app.add_route(
            SingleRoute::<User, ExampleContext> {
                path: "/users/:id".to_string(),
                methods: vec![Method::GET],
                check_to_view: |_, _| Box::pin(async { true }),
                filter_view_data: |ctx, data| {
                    let serialized = serde_json::ser::to_string(&data).unwrap();
                    let mut map: HashMap<String, Value> = serde_json::de::from_str(&serialized).unwrap();
                    if ctx.signed_in.id != data.id {
                        map.remove("id");
                    }
                    Map(map)
                },
            });

        app.add_route(
            CollectionRoute::<Movie, ExampleContext> {
                path: "/movies".to_string(),
                methods: vec![Method::GET, Method::POST],
                check_to_view: |_| Box::pin(async { true }),
                filter_one: |_, data| { to_map(&data).unwrap() },
            }
        );

        let service = RouterService::new(app.router_builder.build().unwrap()).unwrap();

        // The address on which the server will be listening.
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

        // Create a server by passing the created service to `.serve` method.
        let server = Server::bind(&addr).serve(service);

        println!("App will run on: {}", addr);

        if let Err(e) = server.await {
            println!("Failed to start app")
        }
    }
}
