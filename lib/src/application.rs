use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::fmt::Display;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use futures::future::BoxFuture;
use hyper::{Body, Method, Request, Response, Server};
use hyper::server::conn::AddrIncoming;
use mongodb::{bson, Client};
use mongodb::bson::{Bson, doc};
use mongodb::bson::oid::ObjectId;
use routerify::{Router, RouterBuilder, RouterService};
use routerify::prelude::RequestExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use url::Url;

use crate::frontend_http::{Context, DataResource, Route};

pub fn to_map<T: Serialize + DeserializeOwned>(data: &T) -> Option<HashMap<String, Value>>{
    if let Ok(s) = serde_json::ser::to_string(data) {
        if let Ok(m) = serde_json::de::from_str(&*s) {
            m
        } else {
            None
        }
    } else {
        None
    }
}

pub struct Field {
    pub name: String,
    pub is_num: bool,
    pub is_bool: bool,
}

impl Field {
    pub fn new(name: &str) -> Self {
        return Field {
            name: name.to_string(),
            is_num: false,
            is_bool: false
        }
    }
}

pub trait Fields {
    fn fields() -> Vec<Field>;
}

pub trait Filter: Default {
    fn insert<KT: Into<String>, BT: Into<Bson>>(&mut self, key: KT, val: BT) -> Option<Bson>;
}

// driver-agnostic database representation
#[async_trait]
pub trait Database {
    type Filter: Filter + Send + Sync;
    type Error: Error + Send + Sync;
    async fn retrieve_one<T: Send + Serialize + DeserializeOwned>(&self, table_name: String, filter: Self::Filter) -> Result<Option<T>, Self::Error>;
    async fn retrieve_many<T: Send + Serialize + DeserializeOwned>(&self, table_name: String, filter: Self::Filter) -> Result<Vec<T>, Self::Error>;
    async fn insert_one<T: Send + Sync + Serialize + DeserializeOwned>(&self, table_name: String, item: T) -> Result<u32, Self::Error>;
    async fn insert_many<T: Send + Sync + Serialize + DeserializeOwned>(&self, table_name: String, items: Vec<T>) -> Result<Vec<u32>, Self::Error>;
}

