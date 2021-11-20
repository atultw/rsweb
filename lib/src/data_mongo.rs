use std::collections::HashMap;
use std::error::Error;
use mongodb::{bson, Client};
use bson::Document;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::application::Database;
use async_trait::async_trait;
use futures::TryStreamExt;
use serde_json::Value;


pub struct DbMongo {
    pub client: Client
}

#[async_trait]
impl Database for DbMongo {
    type Filter = Document;
    type Error = mongodb::error::Error;

    async fn retrieve_one<T: Send + Serialize + DeserializeOwned>(&self, table_name: String, filter: mongodb::bson::Document) -> Result<Option<T>, Self::Error> {
        let result = self.client
            .database("app")
            .collection(&*table_name)
            .find_one(Some(filter), None)
            .await;


        match result {
            Ok(resp) => {
                match resp {
                    None => {Ok(None)}
                    Some(doc) => {
                        match bson::from_bson(bson::Bson::Document(doc)) {
                            Ok(o) => {Ok(o)}
                            Err(err) => { Err(Self::Error::from(err)) }
                        }
                    }
                }
            }
            Err(bad) => {
                Err(bad)
            }
        }
    }

    async fn retrieve_many<T: Send + Serialize + DeserializeOwned>(&self, table_name: String, filter: mongodb::bson::Document) -> Result<Vec<T>, Self::Error> {
        let mut cursor = self.client
            .database("app")
            .collection(&*table_name)
            .find(Some(filter), None)
            .await.unwrap();

        let mut res: Vec<T> = vec![];

        while let Ok(Some(item)) = cursor.try_next().await {
            println!("next item: {}", item);
            match bson::from_bson(bson::Bson::Document(item)) {
                Ok(loaded) => {
                    res.push(loaded);
                }
                Err(err) => {
                    return Err(Self::Error::from(err))
                }
            }
        }
        return Ok(res)
    }

    async fn insert_one<T: Send + Serialize + DeserializeOwned, ID>(&self, table_name: String) -> Result<ID, Self::Error> {
        todo!()
    }

    async fn insert_many<T: Send + Serialize + DeserializeOwned, ID>(&self, table_name: String) -> Result<Vec<ID>, Self::Error> {
        todo!()
    }
}
