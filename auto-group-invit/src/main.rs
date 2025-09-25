#!/bin/rs

use reqwest::{header::HeaderMap, Client};
use serde::Deserialize;

const CHARACTER_SET: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
const BASE_URL: &str = "https://api.vrchat.cloud/api/1/";
const USER_AGENT: &str = "VRCAI/1.0 support@vrchat.com";
const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(rename = "account")]
    accounts: Vec<Account>,
    group: Group,
}

#[derive(Debug, Deserialize, Clone)]
struct Account {
    name: String,
    cookie: String,
    proxy: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Group {
    name: String,
    id: String,
}

#[derive(Debug, Deserialize)]
struct Target {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
}

struct User {
    client: Client,
    account: Account,
    group: Group,
    offset: char,
}

impl User {
    async fn search_users(&self) -> Vec<Target> {
        let response = self
            .client
            .get(format!(
                "{}users?search={}&sort=last_login&n=10",
                BASE_URL, self.offset
            ))
            .send()
            .await
            .expect("Failed to send request");

        if response.status() != reqwest::StatusCode::OK {
            println!("Oops!! {:?}", response);
            return vec![];
        }

        response.json::<Vec<Target>>().await.unwrap()
    }

    async fn invite_group(&self, target: &Target) {
        let response = self
            .client
            .post(format!("{}groups/{}/invites", BASE_URL, self.group.id))
            .json(&serde_json::json!({
                "userId": target.id,
                "confirmOverrideBlock": "true"
            }))
            .send()
            .await
            .expect("Failed to send request");

        if response.status() != reqwest::StatusCode::OK {
            println!("Oops!! {:?}", response);
        }
    }

    async fn join_group(&self) {
        let response = self
            .client
            .post(format!("{}groups/{}/join", BASE_URL, self.group.id))
            .send()
            .await
            .expect("Failed to send request");

        if response.status() != reqwest::StatusCode::OK {
            println!("Oops!! {:?}", response);
        }
    }

    fn new(account: Account, group: Group, offset: char) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("Cookie", account.cookie.parse().unwrap());

        let client = Client::builder()
            .user_agent(USER_AGENT)
            .default_headers(headers)
            .timeout(DEFAULT_TIMEOUT);

        let client = match &account.proxy {
            Some(proxy) => {
                client.proxy(reqwest::Proxy::https(proxy).expect("Could not load proxy"))
            }
            None => client,
        }
        .build()
        .expect("Could not build client");

        Self {
            client,
            account,
            group,
            offset,
        }
    }

    async fn run(mut self) {
        self.join_group().await;

        let mut num_index = 0;
        loop {
            let index = CHARACTER_SET
                .iter()
                .position(|&c| c == self.offset)
                .unwrap();

            self.offset = CHARACTER_SET[(index + 1) % CHARACTER_SET.len()];

            for target in self.search_users().await {
                num_index += 1;

                self.invite_group(&target).await;
                println!(
                    "({}) Invite sent\n\tTarget: {}\n\tGroup: {}\n\tCookie: {}\n",
                    num_index, target.display_name, self.group.name, self.account.name
                )
            }

            // Wait 3 minutes
            tokio::time::sleep(std::time::Duration::from_secs(180)).await;
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let config = std::fs::read_to_string("config.toml").unwrap();
    let config = toml::from_str::<Config>(&config).unwrap();

    let mut users = Vec::new();

    for (i, account) in config.accounts.iter().enumerate() {
        users.push(User::new(
            account.clone(),
            config.group.clone(),
            CHARACTER_SET[(i * CHARACTER_SET.len() / config.accounts.len()) % CHARACTER_SET.len()],
        ));
    }

    let mut tasks = Vec::new();

    for user in users {
        tasks.push(tokio::spawn(user.run()))
    }

    futures::future::try_join_all(tasks).await.unwrap();
}
