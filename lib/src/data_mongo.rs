use std::collections::HashMap;
use std::error::Error;
use mongodb::{bson, Client};
use bson::Document;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::application::{Database, Filter};
use async_trait::async_trait;
use futures::TryStreamExt;
use mongodb::bson::Bson;
use mongodb::bson::oid::ObjectId;
use serde_json::Value;

impl Filter for Document {
    fn insert<KT: Into<String>, BT: Into<Bson>>(&mut self, key: KT, val: BT) -> Option<Bson> {
        self.insert(key.into(), val.into())
    }
}

pub struct DbMongo {
    pub client: Client,
}

#[async_trait]
impl Database for DbMongo {
    type Filter = Document;
    type Error = mongodb::error::Error;

    async fn retrieve_one<T: Send + Serialize + DeserializeOwned>(&self, table_name: String, filter: Self::Filter) -> Result<Option<T>, Self::Error> {
        let result = self.client
            .database("app")
            .collection(&*table_name)
            .find_one(Some(filter), None)
            .await;


        match result {
            Ok(resp) => {
                match resp {
                    None => { Ok(None) }
                    Some(doc) => {
                        match bson::from_bson(bson::Bson::Document(doc)) {
                            Ok(o) => { Ok(o) }
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

    async fn retrieve_many<T: Send + Serialize + DeserializeOwned>(&self, table_name: String, filter: Self::Filter) -> Result<Vec<T>, Self::Error> {
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
                    return Err(Self::Error::from(err));
                }
            }
        }
        return Ok(res);
    }

    async fn insert_one<T: Send + Sync + Serialize + DeserializeOwned>(&self, table_name: String, item: T) -> Result<u32, Self::Error> {
        match self.client
            .database("app")
            .collection(&*table_name)
            .insert_one(item, None)
            .await {
            Ok(result) => {
                match result.inserted_id {
                    Bson::Int32(id) => {
                        Ok(id as u32)
                    }
                    _ => {
                        println!("{}", result.inserted_id);
                        panic!("Object ID isn't valid")
                    }
                }
            }
            Err(err) => {
                Err(err)
            }
        }
    }

    async fn insert_many<T: Send + Sync + Serialize + DeserializeOwned>(&self, table_name: String, items: Vec<T>) -> Result<Vec<u32>, Self::Error> {
        todo!()
    }
}
